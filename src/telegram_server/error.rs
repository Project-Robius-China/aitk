//! Error types for the Telegram Bot API server.

use std::fmt;

/// Errors that can occur in the Bot API server.
#[derive(Debug)]
pub enum BotApiError {
    /// The provided token is invalid or does not match any registered bot.
    InvalidToken,
    /// No bot exists with the given identifier.
    BotNotFound,
    /// The request body is malformed or missing required fields.
    InvalidRequest(String),
    /// Another `getUpdates` long-poller is already connected for this bot.
    ConflictPoller,
    /// The per-bot update queue has reached its capacity limit.
    QueueFull,
    /// An error occurred in the SQLite database layer.
    DatabaseError(String),
    /// An unexpected internal error.
    InternalError(String),
}

impl fmt::Display for BotApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToken => write!(f, "Invalid bot token"),
            Self::BotNotFound => write!(f, "Bot not found"),
            Self::InvalidRequest(msg) => write!(f, "Invalid request: {msg}"),
            Self::ConflictPoller => {
                write!(f, "Conflict: another getUpdates is already active")
            }
            Self::QueueFull => write!(f, "Update queue is full"),
            Self::DatabaseError(msg) => write!(f, "Database error: {msg}"),
            Self::InternalError(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for BotApiError {}

impl BotApiError {
    /// Returns the corresponding HTTP status code for this error.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::InvalidToken => 401,
            Self::BotNotFound => 404,
            Self::InvalidRequest(_) => 400,
            Self::ConflictPoller => 409,
            Self::QueueFull => 429,
            Self::DatabaseError(_) | Self::InternalError(_) => 500,
        }
    }
}

impl From<rusqlite::Error> for BotApiError {
    fn from(err: rusqlite::Error) -> Self {
        Self::DatabaseError(err.to_string())
    }
}
