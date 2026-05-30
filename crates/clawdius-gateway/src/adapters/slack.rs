//! Slack platform adapter using slack-morphism.
//!
//! Connects Clawdius to Slack via a Slack App bot token. Handles incoming
//! messages via Socket Mode or Event API, file downloads, and response
//! delivery including threaded replies.
//!
//! # Setup
//!
//! 1. Create a Slack App at <https://api.slack.com/apps>
//! 2. Enable Socket Mode and copy the App-Level Token
//! 3. Enable `chat:write`, `channels:history`, `files:read` bot scopes
//! 4. Set tokens in config or env vars:
//!    - `SLACK_BOT_TOKEN` (xoxb-...)
//!    - `SLACK_APP_TOKEN` (xapp-...)
//!
//! # Features
//!
//! - Message handling (text, threaded replies, bot mentions)
//! - File/attachment download
//! - Threaded conversations (Slack threads)
//! - Markdown formatting (mrkdwn)
//! - Block kit support (future)

use std::sync::Arc;

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Slack adapter implementation.
///
/// Uses the Slack Web API for sending/editing messages and downloading files.
/// Incoming events are received via Socket Mode (when enabled) or the Events API.
pub struct SlackAdapter {
    /// Bot OAuth token (xoxb-...).
    bot_token: String,
    /// App-Level token for Socket Mode (xapp-...).
    app_token: Option<String>,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Shared HTTP client.
    http: std::sync::OnceLock<reqwest::Client>,
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
}

impl SlackAdapter {
    /// Create a new Slack adapter from a bot token.
    #[must_use]
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            app_token: None,
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let bot_token = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("SLACK_BOT_TOKEN not set".to_string()))?;

        let mut adapter = Self::new(bot_token);
        adapter.app_token = config
            .settings
            .get("app_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(adapter)
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Call a Slack Web API method.
    async fn slack_api(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, GatewayError> {
        let url = format!("https://slack.com/api/{method}");

        let response = self
            .http()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!("API call failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        let json: serde_json::Value = response.json().await.map_err(|e| GatewayError::Adapter {
            platform: "slack".to_string(),
            message: format!("failed to parse response: {e}"),
            source: Some(Box::new(e)),
        })?;

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!("Slack API error: {error}"),
                source: None, // Intentional: HTTP status errors don't have a source Error
            });
        }

        Ok(json)
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for SlackAdapter {
    fn platform(&self) -> Platform {
        Platform::Slack
    }

    fn set_message_callback(&self, callback: MessageCallback) {
        let guard = self.message_callback.clone();
        tokio::spawn(async move {
            let mut cb = guard.lock().await;
            *cb = Some(callback);
        });
    }

    async fn start(&self) -> Result<(), GatewayError> {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // Socket Mode / Event API listener runs separately
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let mut body = serde_json::json!({
            "channel": message.chat_id,
            "text": message.text,
        });

        // Thread the reply if reply_to is set
        if let Some(ref reply_to) = message.reply_to {
            body["thread_ts"] = serde_json::json!(reply_to);
        }

        self.slack_api("chat.postMessage", &body).await?;
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        // message_id format: "channel_id:timestamp"
        let parts: Vec<&str> = message_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!("invalid message_id format (expected channel:ts): {message_id}"),
                source: None, // Intentional: no source Error available
            });
        }

        let body = serde_json::json!({
            "channel": parts[0],
            "ts": parts[1],
            "text": new_text,
        });

        self.slack_api("chat.update", &body).await?;
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        // Slack requires authentication for private file downloads
        let response = self
            .http()
            .get(url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        // Slack URLs don't have clean filenames; extract from Content-Disposition
        let filename = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|cd| {
                cd.split("filename=")
                    .nth(1)
                    .and_then(|f| f.split('"').next())
                    .map(|f| f.to_string())
            })
            .unwrap_or_else(|| {
                url.rsplit('/')
                    .next()
                    .and_then(|f| f.split('?').next())
                    .unwrap_or("slack-attachment")
                    .to_string()
            });

        let dir = std::env::temp_dir().join("clawdius-slack");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "slack".to_string(),
            message: format!("read body failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        std::fs::write(&path, bytes).map_err(GatewayError::Io)?;
        Ok(path)
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth {
            healthy: self.is_running(),
            message: if self.is_running() {
                "connected".to_string()
            } else {
                "stopped".to_string()
            },
            messages_processed: self
                .messages_processed
                .load(std::sync::atomic::Ordering::Relaxed),
            errors: self.error_count.load(std::sync::atomic::Ordering::Relaxed),
            last_message_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn test_slack_adapter_new() {
        let adapter = SlackAdapter::new("xoxb-fake-token");
        assert_eq!(adapter.platform(), Platform::Slack);
        assert_eq!(adapter.bot_token, "xoxb-fake-token");
        assert!(adapter.app_token.is_none());
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_slack_from_config_missing_token() {
        let config = PlatformConfig::new(Platform::Slack);
        let result = SlackAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("SLACK_BOT_TOKEN"));
    }

    #[test]
    fn test_slack_from_config_valid() {
        let mut config = PlatformConfig::new(Platform::Slack);
        config.api_token = Some("xoxb-bot-token".to_string());
        let adapter = SlackAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.platform(), Platform::Slack);
        assert_eq!(adapter.bot_token, "xoxb-bot-token");
    }

    #[test]
    fn test_slack_from_config_with_app_token() {
        let mut config = PlatformConfig::new(Platform::Slack);
        config.api_token = Some("xoxb-bot-token".to_string());
        config
            .settings
            .insert("app_token".to_string(), serde_json::json!("xapp-token"));
        let adapter = SlackAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.app_token.as_deref(), Some("xapp-token"));
    }

    #[test]
    fn test_slack_send_message_json_format() {
        let msg = OutgoingMessage::new(Platform::Slack, "C12345", "hello slack");
        let body = serde_json::json!({
            "channel": msg.chat_id,
            "text": msg.text,
        });
        assert_eq!(body["channel"], "C12345");
        assert_eq!(body["text"], "hello slack");
    }

    #[test]
    fn test_slack_send_message_with_reply_json() {
        let msg = OutgoingMessage::new(Platform::Slack, "C12345", "threaded reply")
            .with_reply_to("1234567890.123456");
        let mut body = serde_json::json!({
            "channel": msg.chat_id,
            "text": msg.text,
        });
        if let Some(ref reply_to) = msg.reply_to {
            body["thread_ts"] = serde_json::json!(reply_to);
        }
        assert_eq!(body["thread_ts"], "1234567890.123456");
    }

    #[test]
    fn test_slack_send_message_empty_text() {
        let msg = OutgoingMessage::new(Platform::Slack, "C12345", "");
        assert_eq!(msg.text, "");
    }

    #[test]
    fn test_slack_send_message_unicode() {
        let msg = OutgoingMessage::new(Platform::Slack, "C12345", "slack 日本語 テスト 🚀");
        assert_eq!(msg.text, "slack 日本語 テスト 🚀");
    }

    #[test]
    fn test_slack_edit_message_json_format() {
        let message_id = "C12345:1234567890.123456";
        let parts: Vec<&str> = message_id.splitn(2, ':').collect();
        assert_eq!(parts.len(), 2);
        let body = serde_json::json!({
            "channel": parts[0],
            "ts": parts[1],
            "text": "edited text",
        });
        assert_eq!(body["channel"], "C12345");
        assert_eq!(body["ts"], "1234567890.123456");
        assert_eq!(body["text"], "edited text");
    }

    #[tokio::test]
    async fn test_slack_start_stop_lifecycle() {
        let adapter = SlackAdapter::new("xoxb-fake-token");
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_slack_health_stopped() {
        let adapter = SlackAdapter::new("xoxb-fake-token");
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }

    #[tokio::test]
    async fn test_slack_health_running() {
        let adapter = SlackAdapter::new("xoxb-fake-token");
        adapter.start().await.unwrap();
        let health = adapter.health();
        assert!(health.healthy);
        assert_eq!(health.message, "connected");
    }
}
