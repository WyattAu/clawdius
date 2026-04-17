//! Telegram Bot API client.
//!
//! Thin wrapper over the Telegram Bot HTTP API using `reqwest`. Supports
//! long polling (`getUpdates`), sending messages (`sendMessage`), and
//! webhook management (`setWebhook` / `deleteWebhook`).
//!
//! No external Telegram crates are used — this keeps the dependency tree
//! lean and avoids version conflicts.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Telegram Bot API base URL.
const TELEGRAM_API: &str = "https://api.telegram.org/bot";

/// Long polling timeout in seconds (server will wait up to this long
/// before returning empty results).
const POLL_TIMEOUT_SECS: u32 = 30;

/// Maximum message length Telegram allows (4096 characters).
const TELEGRAM_MAX_MSG_LEN: usize = 4096;

// ---------------------------------------------------------------------------
// Telegram API types
// ---------------------------------------------------------------------------

/// An incoming Telegram update.
#[derive(Debug, Clone, Deserialize)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
}

/// A Telegram message.
#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub message_id: i64,
    #[serde(default)]
    pub from: Option<User>,
    /// Unique chat identifier.
    pub chat: Chat,
    /// Date the message was sent (Unix timestamp).
    pub date: i64,
    /// Text of the message (only for text messages).
    #[serde(default)]
    pub text: Option<String>,
}

/// A Telegram user.
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub username: Option<String>,
}

/// A Telegram chat.
#[derive(Debug, Clone, Deserialize)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// Response from `getUpdates`.
#[derive(Debug, Deserialize)]
pub struct GetUpdatesResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub result: Vec<Update>,
}

/// Response from `sendMessage`.
#[derive(Debug, Deserialize)]
pub struct SendMessageResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub description: Option<String>,
}

/// Parameters for `sendMessage`.
#[derive(Debug, Serialize)]
struct SendMessageParams {
    pub chat_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// TelegramBot client
// ---------------------------------------------------------------------------

/// Telegram Bot API client.
///
/// Holds the bot token and an HTTP client. All methods are async and return
/// `Result` with descriptive error messages.
#[derive(Debug, Clone)]
pub struct TelegramBot {
    token: String,
    client: reqwest::Client,
}

impl TelegramBot {
    /// Create a new Telegram bot client.
    ///
    /// # Panics
    /// Panics if the token is empty.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        let token = token.into();
        assert!(!token.is_empty(), "Telegram bot token must not be empty");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self { token, client }
    }

    fn base_url(&self) -> String {
        format!("{TELEGRAM_API}{}", self.token)
    }

    /// Get the base URL (for use by router).
    #[must_use]
    pub fn api_base(&self) -> String {
        self.base_url()
    }

    /// Send a chat action (e.g., "typing").
    pub async fn send_chat_action(&self, chat_id: i64, action: &str) -> crate::Result<()> {
        self.client
            .post(format!("{}/sendChatAction", self.base_url()))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "action": action
            }))
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Telegram sendChatAction failed: {e}")))?;
        Ok(())
    }

    /// Fetch updates via long polling.
    ///
    /// Blocks for up to `POLL_TIMEOUT_SECS` waiting for new messages.
    /// Pass the last `offset` to only receive updates after that ID.
    pub async fn get_updates(&self, offset: Option<i64>) -> crate::Result<Vec<Update>> {
        let mut url = format!("{}/getUpdates?timeout={POLL_TIMEOUT_SECS}", self.base_url());
        if let Some(off) = offset {
            url.push_str(&format!("&offset={}", off + 1));
        }
        url.push_str("&allowed_updates=[\"message\"]");

        let resp: GetUpdatesResponse = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Telegram API request failed: {e}")))?
            .json()
            .await
            .map_err(|e| crate::Error::Llm(format!("Failed to parse Telegram response: {e}")))?;

        if !resp.ok {
            return Err(crate::Error::Llm(
                "Telegram API returned ok=false".to_string(),
            ));
        }

        Ok(resp.result)
    }

    /// Send a text message to a chat.
    ///
    /// If the text exceeds Telegram's 4096-character limit, it is
    /// automatically split into multiple messages.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> crate::Result<()> {
        // Split long messages at line boundaries when possible.
        for chunk in split_message(text, TELEGRAM_MAX_MSG_LEN) {
            let params = SendMessageParams {
                chat_id,
                text: chunk,
                parse_mode: Some("Markdown"),
                reply_to_message_id: None,
            };

            let resp: SendMessageResponse = self
                .client
                .post(format!("{}/sendMessage", self.base_url()))
                .json(&params)
                .send()
                .await
                .map_err(|e| crate::Error::Llm(format!("Telegram sendMessage failed: {e}")))?
                .json()
                .await
                .map_err(|e| {
                    crate::Error::Llm(format!("Failed to parse sendMessage response: {e}"))
                })?;

            if !resp.ok {
                let desc = resp.description.unwrap_or_default();
                return Err(crate::Error::Llm(format!(
                    "Telegram sendMessage failed: {desc}"
                )));
            }
        }
        Ok(())
    }

    /// Send a text message replying to a specific message.
    pub async fn send_reply(&self, chat_id: i64, reply_to: i64, text: &str) -> crate::Result<()> {
        for chunk in split_message(text, TELEGRAM_MAX_MSG_LEN) {
            let params = SendMessageParams {
                chat_id,
                text: chunk,
                parse_mode: Some("Markdown"),
                reply_to_message_id: Some(reply_to),
            };

            let resp: SendMessageResponse = self
                .client
                .post(format!("{}/sendMessage", self.base_url()))
                .json(&params)
                .send()
                .await
                .map_err(|e| crate::Error::Llm(format!("Telegram sendMessage failed: {e}")))?
                .json()
                .await
                .map_err(|e| {
                    crate::Error::Llm(format!("Failed to parse sendMessage response: {e}"))
                })?;

            if !resp.ok {
                let desc = resp.description.unwrap_or_default();
                return Err(crate::Error::Llm(format!(
                    "Telegram sendMessage failed: {desc}"
                )));
            }
        }
        Ok(())
    }

    /// Register a webhook URL with Telegram.
    pub async fn set_webhook(&self, url: &str) -> crate::Result<()> {
        let resp: SendMessageResponse = self
            .client
            .post(format!(
                "{}/setWebhook?url={}&allowed_updates=[\"message\"]",
                self.base_url(),
                url
            ))
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Telegram setWebhook failed: {e}")))?
            .json()
            .await
            .map_err(|e| crate::Error::Llm(format!("Failed to parse setWebhook response: {e}")))?;

        if !resp.ok {
            let desc = resp.description.unwrap_or_default();
            return Err(crate::Error::Llm(format!(
                "Telegram setWebhook failed: {desc}"
            )));
        }
        Ok(())
    }

    /// Delete the registered webhook and switch back to long polling.
    pub async fn delete_webhook(&self) -> crate::Result<()> {
        let resp: SendMessageResponse = self
            .client
            .post(format!("{}/deleteWebhook", self.base_url()))
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Telegram deleteWebhook failed: {e}")))?
            .json()
            .await
            .map_err(|e| {
                crate::Error::Llm(format!("Failed to parse deleteWebhook response: {e}"))
            })?;

        if !resp.ok {
            let desc = resp.description.unwrap_or_default();
            return Err(crate::Error::Llm(format!(
                "Telegram deleteWebhook failed: {desc}"
            )));
        }
        Ok(())
    }

    /// Get the bot's own user info.
    pub async fn get_me(&self) -> crate::Result<User> {
        #[derive(Deserialize)]
        struct GetMeResponse {
            ok: bool,
            result: Option<User>,
        }

        let resp: GetMeResponse = self
            .client
            .get(format!("{}/getMe", self.base_url()))
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Telegram getMe failed: {e}")))?
            .json()
            .await
            .map_err(|e| crate::Error::Llm(format!("Failed to parse getMe response: {e}")))?;

        resp.result
            .ok_or_else(|| crate::Error::Llm("Telegram getMe returned no result".to_string()))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split text into chunks that fit within `max_len`, preferring to break at
/// newlines when possible.
fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to find a newline within the limit.
        let split_at = remaining[..max_len]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(max_len);

        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }

    chunks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_message_short() {
        let chunks = split_message("hello", 4096);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_message_exact() {
        let text = "a".repeat(4096);
        let chunks = split_message(&text, 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 4096);
    }

    #[test]
    fn test_split_message_long() {
        let text = "a".repeat(5000);
        let chunks = split_message(&text, 4096);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 4096);
        assert_eq!(chunks[1].len(), 904);
    }

    #[test]
    fn test_split_message_at_newline() {
        let text = format!("{}\n{}", "a".repeat(4000), "b".repeat(2000));
        let chunks = split_message(&text, 4096);
        assert_eq!(chunks.len(), 2);
        // First chunk should break at the newline.
        assert!(chunks[0].ends_with('\n'));
        assert_eq!(chunks[0].len(), 4001); // 4000 'a' + '\n'
    }

    #[test]
    fn test_split_message_multiple_newlines() {
        let text = "line1\nline2\nline3\nline4\nline5";
        let chunks = split_message(text, 10);
        // With limit 10, we should split at newlines when possible.
        assert!(chunks.len() >= 3);
    }

    #[test]
    fn test_telegram_bot_new_panics_on_empty() {
        let result = std::panic::catch_unwind(|| TelegramBot::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_telegram_bot_new_ok() {
        let bot = TelegramBot::new("123:abc");
        assert!(bot.base_url().contains("123:abc"));
    }

    #[test]
    fn test_telegram_bot_clone() {
        let bot = TelegramBot::new("123:abc");
        let _bot2 = bot.clone();
    }

    #[test]
    fn test_update_deserialize() {
        let json = r#"{
            "update_id": 1,
            "message": {
                "message_id": 42,
                "from": {"id": 100, "is_bot": false, "first_name": "Test"},
                "chat": {"id": 200, "type": "private"},
                "date": 1700000000,
                "text": "hello"
            }
        }"#;
        let update: Update = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 1);
        assert!(update.message.is_some());
        let msg = update.message.unwrap();
        assert_eq!(msg.message_id, 42);
        assert_eq!(msg.text.as_deref(), Some("hello"));
        assert_eq!(msg.chat.id, 200);
        assert_eq!(msg.from.as_ref().unwrap().first_name, "Test");
    }

    #[test]
    fn test_update_deserialize_no_message() {
        let json = r#"{"update_id": 2}"#;
        let update: Update = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 2);
        assert!(update.message.is_none());
    }

    #[test]
    fn test_get_updates_response_deserialize() {
        let json = r#"{"ok": true, "result": []}"#;
        let resp: GetUpdatesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.ok);
        assert!(resp.result.is_empty());
    }
}
