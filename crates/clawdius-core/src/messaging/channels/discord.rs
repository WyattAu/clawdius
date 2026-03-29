//! Discord Channel Implementation
//!
//! Provides integration with Discord API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::messaging::oauth::OAuthTokenStore;
use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct CreateMessageRequest<'a> {
    content: &'a str,
    tts: bool,
}

#[derive(Serialize)]
struct EditMessageRequest<'a> {
    content: &'a str,
}

#[derive(Deserialize)]
struct DiscordMessage {
    id: String,
    #[allow(dead_code)]
    channel_id: String,
}

#[derive(Deserialize)]
struct DiscordError {
    message: String,
}

/// Discord channel adapter using Bot API
pub struct DiscordChannel {
    bot_token: String,
    client: Client,
    base_url: Option<String>,
    token_store: Option<Arc<std::sync::Mutex<OAuthTokenStore>>>,
}

impl DiscordChannel {
    /// Creates a new Discord channel
    ///
    /// # Arguments
    /// * `bot_token` - The Discord bot token
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: None,
            token_store: None,
        }
    }

    fn api_base(&self) -> String {
        self.base_url
            .as_deref()
            .unwrap_or("https://discord.com/api/v10")
            .to_string()
    }

    pub fn with_base_url(base_url: impl Into<String>, bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            client: Client::new(),
            base_url: Some(base_url.into()),
            token_store: None,
        }
    }

    pub fn with_token_store(mut self, store: Arc<std::sync::Mutex<OAuthTokenStore>>) -> Self {
        self.token_store = Some(store);
        self
    }

    fn current_token(&self) -> String {
        if let Some(store) = &self.token_store {
            if let Ok(guard) = store.lock() {
                if let Ok(Some(token)) = guard.get_bot_token(&Platform::Discord) {
                    if !token.is_expired(0) {
                        return token.bot_token;
                    }
                }
            }
        }
        self.bot_token.clone()
    }
}

#[async_trait]
impl MessagingChannel for DiscordChannel {
    fn platform(&self) -> Platform {
        Platform::Discord
    }

    async fn send_message(&self, channel_id: &str, text: &str) -> Result<String> {
        let max_len = Platform::Discord.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let url = format!("{}/channels/{}/messages", self.api_base(), channel_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.current_token()))
            .header("Content-Type", "application/json")
            .json(&CreateMessageRequest {
                content: text,
                tts: false,
            })
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if status.is_success() {
            let result: DiscordMessage = serde_json::from_str(&body)
                .map_err(|e| MessagingError::ParseError(e.to_string()))?;
            Ok(result.id)
        } else {
            let error: DiscordError = serde_json::from_str(&body).unwrap_or(DiscordError {
                message: "Unknown error".to_string(),
            });
            Err(MessagingError::SendFailed(error.message))
        }
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let url = format!(
            "{}/channels/{}/messages/{}",
            self.api_base(),
            channel_id,
            message_id
        );

        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bot {}", self.current_token()))
            .header("Content-Type", "application/json")
            .json(&EditMessageRequest { content: new_text })
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if status.is_success() {
            let result: DiscordMessage = serde_json::from_str(&body)
                .map_err(|e| MessagingError::ParseError(e.to_string()))?;
            Ok(result.id)
        } else {
            let error: DiscordError = serde_json::from_str(&body).unwrap_or(DiscordError {
                message: "Unknown error".to_string(),
            });
            Err(MessagingError::SendFailed(error.message))
        }
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self
            .client
            .get(format!("{}/users/@me", self.api_base()))
            .header("Authorization", format!("Bot {}", self.current_token()))
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
        let channel = DiscordChannel::with_base_url("https://example.com", "test-token");
        assert_eq!(channel.platform(), Platform::Discord);
    }

    #[tokio::test]
    async fn test_discord_channel_uses_token_store() {
        use crate::messaging::oauth::{OAuthToken, OAuthTokenStore};
        use crate::messaging::types::Platform;
        use std::sync::Arc;

        let store = Arc::new(std::sync::Mutex::new(OAuthTokenStore::new()));

        store
            .lock()
            .unwrap()
            .store_bot_token(
                &Platform::Discord,
                OAuthToken {
                    bot_token: "discord-store-token".to_string(),
                    user_token: None,
                    refresh_token: None,
                    expires_at: None,
                    scopes: vec![],
                    extra: Default::default(),
                },
            )
            .unwrap();

        let channel = DiscordChannel::new("discord-static").with_token_store(store);

        assert_eq!(channel.current_token(), "discord-store-token");
    }

    #[tokio::test]
    async fn test_discord_channel_falls_back_to_static() {
        let channel = DiscordChannel::new("discord-static");
        assert_eq!(channel.current_token(), "discord-static");
    }

    #[tokio::test]
    async fn test_discord_channel_expired_token_falls_back() {
        use crate::messaging::oauth::{OAuthToken, OAuthTokenStore};
        use crate::messaging::types::Platform;
        use std::sync::Arc;
        use std::time::{SystemTime, UNIX_EPOCH};

        let store = Arc::new(std::sync::Mutex::new(OAuthTokenStore::new()));

        let past = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1000;
        store
            .lock()
            .unwrap()
            .store_bot_token(
                &Platform::Discord,
                OAuthToken {
                    bot_token: "discord-expired".to_string(),
                    user_token: None,
                    refresh_token: None,
                    expires_at: Some(past),
                    scopes: vec![],
                    extra: Default::default(),
                },
            )
            .unwrap();

        let channel = DiscordChannel::new("discord-fallback").with_token_store(store);

        assert_eq!(channel.current_token(), "discord-fallback");
    }
}
