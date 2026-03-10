//! Telegram Bot API compatible HTTP server.
//!
//! Implements a subset of the Telegram Bot API that allows teloxide-based
//! clients (e.g. crew-rs) to connect to this server instead of
//! `api.telegram.org`. The server provides long-polling (`getUpdates`),
//! message sending, editing, deletion, callback queries, and media handling.
//!
//! # Architecture
//!
//! - **types**: Telegram Bot API data types (User, Chat, Message, Update, etc.)
//! - **error**: Error types with HTTP status code mapping
//! - **store**: SQLite-backed persistence for bots, messages, and updates
//! - **queue**: In-memory per-bot update queue with long-polling support
//! - **server**: axum HTTP server exposing the Bot API endpoints
//! - **api**: Request handlers for each endpoint

mod api;
mod error;
mod queue;
mod server;
mod store;
mod types;

pub use error::BotApiError;
pub use queue::UpdateQueueManager;
pub use server::{ServerConfig, ServerHandle, ServerState, TelegramBotApiServer};
pub use store::{BotStore, BotStoreConfig};
pub use types::*;
