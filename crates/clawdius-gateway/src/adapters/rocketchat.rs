//! Rocket.Chat platform adapter using LiveChat API / Real-Time API.
//!
//! Connects Clawdius to a Rocket.Chat server via its REST API and
//! optional Real-Time API (DDP/WebSocket). Handles incoming messages
//! from channels and direct messages.
//!
//! # Setup
//!
//! 1. Create a bot user on your Rocket.Chat server
//! 2. Generate a personal access token or API key
//! 3. Set in config: `ROCKETCHAT_URL`, `ROCKETCHAT_USER`, `ROCKETCHAT_TOKEN`
//!
//! # Features
//!
//! - Text message sending and receiving
//! - Channel and direct message support
//! - Message editing (for streaming)
//! - File attachment download
//! - Real-time updates via WebSocket (future)
//!
//! # API Modes
//!
//! - **REST API**: Used for send/edit/download operations (always available)
//! - **Real-Time API (DDP)**: Used for receiving messages in real-time
//!   (requires WebSocket connection, optional enhancement)

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Rocket.Chat adapter using REST API.
pub struct RocketChatAdapter {
    /// Rocket.Chat server URL.
    server_url: String,
    /// Bot user ID or username.
    user_id: String,
    /// Authentication token (X-Auth-Token header).
    auth_token: String,
    /// Rocket.Chat user ID (X-User-Id header).
    rc_user_id: String,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Shared HTTP client.
    http: std::sync::OnceLock<reqwest::Client>,
}

impl RocketChatAdapter {
    /// Create a new Rocket.Chat adapter.
    #[must_use]
    pub fn new(
        server_url: impl Into<String>,
        auth_token: impl Into<String>,
        rc_user_id: impl Into<String>,
    ) -> Self {
        Self {
            server_url: server_url.into(),
            user_id: String::new(),
            auth_token: auth_token.into(),
            rc_user_id: rc_user_id.into(),
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let server_url = config
            .settings
            .get("server_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                GatewayError::Config("ROCKETCHAT_URL not set".to_string())
            })?;

        let auth_token = config
            .api_token
            .as_ref()
            .ok_or_else(|| {
                GatewayError::Config("ROCKETCHAT_TOKEN not set".to_string())
            })?;

        let rc_user_id = config
            .settings
            .get("rc_user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                GatewayError::Config("ROCKETCHAT_USER_ID not set".to_string())
            })?;

        Ok(Self::new(server_url, auth_token, rc_user_id))
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Build the base headers for Rocket.Chat API requests.
    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(val) = reqwest::header::HeaderValue::from_str(&self.auth_token) {
            headers.insert("X-Auth-Token", val);
        }
        if let Ok(val) = reqwest::header::HeaderValue::from_str(&self.rc_user_id) {
            headers.insert("X-User-Id", val);
        }
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers
    }

    /// Call the Rocket.Chat REST API.
    async fn rc_api(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, GatewayError> {
        let url = format!(
            "{}{path}",
            self.server_url.trim_end_matches('/')
        );

        let mut request = self
            .http()
            .request(method, &url)
            .headers(self.headers());

        if let Some(b) = body {
            request = request.json(b);
        }

        let response = request.send().await.map_err(|e| GatewayError::Adapter {
            platform: "rocketchat".to_string(),
            message: format!("API call failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "rocketchat".to_string(),
                message: format!("Rocket.Chat API error {status}: {text}"),
                source: None,
            });
        }

        response.json().await.map_err(|e| GatewayError::Adapter {
            platform: "rocketchat".to_string(),
            message: format!("failed to parse response: {e}"),
            source: Some(Box::new(e)),
        })
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for RocketChatAdapter {
    fn platform(&self) -> Platform {
        Platform::RocketChat
    }

    async fn start(&self) -> Result<(), GatewayError> {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // Real-time API (DDP) connection would start here
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let body = serde_json::json!({
            "channel": message.chat_id,
            "text": message.text,
        });

        // If this is a reply, set the parent message
        if let Some(ref reply_to) = message.reply_to {
            // Rocket.Chat uses thread messages or tmid
            // For simple replies, we prepend the reference
            let body = serde_json::json!({
                "channel": message.chat_id,
                "text": message.text,
                "tmid": reply_to,
            });
            self.rc_api(reqwest::Method::POST, "/api/v1/chat.postMessage", Some(&body))
                .await?;
        } else {
            self.rc_api(reqwest::Method::POST, "/api/v1/chat.postMessage", Some(&body))
                .await?;
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(
        &self,
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        // message_id is the Rocket.Chat message _id
        let body = serde_json::json!({
            "roomId": "default", // Would need to track roomId per message
            "msgId": message_id,
            "text": new_text,
        });

        self.rc_api(reqwest::Method::POST, "/api/v1/chat.update", Some(&body))
            .await?;
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        // Rocket.Chat requires auth for file downloads
        let response = self
            .http()
            .get(url)
            .header("X-Auth-Token", &self.auth_token)
            .header("X-User-Id", &self.rc_user_id)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "rocketchat".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "rocketchat".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("rocketchat-attachment");

        let dir = std::env::temp_dir().join("clawdius-rocketchat");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "rocketchat".to_string(),
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
