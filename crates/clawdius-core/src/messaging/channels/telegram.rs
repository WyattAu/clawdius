//! Telegram Channel Implementation
//!
//! Provides integration with Telegram Bot API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    chat_id: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
}

#[derive(Serialize)]
struct EditMessageRequest<'a> {
    chat_id: &'a str,
    message_id: i64,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
}

#[derive(Deserialize)]
struct MessageResponse {
    message_id: i64,
}

#[derive(Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: T,
    #[serde(default)]
    description: Option<String>,
}

pub struct TelegramChannel {
    bot_token: String,
    client: Client,
    base_url: Option<String>,
}

impl TelegramChannel {
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: None,
        }
    }

    fn api_url(&self, method: &str) -> String {
        let base = self
            .base_url
            .as_deref()
            .unwrap_or("https://api.telegram.org");
        format!("{}/bot{}/{}", base, self.bot_token, method)
    }

    pub fn with_base_url(base_url: impl Into<String>, bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: Some(base_url.into()),
        }
    }
}

#[async_trait]
impl MessagingChannel for TelegramChannel {
    fn platform(&self) -> Platform {
        Platform::Telegram
    }

    async fn send_message(&self, chat_id: &str, text: &str) -> Result<String> {
        let max_len = Platform::Telegram.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .post(self.api_url("sendMessage"))
            .json(&SendMessageRequest {
                chat_id,
                text,
                parse_mode: Some("Markdown"),
            })
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(MessagingError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: TelegramResponse<MessageResponse> =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.ok {
            Ok(result.result.message_id.to_string())
        } else {
            Err(MessagingError::SendFailed(
                result
                    .description
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let msg_id: i64 = message_id
            .parse()
            .map_err(|e| MessagingError::SendFailed(format!("Invalid message ID: {}", e)))?;

        let response = self
            .client
            .post(self.api_url("editMessageText"))
            .json(&EditMessageRequest {
                chat_id,
                message_id: msg_id,
                text: new_text,
                parse_mode: Some("Markdown"),
            })
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(MessagingError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: TelegramResponse<MessageResponse> =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.ok {
            Ok(result.result.message_id.to_string())
        } else {
            Err(MessagingError::SendFailed(
                result
                    .description
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self.client.get(self.api_url("getMe")).send().await;

        response.map(|r| r.status().is_success()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform() {
        let channel = TelegramChannel::with_base_url("https://example.com", "test-token");
        assert_eq!(channel.platform(), Platform::Telegram);
    }

    #[test]
    fn test_api_url() {
        let channel = TelegramChannel::with_base_url("https://example.com", "123:ABC");
        let url = channel.api_url("sendMessage");
        assert!(url.contains("example.com"));
        assert!(url.contains("123:ABC"));
        assert!(url.contains("sendMessage"));
    }
}
