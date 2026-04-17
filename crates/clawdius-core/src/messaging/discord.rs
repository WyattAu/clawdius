//! Discord webhook client for sending messages to Discord channels.
//!
//! Thin wrapper over the Discord webhook HTTP API using `reqwest`. Supports
//! sending messages (with automatic splitting at Discord's 2000-character
//! limit) to one or more webhook URLs.
//!
//! No external Discord crates are used — this keeps the dependency tree
//! lean and avoids version conflicts.

use serde::Serialize;
use std::time::Duration;

/// Maximum message length Discord allows (2000 characters).
const DISCORD_MAX_MSG_LEN: usize = 2000;

/// Default username shown on webhook messages.
const DEFAULT_USERNAME: &str = "Clawdius";

// ---------------------------------------------------------------------------
// Discord webhook API types
// ---------------------------------------------------------------------------

/// Payload for a Discord webhook `execute` request.
#[derive(Debug, Clone, Serialize)]
pub struct DiscordWebhookMessage {
    /// Message text (max 2000 characters per chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Override the webhook's default username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// Response from a Discord webhook execute request.
#[derive(Debug, serde::Deserialize)]
pub struct DiscordWebhookResponse {
    #[serde(default)]
    pub id: Option<String>,
    /// Non-empty when Discord returns an error.
    #[serde(default)]
    pub message: Option<String>,
    /// HTTP-style error code from Discord.
    #[serde(default)]
    pub code: Option<u16>,
}

// ---------------------------------------------------------------------------
// DiscordWebhookClient
// ---------------------------------------------------------------------------

/// Discord webhook client.
///
/// Holds one or more webhook URLs and an HTTP client. All methods are async
/// and return `Result` with descriptive error messages.
///
/// **Note:** This is a send-only client. It posts messages to Discord channels
/// via webhook URLs. Receiving messages from Discord would require a full
/// gateway websocket implementation, which is outside the scope of this
/// module.
#[derive(Debug, Clone)]
pub struct DiscordWebhookClient {
    client: reqwest::Client,
    webhook_urls: Vec<String>,
}

impl DiscordWebhookClient {
    /// Create a new Discord webhook client.
    ///
    /// # Panics
    /// Panics if `webhook_urls` is empty.
    #[must_use]
    pub fn new(webhook_urls: Vec<String>) -> Self {
        assert!(
            !webhook_urls.is_empty(),
            "Discord webhook URLs must not be empty"
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            webhook_urls,
        }
    }

    /// Create a client from a single webhook URL.
    #[must_use]
    pub fn from_url(url: impl Into<String>) -> Self {
        Self::new(vec![url.into()])
    }

    /// Send a message to all configured webhook channels.
    ///
    /// If the text exceeds Discord's 2000-character limit, it is
    /// automatically split into multiple messages at line boundaries.
    ///
    /// Returns `Ok(())` if all webhooks succeeded, or `Err` with a list of
    /// per-webhook errors.
    pub async fn send_message(&self, content: &str) -> Result<(), Vec<String>> {
        let chunks = split_message(content, DISCORD_MAX_MSG_LEN);
        let mut errors = Vec::new();

        for url in &self.webhook_urls {
            for chunk in &chunks {
                let body = DiscordWebhookMessage {
                    content: Some(chunk.clone()),
                    username: Some(DEFAULT_USERNAME.to_string()),
                };

                match self.client.post(url).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {},
                    Ok(resp) => {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        errors.push(format!("Webhook {url} returned {status}: {body_text}"));
                    },
                    Err(e) => {
                        errors.push(format!("Webhook {url} request failed: {e}"));
                    },
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Send a message to a single specific webhook URL.
    ///
    /// This is useful when you want to target a particular channel rather
    /// than broadcasting to all configured webhooks.
    pub async fn send_to(&self, webhook_url: &str, content: &str) -> crate::Result<()> {
        for chunk in split_message(content, DISCORD_MAX_MSG_LEN) {
            let body = DiscordWebhookMessage {
                content: Some(chunk),
                username: Some(DEFAULT_USERNAME.to_string()),
            };

            let resp = self
                .client
                .post(webhook_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| crate::Error::Llm(format!("Discord webhook request failed: {e}")))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                return Err(crate::Error::Llm(format!(
                    "Discord webhook returned {status}: {body_text}"
                )));
            }
        }
        Ok(())
    }

    /// Send a message with a custom username override.
    pub async fn send_message_as(&self, content: &str, username: &str) -> Result<(), Vec<String>> {
        let chunks = split_message(content, DISCORD_MAX_MSG_LEN);
        let mut errors = Vec::new();

        for url in &self.webhook_urls {
            for chunk in &chunks {
                let body = DiscordWebhookMessage {
                    content: Some(chunk.clone()),
                    username: Some(username.to_string()),
                };

                match self.client.post(url).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {},
                    Ok(resp) => {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        errors.push(format!("Webhook {url} returned {status}: {body_text}"));
                    },
                    Err(e) => {
                        errors.push(format!("Webhook {url} request failed: {e}"));
                    },
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Return the number of configured webhook URLs.
    #[must_use]
    pub fn webhook_count(&self) -> usize {
        self.webhook_urls.len()
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
        let chunks = split_message("hello", 2000);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_message_exact() {
        let text = "a".repeat(2000);
        let chunks = split_message(&text, 2000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2000);
    }

    #[test]
    fn test_split_message_long() {
        let text = "a".repeat(3000);
        let chunks = split_message(&text, 2000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2000);
        assert_eq!(chunks[1].len(), 1000);
    }

    #[test]
    fn test_split_message_at_newline() {
        let text = format!("{}\n{}", "a".repeat(1500), "b".repeat(1000));
        let chunks = split_message(&text, 2000);
        assert_eq!(chunks.len(), 2);
        // First chunk should break at the newline.
        assert!(chunks[0].ends_with('\n'));
        assert_eq!(chunks[0].len(), 1501); // 1500 'a' + '\n'
    }

    #[test]
    fn test_split_message_multiple_newlines() {
        let text = "line1\nline2\nline3\nline4\nline5";
        let chunks = split_message(text, 10);
        assert!(chunks.len() >= 3);
    }

    #[test]
    fn test_split_message_empty() {
        let chunks = split_message("", 2000);
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn test_discord_webhook_client_new_ok() {
        let client = DiscordWebhookClient::new(vec![
            "https://discord.com/api/webhooks/123/token".to_string()
        ]);
        assert_eq!(client.webhook_count(), 1);
    }

    #[test]
    fn test_discord_webhook_client_from_url() {
        let client = DiscordWebhookClient::from_url("https://discord.com/api/webhooks/123/token");
        assert_eq!(client.webhook_count(), 1);
    }

    #[test]
    fn test_discord_webhook_client_new_panics_on_empty() {
        let result = std::panic::catch_unwind(|| DiscordWebhookClient::new(vec![]));
        assert!(result.is_err());
    }

    #[test]
    fn test_discord_webhook_client_multiple_urls() {
        let client = DiscordWebhookClient::new(vec![
            "https://discord.com/api/webhooks/1/a".to_string(),
            "https://discord.com/api/webhooks/2/b".to_string(),
            "https://discord.com/api/webhooks/3/c".to_string(),
        ]);
        assert_eq!(client.webhook_count(), 3);
    }

    #[test]
    fn test_discord_webhook_client_clone() {
        let client = DiscordWebhookClient::from_url("https://discord.com/api/webhooks/1/a");
        let _client2 = client.clone();
    }

    #[test]
    fn test_webhook_message_serialize() {
        let msg = DiscordWebhookMessage {
            content: Some("hello".to_string()),
            username: Some("Clawdius".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("Clawdius"));
    }

    #[test]
    fn test_webhook_message_serialize_skip_none() {
        let msg = DiscordWebhookMessage {
            content: None,
            username: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_webhook_response_deserialize_success() {
        let json = r#"{"id": "1234567890"}"#;
        let resp: DiscordWebhookResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id.as_deref(), Some("1234567890"));
        assert!(resp.message.is_none());
        assert!(resp.code.is_none());
    }

    #[test]
    fn test_webhook_response_deserialize_error() {
        let json = r#"{"code": 50015, "message": "Invalid Webhook Token"}"#;
        let resp: DiscordWebhookResponse = serde_json::from_str(json).unwrap();
        assert!(resp.id.is_none());
        assert_eq!(resp.message.as_deref(), Some("Invalid Webhook Token"));
        assert_eq!(resp.code, Some(50015));
    }

    #[test]
    fn test_webhook_response_deserialize_empty() {
        let json = r#"{}"#;
        let resp: DiscordWebhookResponse = serde_json::from_str(json).unwrap();
        assert!(resp.id.is_none());
        assert!(resp.message.is_none());
        assert!(resp.code.is_none());
    }
}
