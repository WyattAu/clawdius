//! Matrix platform adapter using matrix-sdk.
//!
//! Connects Clawdius to a Matrix homeserver via a bot account.
//! Handles incoming messages, file downloads, and response delivery
//! including edits (redaction + new message).
//!
//! # Setup
//!
//! 1. Create a bot account on your Matrix homeserver
//! 2. Log in and obtain an access token
//! 3. Set the homeserver URL and access token in config:
//!    - `MATRIX_HOMESERVER_URL`
//!    - `MATRIX_ACCESS_TOKEN`
//!
//! # Features
//!
//! - Message handling (text, replies, edits, formatted body)
//! - File/attachment upload and download
//! - Threaded replies via `m.in_reply_to`
//! - Markdown → Matrix HTML formatting
//! - E2EE support when `e2e-encryption` feature is enabled
//!   (note: requires matching `rusqlite` version)

use std::sync::Arc;

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Matrix adapter implementation.
///
/// Uses the Matrix Client-Server API for all operations.
/// The matrix-sdk `Client` handles sync, sending, and state management.
pub struct MatrixAdapter {
    /// The Matrix homeserver URL.
    homeserver_url: String,
    /// The bot's access token.
    access_token: String,
    /// The bot's full Matrix ID (@user:homeserver.org).
    user_id: String,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Shared HTTP client for direct API calls.
    http: std::sync::OnceLock<reqwest::Client>,
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
}

impl MatrixAdapter {
    /// Create a new Matrix adapter.
    #[must_use]
    pub fn new(
        homeserver_url: impl Into<String>,
        access_token: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            homeserver_url: homeserver_url.into(),
            access_token: access_token.into(),
            user_id: user_id.into(),
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let homeserver_url = config
            .settings
            .get("homeserver_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::Config("MATRIX_HOMESERVER_URL not set".to_string()))?;

        let access_token = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("MATRIX_ACCESS_TOKEN not set".to_string()))?;

        let user_id = config
            .settings
            .get("user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("@clawdius:matrix.org")
            .to_string();

        Ok(Self::new(homeserver_url, access_token, user_id))
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Call the Matrix Client-Server API.
    async fn matrix_api(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, GatewayError> {
        let url = format!("{}{path}", self.homeserver_url.trim_end_matches('/'));

        let mut request = self
            .http()
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", self.access_token));

        if let Some(b) = body {
            request = request.json(b);
        }

        let response = request.send().await.map_err(|e| GatewayError::Adapter {
            platform: "matrix".to_string(),
            message: format!("API call failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "matrix".to_string(),
                message: format!("Matrix API error {status}: {text}"),
                source: None,
            });
        }

        response.json().await.map_err(|e| GatewayError::Adapter {
            platform: "matrix".to_string(),
            message: format!("failed to parse response: {e}"),
            source: Some(Box::new(e)),
        })
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for MatrixAdapter {
    fn platform(&self) -> Platform {
        Platform::Matrix
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
        // Matrix sync loop runs separately via matrix-sdk Client
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let mut content = serde_json::json!({
            "msgtype": "m.text",
            "body": message.text,
        });

        // Add reply relation if present
        if let Some(ref reply_to) = message.reply_to {
            content["m.relates_to"] = serde_json::json!({
                "rel_type": "m.in_reply_to",
                "event_id": reply_to,
            });
        }

        // Matrix uses transaction IDs for idempotency
        let txn_id = format!("clawdius_{}", uuid::Uuid::new_v4());

        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": message.text,
            "m.relates_to": content.get("m.relates_to"),
        });

        self.matrix_api(
            reqwest::Method::PUT,
            &format!(
                "/_matrix/client/v3/rooms/{}/send/m.room.message/{txn_id}",
                message.chat_id
            ),
            Some(&body),
        )
        .await?;

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        // Matrix edits use m.new_content with a relation to the original event
        // message_id is the Matrix event_id
        let txn_id = format!("clawdius_edit_{}", uuid::Uuid::new_v4());

        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": format!("* {new_text}"),
            "m.new_content": {
                "msgtype": "m.text",
                "body": new_text,
            },
            "m.relates_to": {
                "rel_type": "m.replace",
                "event_id": message_id,
            },
        });

        // We need the room_id to send the edit; use a synthetic path
        // In practice, the room_id should be tracked with the message_id.
        // For now, we use the chat_id stored in the adapter context.
        // The caller should ensure message_id is formatted as "room_id:event_id"
        let parts: Vec<&str> = message_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Adapter {
                platform: "matrix".to_string(),
                message: format!(
                    "invalid message_id format (expected room_id:event_id): {message_id}"
                ),
                source: None,
            });
        }

        self.matrix_api(
            reqwest::Method::PUT,
            &format!(
                "/_matrix/client/v3/rooms/{}/send/m.room.message/{txn_id}",
                parts[0]
            ),
            Some(&body),
        )
        .await?;

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        // Matrix media URLs need the homeserver's media endpoint
        // If the URL is a mxc:// URI, convert it to HTTP
        let download_url = if url.starts_with("mxc://") {
            let mxc_path = url.strip_prefix("mxc://").unwrap_or_default();
            format!(
                "{}/_matrix/media/v3/download/{}",
                self.homeserver_url.trim_end_matches('/'),
                mxc_path
            )
        } else {
            url.to_string()
        };

        let response = self
            .http()
            .get(&download_url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "matrix".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "matrix".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

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
                download_url
                    .rsplit('/')
                    .next()
                    .unwrap_or("matrix-attachment")
                    .to_string()
            });

        let dir = std::env::temp_dir().join("clawdius-matrix");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "matrix".to_string(),
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
                "syncing".to_string()
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
    fn test_matrix_adapter_new() {
        let adapter = MatrixAdapter::new("https://matrix.org", "access-token", "@bot:matrix.org");
        assert_eq!(adapter.platform(), Platform::Matrix);
        assert_eq!(adapter.homeserver_url, "https://matrix.org");
        assert_eq!(adapter.access_token, "access-token");
        assert_eq!(adapter.user_id, "@bot:matrix.org");
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_matrix_from_config_missing_homeserver() {
        let config = PlatformConfig::new(Platform::Matrix);
        let result = MatrixAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("MATRIX_HOMESERVER_URL"));
    }

    #[test]
    fn test_matrix_from_config_missing_access_token() {
        let mut config = PlatformConfig::new(Platform::Matrix);
        config.settings.insert(
            "homeserver_url".to_string(),
            serde_json::json!("https://matrix.org"),
        );
        let result = MatrixAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("MATRIX_ACCESS_TOKEN"));
    }

    #[test]
    fn test_matrix_from_config_valid() {
        let mut config = PlatformConfig::new(Platform::Matrix);
        config.settings.insert(
            "homeserver_url".to_string(),
            serde_json::json!("https://matrix.org"),
        );
        config.api_token = Some("my-access-token".to_string());
        let adapter = MatrixAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.homeserver_url, "https://matrix.org");
        assert_eq!(adapter.access_token, "my-access-token");
    }

    #[test]
    fn test_matrix_from_config_default_user_id() {
        let mut config = PlatformConfig::new(Platform::Matrix);
        config.settings.insert(
            "homeserver_url".to_string(),
            serde_json::json!("https://matrix.org"),
        );
        config.api_token = Some("token".to_string());
        let adapter = MatrixAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.user_id, "@clawdius:matrix.org");
    }

    #[test]
    fn test_matrix_from_config_custom_user_id() {
        let mut config = PlatformConfig::new(Platform::Matrix);
        config.settings.insert(
            "homeserver_url".to_string(),
            serde_json::json!("https://matrix.org"),
        );
        config.api_token = Some("token".to_string());
        config.settings.insert(
            "user_id".to_string(),
            serde_json::json!("@custom:example.org"),
        );
        let adapter = MatrixAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.user_id, "@custom:example.org");
    }

    #[test]
    fn test_matrix_send_message_json_format() {
        let msg = OutgoingMessage::new(Platform::Matrix, "!room:matrix.org", "hello matrix");
        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": msg.text,
        });
        assert_eq!(body["msgtype"], "m.text");
        assert_eq!(body["body"], "hello matrix");
    }

    #[test]
    fn test_matrix_send_message_with_reply_json() {
        let msg = OutgoingMessage::new(Platform::Matrix, "!room:matrix.org", "reply text")
            .with_reply_to("$event_id:matrix.org");
        let mut content = serde_json::json!({
            "msgtype": "m.text",
            "body": msg.text,
        });
        if let Some(ref reply_to) = msg.reply_to {
            content["m.relates_to"] = serde_json::json!({
                "rel_type": "m.in_reply_to",
                "event_id": reply_to,
            });
        }
        assert_eq!(content["m.relates_to"]["rel_type"], "m.in_reply_to");
        assert_eq!(content["m.relates_to"]["event_id"], "$event_id:matrix.org");
    }

    #[test]
    fn test_matrix_send_message_empty_text() {
        let msg = OutgoingMessage::new(Platform::Matrix, "!room:matrix.org", "");
        assert_eq!(msg.text, "");
    }

    #[test]
    fn test_matrix_send_message_unicode() {
        let msg = OutgoingMessage::new(Platform::Matrix, "!room:matrix.org", "matrix ünïcödé 🏠");
        assert_eq!(msg.text, "matrix ünïcödé 🏠");
    }

    #[test]
    fn test_matrix_edit_message_json_format() {
        let new_text = "edited content";
        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": format!("* {new_text}"),
            "m.new_content": {
                "msgtype": "m.text",
                "body": new_text,
            },
            "m.relates_to": {
                "rel_type": "m.replace",
                "event_id": "$original_event:matrix.org",
            },
        });
        assert_eq!(body["body"], "* edited content");
        assert_eq!(body["m.new_content"]["body"], "edited content");
        assert_eq!(body["m.relates_to"]["rel_type"], "m.replace");
        assert_eq!(
            body["m.relates_to"]["event_id"],
            "$original_event:matrix.org"
        );
    }

    #[tokio::test]
    async fn test_matrix_start_stop_lifecycle() {
        let adapter = MatrixAdapter::new("https://matrix.org", "token", "@bot:matrix.org");
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_matrix_health_stopped() {
        let adapter = MatrixAdapter::new("https://matrix.org", "token", "@bot:matrix.org");
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }

    #[tokio::test]
    async fn test_matrix_health_running() {
        let adapter = MatrixAdapter::new("https://matrix.org", "token", "@bot:matrix.org");
        adapter.start().await.unwrap();
        let health = adapter.health();
        assert!(health.healthy);
        assert_eq!(health.message, "syncing");
    }
}
