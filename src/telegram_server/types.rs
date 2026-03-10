//! Telegram Bot API compatible data types.
//!
//! These types mirror the subset of the Telegram Bot API used by teloxide
//! clients. They serialize/deserialize to the same JSON format as the real API.

use serde::{Deserialize, Serialize};

/// Standard Telegram API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a successful response.
    pub fn ok(result: T) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error_code: None,
            description: None,
        }
    }
}

impl ApiResponse<bool> {
    /// Creates an error response with status code and description.
    pub fn error(code: i32, description: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error_code: Some(code),
            description: Some(description.into()),
        }
    }
}

/// A Telegram user or bot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// A Telegram chat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// A Telegram message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub message_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<User>,
    pub chat: Chat,
    pub date: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo: Option<Vec<PhotoSize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<Voice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<Audio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Document>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

/// An incoming update from the Bot API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Update {
    pub update_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_query: Option<CallbackQuery>,
}

/// A callback query from an inline keyboard button press.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Inline keyboard markup attached to a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

/// A button in an inline keyboard row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InlineKeyboardButton {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// --- Media types ---

/// One size of a photo or file thumbnail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoSize {
    pub file_id: String,
    pub file_unique_id: String,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// A voice message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Voice {
    pub file_id: String,
    pub file_unique_id: String,
    pub duration: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// An audio file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Audio {
    pub file_id: String,
    pub file_unique_id: String,
    pub duration: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// A general file (document).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub file_id: String,
    pub file_unique_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
}

/// File download information returned by `getFile`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct File {
    pub file_id: String,
    pub file_unique_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

// --- Request body types ---

/// Body for `getUpdates` long-polling requests.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GetUpdatesRequest {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default = "default_timeout")]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub limit: Option<u32>,
}

fn default_timeout() -> Option<u64> {
    Some(30)
}

/// Body for `sendMessage`.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub chat_id: serde_json::Value,
    pub text: String,
    #[serde(default)]
    pub parse_mode: Option<String>,
    #[serde(default)]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

/// Body for `editMessageText`.
#[derive(Debug, Clone, Deserialize)]
pub struct EditMessageTextRequest {
    pub chat_id: serde_json::Value,
    pub message_id: i64,
    pub text: String,
    #[serde(default)]
    pub parse_mode: Option<String>,
    #[serde(default)]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

/// Body for `deleteMessage`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteMessageRequest {
    pub chat_id: serde_json::Value,
    pub message_id: i64,
}

/// Body for `answerCallbackQuery`.
#[derive(Debug, Clone, Deserialize)]
pub struct AnswerCallbackQueryRequest {
    pub callback_query_id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub show_alert: bool,
}

/// Body for `getFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct GetFileRequest {
    pub file_id: String,
}

/// A bot command entry for `setMyCommands`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
}

/// Body for `setMyCommands`.
#[derive(Debug, Clone, Deserialize)]
pub struct SetMyCommandsRequest {
    pub commands: Vec<BotCommand>,
}

/// Webhook info returned by `getWebhookInfo` (always empty for us).
#[derive(Debug, Clone, Serialize)]
pub struct WebhookInfo {
    pub url: String,
    pub has_custom_certificate: bool,
    pub pending_update_count: i32,
}

// --- App-layer types (not Telegram API) ---

/// Metadata about a registered bot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BotInfo {
    pub id: i64,
    pub token: String,
    pub name: String,
    pub username: String,
    pub description: String,
    pub about_text: String,
    pub photo_path: Option<String>,
    pub created_at: String,
}

/// Fields to update on a bot (all optional).
#[derive(Debug, Clone, Default)]
pub struct BotUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub about_text: Option<String>,
    pub photo_path: Option<String>,
}

/// An outbound event from a bot to the application layer.
#[derive(Debug, Clone)]
pub enum OutboundEvent {
    /// Bot sent a new text or media message.
    SendMessage {
        bot_token: String,
        chat_id: i64,
        message: Message,
    },
    /// Bot edited an existing message.
    EditMessage {
        bot_token: String,
        chat_id: i64,
        message_id: i64,
        new_text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    },
    /// Bot deleted a message.
    DeleteMessage {
        bot_token: String,
        chat_id: i64,
        message_id: i64,
    },
}

/// Parses a `chat_id` value that may be a number or a string.
pub fn parse_chat_id(value: &serde_json::Value) -> i64 {
    match value {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(1),
        serde_json::Value::String(s) => s.parse().unwrap_or(1),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_ok_serialization() {
        let resp = ApiResponse::ok(User {
            id: 1,
            is_bot: true,
            first_name: "TestBot".into(),
            last_name: None,
            username: Some("test_bot".into()),
        });
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["ok"], true);
        assert_eq!(json["result"]["id"], 1);
        assert!(json.get("error_code").is_none());
    }

    #[test]
    fn test_api_response_error_serialization() {
        let resp = ApiResponse::<bool>::error(401, "Unauthorized");
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["ok"], false);
        assert_eq!(json["error_code"], 401);
    }

    #[test]
    fn test_update_with_message_roundtrip() {
        let json = r#"{
            "update_id": 100,
            "message": {
                "message_id": 1,
                "from": {"id": 1, "is_bot": false, "first_name": "User"},
                "chat": {"id": 1, "type": "private"},
                "date": 1700000000,
                "text": "hello"
            }
        }"#;
        let update: Update = serde_json::from_str(json).expect("deserialize");
        assert_eq!(update.update_id, 100);
        assert_eq!(
            update.message.as_ref().and_then(|m| m.text.as_deref()),
            Some("hello")
        );
    }

    #[test]
    fn test_callback_query_deserialization() {
        let json = r#"{
            "update_id": 101,
            "callback_query": {
                "id": "cq_001",
                "from": {"id": 1, "is_bot": false, "first_name": "User"},
                "data": "option_a"
            }
        }"#;
        let update: Update = serde_json::from_str(json).expect("deserialize");
        let cq = update.callback_query.expect("callback_query");
        assert_eq!(cq.data.as_deref(), Some("option_a"));
    }

    #[test]
    fn test_inline_keyboard_roundtrip() {
        let kb = InlineKeyboardMarkup {
            inline_keyboard: vec![vec![
                InlineKeyboardButton {
                    text: "A".into(),
                    callback_data: Some("a".into()),
                    url: None,
                },
                InlineKeyboardButton {
                    text: "B".into(),
                    callback_data: Some("b".into()),
                    url: None,
                },
            ]],
        };
        let json = serde_json::to_string(&kb).expect("serialize");
        let parsed: InlineKeyboardMarkup =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, kb);
    }

    #[test]
    fn test_parse_chat_id_number() {
        let v = serde_json::json!(42);
        assert_eq!(parse_chat_id(&v), 42);
    }

    #[test]
    fn test_parse_chat_id_string() {
        let v = serde_json::json!("123");
        assert_eq!(parse_chat_id(&v), 123);
    }
}
