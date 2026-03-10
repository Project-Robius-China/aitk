//! Example: Telegram Bot API compatible server.
//!
//! Starts the server, creates a test bot, and demonstrates the full
//! message round-trip: user → push_update → getUpdates → bot → sendMessage → outbound.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p telegram-server-example
//! ```
//!
//! Then in another terminal, use curl to interact:
//!
//! ```sh
//! # The bot token is printed at startup — replace {TOKEN} below.
//!
//! # Verify the bot identity
//! curl -s http://127.0.0.1:8488/bot{TOKEN}/getMe | jq
//!
//! # Start long-polling (blocks up to 30s waiting for messages)
//! curl -s -X POST http://127.0.0.1:8488/bot{TOKEN}/getUpdates \
//!   -H 'Content-Type: application/json' -d '{"timeout":30}' | jq
//!
//! # Bot sends a message (appears in the outbound log)
//! curl -s -X POST http://127.0.0.1:8488/bot{TOKEN}/sendMessage \
//!   -H 'Content-Type: application/json' \
//!   -d '{"chat_id":"12345","text":"Hello from bot!"}' | jq
//! ```

use aitk::telegram_server::*;
use futures::StreamExt;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // 1. Start the server
    let config = ServerConfig {
        port: 8488,
        db_path: "test_bots.db".into(),
        media_dir: "test_media".into(),
    };
    let (handle, state, mut outbound_rx) = TelegramBotApiServer::start(config)
        .await
        .expect("Failed to start server");

    println!("Server listening on http://{}", handle.addr);

    // 2. Create a test bot (or reuse from a previous run)
    let bot = match state.store.create_bot("TestBot", "test_bot") {
        Ok(b) => b,
        Err(BotApiError::DatabaseError(e)) if e.contains("UNIQUE") => {
            state
                .store
                .list_bots()
                .expect("list_bots")
                .into_iter()
                .find(|b| b.username == "test_bot")
                .expect("test_bot not found")
        }
        Err(e) => panic!("Failed to create bot: {e}"),
    };

    let port = handle.addr.port();
    println!();
    println!("=== Test Bot ===");
    println!("  ID       : {}", bot.id);
    println!("  Username : @{}", bot.username);
    println!("  Token    : {}", bot.token);
    println!();
    println!("--- Try these commands ---");
    println!();
    println!("  # 1. Verify bot identity");
    println!(
        "  curl -s http://127.0.0.1:{port}/bot{token}/getMe | jq",
        token = bot.token
    );
    println!();
    println!("  # 2. Long-poll for updates (run this first, then step 3 in another terminal)");
    println!(
        "  curl -s -X POST http://127.0.0.1:{port}/bot{token}/getUpdates \\",
        token = bot.token
    );
    println!("    -H 'Content-Type: application/json' -d '{{\"timeout\":30}}' | jq");
    println!();
    println!("  # 3. Bot sends a message (watch the [outbound] log below)");
    println!(
        "  curl -s -X POST http://127.0.0.1:{port}/bot{token}/sendMessage \\",
        token = bot.token
    );
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -d '{{\"chat_id\":\"12345\",\"text\":\"Hello from bot!\"}}' | jq");
    println!();

    // 3. Simulate a user message after 3s (wakes any active poller)
    let push_state = Arc::clone(&state);
    let push_token = bot.token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let update_json = serde_json::json!({
            "message": {
                "message_id": 100,
                "from": {
                    "id": 12345,
                    "is_bot": false,
                    "first_name": "TestUser"
                },
                "chat": {
                    "id": 12345,
                    "type": "private"
                },
                "date": chrono::Utc::now().timestamp(),
                "text": "Hello bot! This is a simulated user message."
            }
        })
        .to_string();

        match push_state.push_update(&push_token, &update_json) {
            Ok(id) => println!("[sim] Pushed user message (update_id={id}). Active pollers will receive it."),
            Err(e) => eprintln!("[sim] Failed to push update: {e}"),
        }
    });

    // 4. Log outbound events (bot replies)
    let outbound_task = tokio::spawn(async move {
        while let Some(event) = outbound_rx.next().await {
            match event {
                OutboundEvent::SendMessage {
                    chat_id,
                    message,
                    ..
                } => {
                    println!(
                        "[outbound] SendMessage chat={chat_id} text={:?}",
                        message.text
                    );
                }
                OutboundEvent::EditMessage {
                    chat_id,
                    message_id,
                    new_text,
                    ..
                } => {
                    println!(
                        "[outbound] EditMessage chat={chat_id} msg_id={message_id} text={new_text:?}"
                    );
                }
                OutboundEvent::DeleteMessage {
                    chat_id,
                    message_id,
                    ..
                } => {
                    println!(
                        "[outbound] DeleteMessage chat={chat_id} msg_id={message_id}"
                    );
                }
            }
        }
    });

    // 5. Wait for Ctrl+C
    println!("Press Ctrl+C to stop.\n");
    tokio::signal::ctrl_c()
        .await
        .expect("ctrl_c listener failed");

    println!("\nShutting down...");
    handle.shutdown();
    outbound_task.abort();
}
