# toptl-teloxide

[![Crates.io](https://img.shields.io/crates/v/toptl-teloxide.svg?color=3775a9)](https://crates.io/crates/toptl-teloxide)
[![docs.rs](https://img.shields.io/docsrs/toptl-teloxide/latest?color=3776ab)](https://docs.rs/toptl-teloxide)
[![Downloads](https://img.shields.io/crates/d/toptl-teloxide.svg?color=blue)](https://crates.io/crates/toptl-teloxide)
[![License](https://img.shields.io/crates/l/toptl-teloxide.svg?color=green)](https://github.com/top-tl/teloxide/blob/main/LICENSE)
[![teloxide](https://img.shields.io/badge/teloxide-0.13-26a5e4)](https://github.com/teloxide/teloxide)
[![TOP.TL](https://img.shields.io/badge/top.tl-developers-2ec4b6)](https://top.tl/developers)

[Teloxide](https://github.com/teloxide/teloxide) plugin for [TOP.TL](https://top.tl) — autopost bot stats, gate handlers behind votes, and handle vote webhooks.

## Install

```toml
[dependencies]
toptl-teloxide = "0.1"
teloxide = "0.13"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Built on the [`toptl`](https://crates.io/crates/toptl) SDK.

## Quick start

```rust
use std::time::Duration;
use teloxide::prelude::*;
use toptl::TopTL;
use toptl_teloxide::{record_update, TopTLPlugin};

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();
    let client = TopTL::new("toptl_xxx");
    let plugin = TopTLPlugin::new(client, "mybot");
    plugin.start(Duration::from_secs(30 * 60));

    let plugin_clone = plugin.clone();
    teloxide::repl(bot, move |msg: Message, bot: Bot| {
        let plugin = plugin_clone.clone();
        async move {
            record_update(&plugin, &msg).await;
            bot.send_message(msg.chat.id, "hi").await?;
            Ok::<_, teloxide::RequestError>(())
        }
    })
    .await;
}
```

`plugin.start(...)` spawns a background task that flushes unique user/group/channel counts every interval. `record_update(...)` ingests one incoming message so the counters stay current.

## Vote gating

```rust
async fn premium(msg: Message, bot: Bot, plugin: TopTLPlugin) -> ResponseResult<()> {
    if let Some(user) = &msg.from {
        if !plugin.has_voted(user.id.0 as i64).await {
            bot.send_message(msg.chat.id, "Vote first: https://top.tl/mybot").await?;
            return Ok(());
        }
    }
    bot.send_message(msg.chat.id, "Thanks for voting!").await?;
    Ok(())
}
```

`has_voted` is **fail-open** — network or auth errors count as "not voted" and log, never crash your handler.

## Manual flush

Useful from a shutdown hook:

```rust
plugin.post_now().await?;
```

## License

MIT — see [`LICENSE`](LICENSE).
