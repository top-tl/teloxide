//! Teloxide plugin for [TOP.TL](https://top.tl) — auto-post bot stats & check votes.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use toptl::TopTLClient;
//! use toptl_teloxide::TopTLPlugin;
//!
//! let client = TopTLClient::new("your-api-token");
//! let plugin = TopTLPlugin::new(client, "mybot");
//! plugin.start(); // spawns background posting task
//! ```

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

/// Re-export for convenience.
pub use toptl::TopTLClient;

/// Tracks unique chat IDs and auto-posts stats to TOP.TL.
#[derive(Clone)]
pub struct TopTLPlugin {
    client: Arc<TopTLClient>,
    username: String,
    state: Arc<Mutex<PluginState>>,
}

struct PluginState {
    user_ids: HashSet<i64>,
    group_ids: HashSet<i64>,
    channel_ids: HashSet<i64>,
    post_interval_secs: u64,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            user_ids: HashSet::new(),
            group_ids: HashSet::new(),
            channel_ids: HashSet::new(),
            post_interval_secs: 300,
        }
    }
}

/// Chat type for [`TopTLPlugin::record`].
#[derive(Debug, Clone, Copy)]
pub enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
}

impl TopTLPlugin {
    /// Create a new plugin instance.
    pub fn new(client: TopTLClient, username: impl Into<String>) -> Self {
        Self {
            client: Arc::new(client),
            username: username.into(),
            state: Arc::new(Mutex::new(PluginState::default())),
        }
    }

    /// Record a user and/or chat from an incoming update.
    ///
    /// Call this from your handler or middleware to track unique IDs.
    ///
    /// ```rust,no_run
    /// # use toptl_teloxide::{TopTLPlugin, ChatKind};
    /// # async fn example(plugin: &TopTLPlugin) {
    /// plugin.record(Some(12345), Some((-100123, ChatKind::Group))).await;
    /// # }
    /// ```
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
                ChatKind::Private => {
                    // Private chats are counted as users, already tracked above.
                }
            }
        }
    }

    /// Spawn a background tokio task that periodically posts stats to TOP.TL.
    ///
    /// The interval is initially 300 s and adapts to the server response.
    pub fn start(&self) {
        let client = self.client.clone();
        let username = self.username.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut interval_duration = {
                let s = state.lock().await;
                Duration::from_secs(s.post_interval_secs)
            };

            loop {
                time::sleep(interval_duration).await;

                let stats = {
                    let s = state.lock().await;
                    toptl::BotStats {
                        users: s.user_ids.len() as u64,
                        groups: s.group_ids.len() as u64,
                        channels: s.channel_ids.len() as u64,
                    }
                };

                match client.post_stats(&username, &stats).await {
                    Ok(resp) => {
                        if let Some(new_interval) = resp.interval {
                            let new_dur = Duration::from_secs(new_interval);
                            if new_dur != interval_duration {
                                interval_duration = new_dur;
                            }
                        }
                        log::debug!("toptl: posted stats for {}", username);
                    }
                    Err(e) => {
                        log::warn!("toptl: failed to post stats: {}", e);
                    }
                }
            }
        });
    }

    /// Check whether a user has voted for this bot on TOP.TL.
    pub async fn has_voted(&self, user_id: i64) -> bool {
        match self.client.has_voted(&self.username, user_id).await {
            Ok(voted) => voted,
            Err(e) => {
                log::warn!("toptl: failed to check vote for {}: {}", user_id, e);
                false
            }
        }
    }
}

/// Convenience handler wrapper for teloxide that records every update.
///
/// ```rust,no_run
/// use teloxide::prelude::*;
/// use toptl_teloxide::{TopTLPlugin, record_update};
///
/// async fn handler(plugin: TopTLPlugin, msg: Message) {
///     record_update(&plugin, &msg).await;
///     // ... your logic
/// }
/// ```
#[cfg(feature = "teloxide")]
pub async fn record_update(plugin: &TopTLPlugin, msg: &teloxide::types::Message) {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64);
    let chat = {
        let c = &msg.chat;
        let kind = match c.kind {
            teloxide::types::ChatKind::Private(_) => ChatKind::Private,
            teloxide::types::ChatKind::Public(ref p) => match p.kind {
                teloxide::types::PublicChatKind::Group(_) => ChatKind::Group,
                teloxide::types::PublicChatKind::Supergroup(_) => ChatKind::Supergroup,
                teloxide::types::PublicChatKind::Channel(_) => ChatKind::Channel,
            },
        };
        Some((c.id.0, kind))
    };
    plugin.record(user_id, chat).await;
}
