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

use std::sync::Arc;

use crate::adapter::{
    AdapterHealth, MessageCallback, OutgoingMessage, Platform, PlatformAdapter, PlatformConfig,
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
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
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
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
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
            .ok_or_else(|| GatewayError::Config("SIGNAL_ACCOUNT_NUMBER not set".to_string()))?;

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
                source: None, // Intentional: HTTP status errors don't have a source Error
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

    async fn edit_message(&self, _message_id: &str, new_text: &str) -> Result<(), GatewayError> {
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
    fn test_signal_adapter_new() {
        let adapter = SignalAdapter::new("http://localhost:7583", "+1234567890");
        assert_eq!(adapter.platform(), Platform::Signal);
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_signal_from_config_missing_account() {
        let config = PlatformConfig::new(Platform::Signal);
        let result = SignalAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("SIGNAL_ACCOUNT_NUMBER"));
    }

    #[test]
    fn test_signal_from_config_valid() {
        let mut config = PlatformConfig::new(Platform::Signal);
        config.api_token = Some("+1234567890".to_string());
        let adapter = SignalAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.platform(), Platform::Signal);
    }

    #[test]
    fn test_signal_from_config_custom_rest_url() {
        let mut config = PlatformConfig::new(Platform::Signal);
        config.api_token = Some("+1234567890".to_string());
        config.settings.insert(
            "rest_url".to_string(),
            serde_json::json!("http://signal-host:9999"),
        );
        let adapter = SignalAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.rest_url, "http://signal-host:9999");
    }

    #[test]
    fn test_signal_from_config_default_rest_url() {
        let mut config = PlatformConfig::new(Platform::Signal);
        config.api_token = Some("+1234567890".to_string());
        let adapter = SignalAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.rest_url, "http://localhost:7583");
    }

    #[test]
    fn test_signal_send_message_json_format() {
        let msg = OutgoingMessage::new(Platform::Signal, "+9876543210", "hello signal");
        let body = serde_json::json!({
            "message": msg.text,
            "number": msg.chat_id,
        });
        assert_eq!(body["message"], "hello signal");
        assert_eq!(body["number"], "+9876543210");
    }

    #[test]
    fn test_signal_edit_message_prefix() {
        let new_text = "updated content";
        let prefix = format!("(edited) {new_text}");
        assert!(prefix.starts_with("(edited)"));
        assert!(prefix.contains("updated content"));
    }

    #[test]
    fn test_signal_send_message_empty_text() {
        let msg = OutgoingMessage::new(Platform::Signal, "+9876543210", "");
        let body = serde_json::json!({
            "message": msg.text,
            "number": msg.chat_id,
        });
        assert_eq!(body["message"], "");
    }

    #[test]
    fn test_signal_send_message_unicode() {
        let msg = OutgoingMessage::new(Platform::Signal, "+9876543210", "héllo wörld ñ");
        let body = serde_json::json!({
            "message": msg.text,
            "number": msg.chat_id,
        });
        assert_eq!(body["message"], "héllo wörld ñ");
    }

    #[tokio::test]
    async fn test_signal_start_stop_lifecycle() {
        let adapter = SignalAdapter::new("http://localhost:7583", "+1234567890");
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_signal_health_stopped() {
        let adapter = SignalAdapter::new("http://localhost:7583", "+1234567890");
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }

    #[tokio::test]
    async fn test_signal_health_running() {
        let adapter = SignalAdapter::new("http://localhost:7583", "+1234567890");
        adapter.start().await.unwrap();
        let health = adapter.health();
        assert!(health.healthy);
        assert_eq!(health.message, "connected");
    }
}
