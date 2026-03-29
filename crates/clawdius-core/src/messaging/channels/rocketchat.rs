//! RocketChat Channel Implementation
//!
//! Provides integration with Rocket.Chat API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    channel: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct EditMessageRequest<'a> {
    room_id: &'a str,
    msg_id: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct MessageResponse {
    #[allow(dead_code)]
    ts: String,
    #[serde(rename = "_id")]
    id: String,
}

#[derive(Deserialize)]
struct RocketChatResponse<T> {
    success: bool,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<String>,
    result: Option<T>,
}

/// Rocket.Chat channel adapter
pub struct RocketChatChannel {
    server_url: String,
    user_id: String,
    auth_token: String,
    client: Client,
}

impl RocketChatChannel {
    /// Creates a new Rocket.Chat channel
    ///
    /// # Arguments
    /// * `server_url` - The Rocket.Chat server URL (e.g., "https://chat.example.com")
    /// * `user_id` - The user ID for authentication
    /// * `auth_token` - The personal access token or auth token
    pub fn new(
        server_url: impl Into<String>,
        user_id: impl Into<String>,
        auth_token: impl Into<String>,
    ) -> Self {
        Self {
            server_url: server_url.into(),
            user_id: user_id.into(),
            auth_token: auth_token.into(),
            client: Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/api/v1/{}", self.server_url.trim_end_matches('/'), path)
    }
}

#[async_trait]
impl MessagingChannel for RocketChatChannel {
    fn platform(&self) -> Platform {
        Platform::RocketChat
    }

    async fn send_message(&self, channel: &str, text: &str) -> Result<String> {
        let max_len = Platform::RocketChat.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .post(self.api_url("chat.postMessage"))
            .header("X-User-Id", &self.user_id)
            .header("X-Auth-Token", &self.auth_token)
            .header("Content-Type", "application/json")
            .json(&SendMessageRequest {
                channel,
                text,
                attachments: None,
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

        let result: RocketChatResponse<MessageResponse> =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.success {
            result
                .result
                .map(|r| r.id)
                .ok_or_else(|| MessagingError::SendFailed("No message ID returned".to_string()))
        } else {
            Err(MessagingError::SendFailed(
                result
                    .error
                    .or(result.message)
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn edit_message(
        &self,
        room_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let response = self
            .client
            .post(self.api_url("chat.update"))
            .header("X-User-Id", &self.user_id)
            .header("X-Auth-Token", &self.auth_token)
            .header("Content-Type", "application/json")
            .json(&EditMessageRequest {
                room_id,
                msg_id: message_id,
                text: new_text,
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

        let result: RocketChatResponse<MessageResponse> =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.success {
            result
                .result
                .map(|r| r.id)
                .ok_or_else(|| MessagingError::SendFailed("No message ID returned".to_string()))
        } else {
            Err(MessagingError::SendFailed(
                result
                    .error
                    .or(result.message)
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self
            .client
            .get(self.api_url("me"))
            .header("X-User-Id", &self.user_id)
            .header("X-Auth-Token", &self.auth_token)
            .send()
            .await;

        response.map(|r| r.status().is_success()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform() {
        let channel = RocketChatChannel::new("https://chat.example.com", "user-123", "token");
        assert_eq!(channel.platform(), Platform::RocketChat);
    }

    #[test]
    fn test_api_url() {
        let channel = RocketChatChannel::new("https://chat.example.com", "user-123", "token");
        let url = channel.api_url("chat.postMessage");
        assert_eq!(url, "https://chat.example.com/api/v1/chat.postMessage");
    }
}
