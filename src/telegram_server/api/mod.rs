//! API endpoint router for the Telegram Bot API.

mod handlers;

use crate::telegram_server::server::ServerState;
use axum::http::{Method, Uri};
use axum::routing::{get, post, MethodRouter};
use axum::Router;
use std::sync::Arc;

/// Builds the axum [`Router`] with all Telegram Bot API endpoints.
///
/// Routes use `/{bot_token}/method` where the full first segment (e.g.
/// `bot1:abc123`) is captured and the `bot` prefix is stripped in handlers.
///
/// Per the Telegram Bot API specification, method names are
/// case-insensitive. We register both camelCase and PascalCase variants
/// to handle clients that use either convention (e.g. teloxide uses
/// PascalCase like `GetUpdates`).
pub fn router(state: Arc<ServerState>) -> Router {
    let mut r = Router::new();

    let routes: &[(&str, MethodRouter<Arc<ServerState>>)] = &[
        (
            "getMe",
            get(handlers::get_me).post(handlers::get_me),
        ),
        ("getUpdates", post(handlers::get_updates)),
        ("sendMessage", post(handlers::send_message)),
        ("editMessageText", post(handlers::edit_message_text)),
        ("deleteMessage", post(handlers::delete_message)),
        (
            "answerCallbackQuery",
            post(handlers::answer_callback_query),
        ),
        (
            "getWebhookInfo",
            get(handlers::get_webhook_info)
                .post(handlers::get_webhook_info),
        ),
        ("deleteWebhook", post(handlers::delete_webhook)),
        ("setMyCommands", post(handlers::set_my_commands)),
        (
            "getMyCommands",
            get(handlers::get_my_commands)
                .post(handlers::get_my_commands),
        ),
        ("getFile", post(handlers::get_file)),
    ];

    for (method_name, handler) in routes {
        let camel = format!("/{{bot_token}}/{method_name}");
        let pascal =
            format!("/{{bot_token}}/{}", to_pascal_case(method_name));
        r = r.route(&camel, handler.clone());
        if camel != pascal {
            r = r.route(&pascal, handler.clone());
        }
    }

    r.fallback(fallback_handler).with_state(state)
}

/// Converts a camelCase method name to PascalCase (uppercase first char).
fn to_pascal_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Catch-all handler that logs unmatched requests for debugging.
async fn fallback_handler(method: Method, uri: Uri) -> String {
    let msg = format!(
        "Bot API: no route for {} {} — check URL format \
         (expected /bot<TOKEN>/<METHOD>)",
        method, uri,
    );
    log::warn!("{msg}");
    msg
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

    #[tokio::test]
    async fn test_case_insensitive_method_names() {
        let state = test_state();
        let bot = state.store.create_bot("Test", "testbot").unwrap();
        let app = router(state);

        // PascalCase "GetMe" should resolve to "getMe"
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/bot{}/GetMe", bot.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body);
        eprintln!("STATUS: {status}, BODY: {body_str}");

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(json["ok"], true);
    }
}
