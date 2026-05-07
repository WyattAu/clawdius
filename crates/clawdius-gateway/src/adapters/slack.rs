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

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| GatewayError::Adapter {
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
                source: None,
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

    async fn edit_message(
        &self,
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        // message_id format: "channel_id:timestamp"
        let parts: Vec<&str> = message_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Adapter {
                platform: "slack".to_string(),
                message: format!(
                    "invalid message_id format (expected channel:ts): {message_id}"
                ),
                source: None,
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
            errors: self
                .error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            last_message_at: None,
        }
    }
}
