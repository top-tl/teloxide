# toptl-teloxide

Teloxide plugin for [TOP.TL](https://top.tl) — auto-post bot stats and check votes.

## Installation

```toml
[dependencies]
toptl-teloxide = "1.0"
```

## Quick start

```rust
use teloxide::prelude::*;
use toptl::TopTLClient;
use toptl_teloxide::{TopTLPlugin, record_update};

#[tokio::main]
async fn main() {
    let client = TopTLClient::new("your-api-token");
    let plugin = TopTLPlugin::new(client, "mybot");
    plugin.start(); // spawns background stats posting

    let bot = Bot::from_env();

    teloxide::repl(bot, move |msg: Message| {
        let plugin = plugin.clone();
        async move {
            record_update(&plugin, &msg).await;
            // ... your handler logic
            respond(())
        }
    })
    .await;
}
```

## Vote-gating

```rust
async fn premium_command(plugin: &TopTLPlugin, msg: &Message, bot: &Bot) {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if !plugin.has_voted(user_id).await {
        bot.send_message(msg.chat.id, "Please vote first: https://top.tl/mybot").await?;
        return Ok(());
    }
    bot.send_message(msg.chat.id, "Thanks for voting!").await?;
    Ok(())
}
```

## How it works

- **Tracking** — `record_update` (or manual `record`) stores unique user, group, and channel IDs.
- **Auto-posting** — `start()` spawns a tokio task that posts stats at a server-controlled interval.
- **Vote checks** — `has_voted(user_id)` queries the TOP.TL API.

## License

MIT
