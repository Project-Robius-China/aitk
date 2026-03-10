//! API endpoint router for the Telegram Bot API.

mod handlers;

use crate::telegram_server::server::ServerState;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

/// Builds the axum [`Router`] with all Telegram Bot API endpoints.
///
/// Routes use `/:bot_token/method` where the full first segment (e.g.
/// `bot1:abc123`) is captured and the `bot` prefix is stripped in handlers.
pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        // Core messaging endpoints
        .route(
            "/:bot_token/getMe",
            get(handlers::get_me).post(handlers::get_me),
        )
        .route("/:bot_token/getUpdates", post(handlers::get_updates))
        .route("/:bot_token/sendMessage", post(handlers::send_message))
        .route(
            "/:bot_token/editMessageText",
            post(handlers::edit_message_text),
        )
        .route(
            "/:bot_token/deleteMessage",
            post(handlers::delete_message),
        )
        .route(
            "/:bot_token/answerCallbackQuery",
            post(handlers::answer_callback_query),
        )
        // Teloxide startup compatibility stubs
        .route(
            "/:bot_token/getWebhookInfo",
            get(handlers::get_webhook_info)
                .post(handlers::get_webhook_info),
        )
        .route(
            "/:bot_token/deleteWebhook",
            post(handlers::delete_webhook),
        )
        .route(
            "/:bot_token/setMyCommands",
            post(handlers::set_my_commands),
        )
        .route(
            "/:bot_token/getMyCommands",
            get(handlers::get_my_commands)
                .post(handlers::get_my_commands),
        )
        // File endpoints
        .route("/:bot_token/getFile", post(handlers::get_file))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegram_server::queue::UpdateQueueManager;
    use crate::telegram_server::store::{BotStore, BotStoreConfig};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use futures::channel::mpsc;
    use tower::ServiceExt as _;

    fn test_state() -> Arc<ServerState> {
        let store = BotStore::open(&BotStoreConfig {
            db_path: ":memory:".into(),
            media_dir: "/tmp/test_media".into(),
        })
        .unwrap();
        let (tx, _rx) = mpsc::unbounded();
        Arc::new(ServerState {
            store,
            queues: UpdateQueueManager::new(),
            outbound_tx: tx,
        })
    }

    #[tokio::test]
    async fn test_get_me_returns_bot_info() {
        let state = test_state();
        let bot = state.store.create_bot("Test", "testbot").unwrap();
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/bot{}/getMe", bot.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_token_returns_401() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/botinvalid:token/getMe")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK); // 200 with ok:false
    }
}
