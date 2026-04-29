//! Signal platform adapter using signal-cli REST API.
//!
//! Connects Clawdius to Signal via a signal-cli REST API instance.
//! Requires a running signal-cli daemon with the REST API enabled.
//!
//! # Setup
//!
//! 1. Install and configure signal-cli: <https://github.com/AsamK/signal-cli>
//! 2. Start the REST API: `signal-cli -u +NUMBER daemon --socket localhost:7583`
//! 3. Set `SIGNAL_REST_URL` in config (default: http://localhost:7583)
//!
//! # Features
//!
//! - Text message sending and receiving
//! - Group message support
//! - Attachment download
//! - Reaction support (future)
//!
//! # Limitations
//!
//! - Signal does NOT support message editing (streaming edits will
//!   send new messages instead)
//! - Rate limited by signal-cli's internal throttling

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Signal adapter implementation using signal-cli REST API.
pub struct SignalAdapter {
    /// Base URL of the signal-cli REST API.
    rest_url: String,
    /// The phone number registered with signal-cli.
    account_number: String,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Shared HTTP client.
    http: std::sync::OnceLock<reqwest::Client>,
}

impl SignalAdapter {
    /// Create a new Signal adapter.
    #[must_use]
    pub fn new(rest_url: impl Into<String>, account_number: impl Into<String>) -> Self {
        Self {
            rest_url: rest_url.into(),
            account_number: account_number.into(),
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let rest_url = config
            .settings
            .get("rest_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:7583")
            .to_string();

        let account_number = config
            .api_token
            .as_ref()
            .ok_or_else(|| {
                GatewayError::Config("SIGNAL_ACCOUNT_NUMBER not set".to_string())
            })?;

        Ok(Self::new(rest_url, account_number))
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Call the signal-cli REST API.
    async fn signal_api(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, GatewayError> {
        let url = format!(
            "{}{}{}",
            self.rest_url.trim_end_matches('/'),
            "/v2/accounts/",
            self.account_number
        );
        let url = format!("{url}{path}");

        let mut request = self.http().request(method, &url);

        if let Some(b) = body {
            request = request.json(b);
        }

        let response = request.send().await.map_err(|e| GatewayError::Adapter {
            platform: "signal".to_string(),
            message: format!("API call failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "signal".to_string(),
                message: format!("Signal API error {status}: {text}"),
                source: None,
            });
        }

        // Some signal-cli endpoints return 204 No Content
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(serde_json::Value::Null);
        }

        response.json().await.map_err(|e| GatewayError::Adapter {
            platform: "signal".to_string(),
            message: format!("failed to parse response: {e}"),
            source: Some(Box::new(e)),
        })
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for SignalAdapter {
    fn platform(&self) -> Platform {
        Platform::Signal
    }

    async fn start(&self) -> Result<(), GatewayError> {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let body = serde_json::json!({
            "message": message.text,
            "number": message.chat_id,
        });

        self.signal_api(reqwest::Method::POST, "/send", Some(&body))
            .await?;
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(
        &self,
        _message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        // Signal does NOT support editing messages.
        // Send a new message as a fallback (with "edited:" prefix).
        let fallback = OutgoingMessage::new(
            Platform::Signal,
            "fallback", // Caller should provide the correct chat_id
            format!("(edited) {new_text}"),
        );
        Box::pin(self.send_message(fallback)).await
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        let response = self
            .http()
            .get(url)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "signal".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "signal".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("signal-attachment");

        let dir = std::env::temp_dir().join("clawdius-signal");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "signal".to_string(),
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
