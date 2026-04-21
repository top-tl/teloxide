//! Teloxide plugin for [TOP.TL](https://top.tl) — autopost stats, gate
//! handlers behind votes, handle vote webhooks.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use toptl::TopTL;
//! use toptl_teloxide::TopTLPlugin;
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = TopTL::new("toptl_xxx");
//!     let plugin = TopTLPlugin::new(client, "mybot");
//!     plugin.start(Duration::from_secs(30 * 60));
//!
//!     // Call plugin.record(...) from your handlers, or use
//!     // record_update(&plugin, &msg).await when the "teloxide"
//!     // feature is enabled.
//! }
//! ```

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

pub use toptl::{StatsPayload, TopTL};

/// Tracks unique users / groups / channels and autoposts counts to TOP.TL.
#[derive(Clone)]
pub struct TopTLPlugin {
    client: Arc<TopTL>,
    username: String,
    state: Arc<Mutex<PluginState>>,
}

#[derive(Default)]
struct PluginState {
    user_ids: HashSet<i64>,
    group_ids: HashSet<i64>,
    channel_ids: HashSet<i64>,
}

/// Chat kind for [`TopTLPlugin::record`].
#[derive(Debug, Clone, Copy)]
pub enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
}

impl TopTLPlugin {
    pub fn new(client: TopTL, username: impl Into<String>) -> Self {
        Self {
            client: Arc::new(client),
            username: username.into(),
            state: Arc::new(Mutex::new(PluginState::default())),
        }
    }

    /// Record one update's IDs into the plugin's counters. Call from
    /// your handler, or use [`record_update`] when the `teloxide`
    /// feature is enabled.
    pub async fn record(&self, user_id: Option<i64>, chat: Option<(i64, ChatKind)>) {
        let mut state = self.state.lock().await;
        if let Some(uid) = user_id {
            state.user_ids.insert(uid);
        }
        if let Some((cid, kind)) = chat {
            match kind {
                ChatKind::Group | ChatKind::Supergroup => {
                    state.group_ids.insert(cid);
                }
                ChatKind::Channel => {
                    state.channel_ids.insert(cid);
                }
                ChatKind::Private => { /* private chat → already counted as user */ }
            }
        }
    }

    /// Spawn a background task that flushes stats to TOP.TL every
    /// `interval`. Keep the plugin alive for the lifetime of your bot.
    pub fn start(&self, interval: Duration) {
        let client = self.client.clone();
        let username = self.username.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            // Skip the immediate tick — let the bot collect at least
            // one update before flushing.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let payload = {
                    let s = state.lock().await;
                    StatsPayload {
                        member_count: Some(s.user_ids.len() as u64),
                        group_count: Some(s.group_ids.len() as u64),
                        channel_count: Some(s.channel_ids.len() as u64),
                        bot_serves: None,
                    }
                };
                match client.post_stats(&username, &payload).await {
                    Ok(_) => log::debug!("toptl: posted stats for @{username}"),
                    Err(e) => log::warn!("toptl: post_stats for @{username} failed: {e}"),
                }
            }
        });
    }

    /// Has `user_id` voted for this bot on TOP.TL?
    /// Network / auth errors fall through as `false` so vote gates
    /// never block your bot — they're also logged at warn level.
    pub async fn has_voted(&self, user_id: i64) -> bool {
        match self.client.has_voted(&self.username, user_id as u64).await {
            Ok(check) => check.voted,
            Err(e) => {
                log::warn!("toptl: has_voted({user_id}) failed: {e}");
                false
            }
        }
    }

    /// Flush current counts immediately. Useful from a shutdown hook.
    pub async fn post_now(&self) -> Result<(), toptl::Error> {
        let payload = {
            let s = self.state.lock().await;
            StatsPayload {
                member_count: Some(s.user_ids.len() as u64),
                group_count: Some(s.group_ids.len() as u64),
                channel_count: Some(s.channel_ids.len() as u64),
                bot_serves: None,
            }
        };
        self.client.post_stats(&self.username, &payload).await?;
        Ok(())
    }
}

/// Helper that records every teloxide `Message` into the plugin.
///
/// ```rust,no_run
/// use teloxide::prelude::*;
/// use toptl_teloxide::{record_update, TopTLPlugin};
///
/// async fn handler(plugin: TopTLPlugin, msg: Message) {
///     record_update(&plugin, &msg).await;
///     // your handler logic …
/// }
/// ```
#[cfg(feature = "teloxide")]
pub async fn record_update(plugin: &TopTLPlugin, msg: &teloxide::types::Message) {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64);
    let kind = match msg.chat.kind {
        teloxide::types::ChatKind::Private(_) => ChatKind::Private,
        teloxide::types::ChatKind::Public(ref p) => match p.kind {
            teloxide::types::PublicChatKind::Group(_) => ChatKind::Group,
            teloxide::types::PublicChatKind::Supergroup(_) => ChatKind::Supergroup,
            teloxide::types::PublicChatKind::Channel(_) => ChatKind::Channel,
        },
    };
    plugin.record(user_id, Some((msg.chat.id.0, kind))).await;
}
