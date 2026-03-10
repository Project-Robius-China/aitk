//! HTTP handler functions for each Telegram Bot API endpoint.

use crate::telegram_server::error::BotApiError;
use crate::telegram_server::server::ServerState;
use crate::telegram_server::types::*;
use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use std::time::Duration;

type AppState = State<Arc<ServerState>>;

/// Strips the `bot` prefix from the path segment and looks up the bot.
///
/// Telegram URLs use `/bot<token>/method`; our router captures the full
/// segment `bot<token>` as `{bot_token}`, so we strip "bot" here.
fn extract_token(raw: &str) -> &str {
    raw.strip_prefix("bot").unwrap_or(raw)
}

/// Validates a bot token and returns the bot info on success.
fn validate_token(
    state: &ServerState,
    raw_path: &str,
) -> Result<(BotInfo, String), (i32, &'static str)> {
    let token = extract_token(raw_path);
    let bot = state
        .store
        .get_bot_by_token(token)
        .map_err(|_| (500, "Internal error"))?
        .ok_or((401, "Unauthorized"))?;
    Ok((bot, token.to_owned()))
}

// --- Core endpoints ---

/// `GET/POST /bot{token}/getMe` — returns bot identity.
pub async fn get_me(
    State(state): AppState,
    Path(bot_token): Path<String>,
) -> Json<ApiResponse<User>> {
    match validate_token(&state, &bot_token) {
        Ok((bot, _)) => Json(ApiResponse::ok(User {
            id: bot.id,
            is_bot: true,
            first_name: bot.name,
            last_name: None,
            username: Some(bot.username),
        })),
        Err((code, desc)) => Json(ApiResponse {
            ok: false,
            result: None,
            error_code: Some(code),
            description: Some(desc.into()),
        }),
    }
}

/// `POST /bot{token}/getUpdates` — long-polling endpoint.
///
/// Blocks until new updates are available or `timeout` seconds elapse.
/// Only one concurrent poller is allowed per bot.
pub async fn get_updates(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(req): Json<GetUpdatesRequest>,
) -> Json<ApiResponse<Vec<Update>>> {
    let bot = match validate_token(&state, &bot_token) {
        Ok((b, _)) => b,
        Err((code, desc)) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(code),
                description: Some(desc.into()),
            });
        }
    };

    let offset = req.offset.unwrap_or(0);
    let timeout_secs = req.timeout.unwrap_or(30).min(60);
    let limit = req.limit.unwrap_or(100).min(100);

    // Check for already-pending updates.
    if let Ok(updates) = state.store.get_updates(bot.id, offset, limit) {
        if !updates.is_empty() {
            return Json(ApiResponse::ok(deserialize_updates(updates)));
        }
    }

    // No updates yet — register poller and wait.
    let rx = match state.queues.register_poller(bot.id) {
        Ok(rx) => rx,
        Err(BotApiError::ConflictPoller) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(409),
                description: Some(
                    "Conflict: another getUpdates is active".into(),
                ),
            });
        }
        Err(_) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(500),
                description: Some("Internal error".into()),
            });
        }
    };

    let timeout_fut = tokio::time::sleep(Duration::from_secs(timeout_secs));
    let mut rx = rx;

    tokio::select! {
        _ = timeout_fut => {}
        _ = futures::StreamExt::next(&mut rx) => {}
    }

    state.queues.unregister_poller(bot.id);

    let updates = state
        .store
        .get_updates(bot.id, offset, limit)
        .unwrap_or_default();
    Json(ApiResponse::ok(deserialize_updates(updates)))
}

/// `POST /bot{token}/sendMessage` — bot sends a text message.
pub async fn send_message(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Json<ApiResponse<Message>> {
    let (bot, token) = match validate_token(&state, &bot_token) {
        Ok(pair) => pair,
        Err((code, desc)) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(code),
                description: Some(desc.into()),
            });
        }
    };

    let chat_id = parse_chat_id(&req.chat_id);
    let reply_markup_json = req
        .reply_markup
        .as_ref()
        .and_then(|rm| serde_json::to_string(rm).ok());

    let msg_id = match state.store.store_message(
        bot.id,
        chat_id,
        true,
        Some(&req.text),
        None,
        None,
        None,
        reply_markup_json.as_deref(),
    ) {
        Ok(id) => id,
        Err(_) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(500),
                description: Some("Failed to store message".into()),
            });
        }
    };

    let message = build_bot_message(
        msg_id,
        &bot,
        chat_id,
        Some(req.text),
        req.reply_markup,
    );

    let _ =
        state
            .outbound_tx
            .unbounded_send(OutboundEvent::SendMessage {
                bot_token: token,
                chat_id,
                message: message.clone(),
            });

    Json(ApiResponse::ok(message))
}

/// `POST /bot{token}/editMessageText` — edit an existing message.
pub async fn edit_message_text(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(req): Json<EditMessageTextRequest>,
) -> Json<ApiResponse<Message>> {
    let (bot, token) = match validate_token(&state, &bot_token) {
        Ok(pair) => pair,
        Err((code, desc)) => {
            return Json(ApiResponse {
                ok: false,
                result: None,
                error_code: Some(code),
                description: Some(desc.into()),
            });
        }
    };

    let chat_id = parse_chat_id(&req.chat_id);

    let _ =
        state
            .outbound_tx
            .unbounded_send(OutboundEvent::EditMessage {
                bot_token: token,
                chat_id,
                message_id: req.message_id,
                new_text: req.text.clone(),
                reply_markup: req.reply_markup.clone(),
            });

    let message = build_bot_message(
        req.message_id,
        &bot,
        chat_id,
        Some(req.text),
        req.reply_markup,
    );
    Json(ApiResponse::ok(message))
}

/// `POST /bot{token}/deleteMessage` — delete a message.
pub async fn delete_message(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(req): Json<DeleteMessageRequest>,
) -> Json<ApiResponse<bool>> {
    let token = match validate_token(&state, &bot_token) {
        Ok((_, t)) => t,
        Err((code, desc)) => {
            return Json(ApiResponse::<bool>::error(code, desc));
        }
    };

    let chat_id = parse_chat_id(&req.chat_id);
    let _ = state.outbound_tx.unbounded_send(
        OutboundEvent::DeleteMessage {
            bot_token: token,
            chat_id,
            message_id: req.message_id,
        },
    );

    Json(ApiResponse::ok(true))
}

/// `POST /bot{token}/answerCallbackQuery` — acknowledge a button press.
pub async fn answer_callback_query(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(_req): Json<AnswerCallbackQueryRequest>,
) -> Json<ApiResponse<bool>> {
    match validate_token(&state, &bot_token) {
        Ok(_) => Json(ApiResponse::ok(true)),
        Err((code, desc)) => {
            Json(ApiResponse::<bool>::error(code, desc))
        }
    }
}

// --- Teloxide compatibility stubs ---

/// `GET/POST /bot{token}/getWebhookInfo` — returns empty webhook info.
pub async fn get_webhook_info(
    State(state): AppState,
    Path(bot_token): Path<String>,
) -> Json<ApiResponse<WebhookInfo>> {
    match validate_token(&state, &bot_token) {
        Ok(_) => Json(ApiResponse::ok(WebhookInfo {
            url: String::new(),
            has_custom_certificate: false,
            pending_update_count: 0,
        })),
        Err((code, desc)) => Json(ApiResponse {
            ok: false,
            result: None,
            error_code: Some(code),
            description: Some(desc.into()),
        }),
    }
}

/// `POST /bot{token}/deleteWebhook` — no-op stub.
pub async fn delete_webhook(
    State(state): AppState,
    Path(bot_token): Path<String>,
) -> Json<ApiResponse<bool>> {
    match validate_token(&state, &bot_token) {
        Ok(_) => Json(ApiResponse::ok(true)),
        Err((code, desc)) => {
            Json(ApiResponse::<bool>::error(code, desc))
        }
    }
}

/// `POST /bot{token}/setMyCommands` — accepts and discards commands.
pub async fn set_my_commands(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(_req): Json<SetMyCommandsRequest>,
) -> Json<ApiResponse<bool>> {
    match validate_token(&state, &bot_token) {
        Ok(_) => Json(ApiResponse::ok(true)),
        Err((code, desc)) => {
            Json(ApiResponse::<bool>::error(code, desc))
        }
    }
}

/// `GET/POST /bot{token}/getMyCommands` — returns empty list.
pub async fn get_my_commands(
    State(state): AppState,
    Path(bot_token): Path<String>,
) -> Json<ApiResponse<Vec<BotCommand>>> {
    match validate_token(&state, &bot_token) {
        Ok(_) => Json(ApiResponse::ok(vec![])),
        Err((code, desc)) => Json(ApiResponse {
            ok: false,
            result: None,
            error_code: Some(code),
            description: Some(desc.into()),
        }),
    }
}

/// `POST /bot{token}/getFile` — returns file path for downloading.
pub async fn get_file(
    State(state): AppState,
    Path(bot_token): Path<String>,
    Json(req): Json<GetFileRequest>,
) -> Json<ApiResponse<File>> {
    if let Err((code, desc)) = validate_token(&state, &bot_token) {
        return Json(ApiResponse {
            ok: false,
            result: None,
            error_code: Some(code),
            description: Some(desc.into()),
        });
    }

    match state.store.get_media(&req.file_id) {
        Ok(Some((path, _))) => {
            let relative = path.rsplit('/').next().unwrap_or(&path);
            Json(ApiResponse::ok(File {
                file_id: req.file_id.clone(),
                file_unique_id: req.file_id,
                file_size: None,
                file_path: Some(format!("media/{relative}")),
            }))
        }
        _ => Json(ApiResponse {
            ok: false,
            result: None,
            error_code: Some(404),
            description: Some("File not found".into()),
        }),
    }
}

// --- Helpers ---

fn deserialize_updates(rows: Vec<(i64, String)>) -> Vec<Update> {
    rows.into_iter()
        .filter_map(|(id, payload)| {
            serde_json::from_str::<Update>(&payload)
                .map(|mut u| {
                    u.update_id = id;
                    u
                })
                .ok()
        })
        .collect()
}

fn build_bot_message(
    message_id: i64,
    bot: &BotInfo,
    chat_id: i64,
    text: Option<String>,
    reply_markup: Option<InlineKeyboardMarkup>,
) -> Message {
    Message {
        message_id,
        from: Some(User {
            id: bot.id,
            is_bot: true,
            first_name: bot.name.clone(),
            last_name: None,
            username: Some(bot.username.clone()),
        }),
        chat: Chat {
            id: chat_id,
            chat_type: "private".into(),
            title: None,
            first_name: None,
            username: None,
        },
        date: chrono::Utc::now().timestamp(),
        text,
        caption: None,
        photo: None,
        voice: None,
        audio: None,
        document: None,
        reply_markup,
    }
}
