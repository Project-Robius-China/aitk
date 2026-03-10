//! Telegram Bot API compatible HTTP server.
//!
//! Starts an axum server implementing the Telegram Bot API endpoints used by
//! teloxide. Exposes [`ServerState::push_update`] for the app layer to inject
//! user messages and an outbound channel for receiving bot replies.

use crate::telegram_server::api;
use crate::telegram_server::error::BotApiError;
use crate::telegram_server::queue::UpdateQueueManager;
use crate::telegram_server::store::{BotStore, BotStoreConfig};
use crate::telegram_server::types::OutboundEvent;
use futures::channel::mpsc;
use std::net::SocketAddr;
use std::sync::Arc;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to listen on (0 for OS-assigned).
    pub port: u16,
    /// Path to the SQLite database file.
    pub db_path: String,
    /// Directory for storing media files.
    pub media_dir: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8488,
            db_path: "moly_bots.db".into(),
            media_dir: "bot_media".into(),
        }
    }
}

/// Handle to a running server, used for address discovery and shutdown.
pub struct ServerHandle {
    /// The address the server is listening on.
    pub addr: SocketAddr,
    shutdown_tx: Option<futures::channel::oneshot::Sender<()>>,
}

impl ServerHandle {
    /// Initiates graceful shutdown of the server.
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Shared state accessible by all API handlers.
pub struct ServerState {
    /// Persistent bot and message storage.
    pub store: BotStore,
    /// Per-bot long-polling queues.
    pub queues: UpdateQueueManager,
    /// Channel to notify the app layer of outbound bot events.
    pub outbound_tx: mpsc::UnboundedSender<OutboundEvent>,
}

impl ServerState {
    /// Pushes a user message as a Telegram [`Update`] for a bot to receive
    /// via `getUpdates`. Returns the assigned `update_id`.
    ///
    /// Called by the application layer when the user sends a message in the UI.
    pub fn push_update(
        &self,
        bot_token: &str,
        update_json: &str,
    ) -> Result<i64, BotApiError> {
        let bot = self
            .store
            .get_bot_by_token(bot_token)?
            .ok_or(BotApiError::InvalidToken)?;
        let update_id = self.store.insert_update(bot.id, update_json)?;
        self.queues.notify_bot(bot.id);
        Ok(update_id)
    }
}

/// Entry point for starting the Telegram Bot API server.
pub struct TelegramBotApiServer;

impl TelegramBotApiServer {
    /// Starts the HTTP server and returns a handle, the shared server state,
    /// and an outbound event receiver for the application layer.
    ///
    /// The returned [`Arc<ServerState>`] allows the caller to inject user
    /// messages via [`ServerState::push_update`].
    pub async fn start(
        config: ServerConfig,
    ) -> Result<
        (
            ServerHandle,
            Arc<ServerState>,
            mpsc::UnboundedReceiver<OutboundEvent>,
        ),
        BotApiError,
    > {
        let store = BotStore::open(&BotStoreConfig {
            db_path: config.db_path,
            media_dir: config.media_dir,
        })?;

        let (outbound_tx, outbound_rx) = mpsc::unbounded();
        let state = Arc::new(ServerState {
            store,
            queues: UpdateQueueManager::new(),
            outbound_tx,
        });

        let app = api::router(Arc::clone(&state));

        let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
        let listener =
            tokio::net::TcpListener::bind(addr).await.map_err(|e| {
                BotApiError::InternalError(format!(
                    "Failed to bind {addr}: {e}"
                ))
            })?;
        let bound_addr = listener.local_addr().map_err(|e| {
            BotApiError::InternalError(format!(
                "Failed to get local addr: {e}"
            ))
        })?;

        let (shutdown_tx, shutdown_rx) =
            futures::channel::oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        Ok((
            ServerHandle {
                addr: bound_addr,
                shutdown_tx: Some(shutdown_tx),
            },
            state,
            outbound_rx,
        ))
    }
}
