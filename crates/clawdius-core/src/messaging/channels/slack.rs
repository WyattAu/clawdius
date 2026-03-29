//! Slack Channel Implementation
//!
//! Provides integration with Slack API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::messaging::oauth::OAuthTokenStore;
use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    channel: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mrkdwn: Option<bool>,
}

#[derive(Serialize)]
struct EditMessageRequest<'a> {
    channel: &'a str,
    ts: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct MessageResponse {
    ok: bool,
    ts: Option<String>,
    #[allow(dead_code)]
    message: Option<MessageInfo>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct MessageInfo {
    #[allow(dead_code)]
    text: String,
    #[allow(dead_code)]
    user: String,
}

/// Slack channel adapter using Web API
pub struct SlackChannel {
    bot_token: String,
    client: Client,
    base_url: Option<String>,
    token_store: Option<Arc<OAuthTokenStore>>,
}

impl SlackChannel {
    /// Creates a new Slack channel
    ///
    /// # Arguments
    /// * `bot_token` - The Slack bot token (xoxb-...)
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: None,
            token_store: None,
        }
    }

    fn api_url(&self, method: &str) -> String {
        let base = self.base_url.as_deref().unwrap_or("https://slack.com/api");
        format!("{}/{}", base, method)
    }

    pub fn with_base_url(base_url: impl Into<String>, bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: Some(base_url.into()),
            token_store: None,
        }
    }

    pub fn with_token_store(mut self, store: Arc<OAuthTokenStore>) -> Self {
        self.token_store = Some(store);
        self
    }

    fn current_token(&self) -> String {
        if let Some(store) = &self.token_store {
            if let Ok(Some(token)) = store.get_bot_token(&Platform::Slack) {
                if !token.is_expired(0) {
                    return token.bot_token;
                }
            }
        }
        self.bot_token.clone()
    }
}

#[async_trait]
impl MessagingChannel for SlackChannel {
    fn platform(&self) -> Platform {
        Platform::Slack
    }

    async fn send_message(&self, channel: &str, text: &str) -> Result<String> {
        let max_len = Platform::Slack.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .post(self.api_url("chat.postMessage"))
            .header("Authorization", format!("Bearer {}", self.current_token()))
            .header("Content-Type", "application/json")
            .json(&SendMessageRequest {
                channel,
                text,
                thread_ts: None,
                blocks: None,
                mrkdwn: Some(true),
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

        let result: MessageResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.ok {
            result
                .ts
                .ok_or_else(|| MessagingError::SendFailed("No timestamp returned".to_string()))
        } else {
            Err(MessagingError::SendFailed(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn edit_message(
        &self,
        channel: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let response = self
            .client
            .post(self.api_url("chat.update"))
            .header("Authorization", format!("Bearer {}", self.current_token()))
            .header("Content-Type", "application/json")
            .json(&EditMessageRequest {
                channel,
                ts: message_id,
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

        let result: MessageResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        if result.ok {
            result
                .ts
                .ok_or_else(|| MessagingError::SendFailed("No timestamp returned".to_string()))
        } else {
            Err(MessagingError::SendFailed(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self
            .client
            .post(self.api_url("auth.test"))
            .header("Authorization", format!("Bearer {}", self.current_token()))
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
        let channel = SlackChannel::with_base_url("https://example.com", "xoxb-test-token");
        assert_eq!(channel.platform(), Platform::Slack);
    }

    #[test]
    fn test_api_url() {
        let channel = SlackChannel::with_base_url("https://example.com", "xoxb-test-token");
        let url = channel.api_url("chat.postMessage");
        assert_eq!(url, "https://example.com/chat.postMessage");
    }

    #[tokio::test]
    async fn test_slack_channel_uses_token_store() {
        use crate::messaging::oauth::{OAuthToken, OAuthTokenStore};
        use crate::messaging::types::Platform;
        use std::sync::Arc;

        let store = Arc::new(OAuthTokenStore::new());

        store
            .store_bot_token(
                &Platform::Slack,
                OAuthToken {
                    bot_token: "xoxb-store-token".to_string(),
                    user_token: None,
                    refresh_token: None,
                    expires_at: None,
                    scopes: vec![],
                    extra: Default::default(),
                },
            )
            .unwrap();

        let channel = SlackChannel::new("xoxb-static-token").with_token_store(store);

        assert_eq!(channel.current_token(), "xoxb-store-token");
    }

    #[tokio::test]
    async fn test_slack_channel_falls_back_to_static_token() {
        let channel = SlackChannel::new("xoxb-static-token");
        assert_eq!(channel.current_token(), "xoxb-static-token");
    }

    #[tokio::test]
    async fn test_slack_channel_expired_token_falls_back() {
        use crate::messaging::oauth::{OAuthToken, OAuthTokenStore};
        use crate::messaging::types::Platform;
        use std::sync::Arc;
        use std::time::{SystemTime, UNIX_EPOCH};

        let store = Arc::new(OAuthTokenStore::new());

        let past = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1000;
        store
            .store_bot_token(
                &Platform::Slack,
                OAuthToken {
                    bot_token: "xoxb-expired".to_string(),
                    user_token: None,
                    refresh_token: None,
                    expires_at: Some(past),
                    scopes: vec![],
                    extra: Default::default(),
                },
            )
            .unwrap();

        let channel = SlackChannel::new("xoxb-fallback").with_token_store(store);

        assert_eq!(channel.current_token(), "xoxb-fallback");
    }
}
