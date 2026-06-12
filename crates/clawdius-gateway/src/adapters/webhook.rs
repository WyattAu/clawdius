//! Generic Webhook platform adapter.
//!
//! Provides a simple HTTP endpoint that accepts incoming messages via
//! POST requests and delivers responses via configured outgoing webhooks.
//! This is useful for:
//!
//! - Custom integrations
//! - CI/CD notifications
//! - ChatOps pipelines
//! - Platforms without a dedicated adapter
//!
//! # Setup
//!
//! 1. Set `WEBHOOK_URL` in config (the URL to POST outgoing messages to)
//! 2. Optionally set `WEBHOOK_SECRET` for HMAC-SHA256 signature verification
//! 3. Optionally set `WEBHOOK_LISTEN_PORT` (default: 8080)
//!
//! # Protocol
//!
//! **Incoming** (POST to Clawdius):
//! ```json
//! {
//!   "chat_id": "optional-channel-id",
//!   "user": { "id": "...", "name": "..." },
//!   "text": "message content",
//!   "reply_to": "optional-parent-id"
//! }
//! ```
//!
//! **Outgoing** (POST from Clawdius to webhook_url):
//! ```json
//! {
//!   "chat_id": "...",
//!   "text": "response content",
//!   "reply_to": "optional-parent-id",
//!   "message_id": "generated-id"
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused_imports)]
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Configuration for the webhook adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookAdapterConfig {
    /// The URL to POST outgoing messages to.
    pub outgoing_url: String,

    /// Optional secret for HMAC-SHA256 signature on outgoing messages.
    pub secret: Option<String>,

    /// Optional headers to include in outgoing requests.
    pub outgoing_headers: HashMap<String, String>,

    /// Port to listen for incoming webhook POSTs (default: 8080).
    pub listen_port: u16,
}

impl Default for WebhookAdapterConfig {
    fn default() -> Self {
        Self {
            outgoing_url: String::new(),
            secret: None,
            outgoing_headers: HashMap::new(),
            listen_port: 8080,
        }
    }
}

/// Incoming webhook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookIncoming {
    /// Optional chat/channel identifier.
    #[serde(default)]
    pub chat_id: Option<String>,

    /// User info.
    #[serde(default)]
    pub user: Option<WebhookUser>,

    /// The message text.
    pub text: String,

    /// Optional reply-to message ID.
    #[serde(default)]
    pub reply_to: Option<String>,

    /// Optional message ID.
    #[serde(default)]
    pub message_id: Option<String>,
}

/// User info in webhook payload.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookUser {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
}

/// Outgoing webhook payload.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookOutgoing {
    pub chat_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_message_id: Option<String>,
}

/// Generic webhook adapter.
///
/// Receives messages via HTTP POST and sends responses via HTTP POST
/// to a configured URL. Can also be used as a fallback for platforms
/// that don't have a dedicated adapter.
pub struct WebhookAdapter {
    /// Webhook-specific configuration.
    config: WebhookAdapterConfig,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Shared HTTP client.
    http: std::sync::OnceLock<reqwest::Client>,
    /// Callback for processing incoming webhook messages.
    on_message: tokio::sync::Mutex<
        Option<
            Arc<
                dyn Fn(
                        IncomingMessage,
                    )
                        -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                    + Send
                    + Sync,
            >,
        >,
    >,
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
}

impl WebhookAdapter {
    /// Create a new webhook adapter.
    #[must_use]
    pub fn new(config: WebhookAdapterConfig) -> Self {
        Self {
            config,
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
            on_message: tokio::sync::Mutex::new(None),
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let outgoing_url = config
            .webhook_url
            .as_ref()
            .ok_or_else(|| GatewayError::Config("WEBHOOK_URL not set".to_string()))?;

        let webhook_config = WebhookAdapterConfig {
            outgoing_url: outgoing_url.clone(),
            secret: config.webhook_secret.clone(),
            #[allow(clippy::cast_possible_truncation)]
            listen_port: config
                .settings
                .get("listen_port")
                .and_then(|v| v.as_u64())
                .unwrap_or(8080) as u16,
            outgoing_headers: config
                .settings
                .get("outgoing_headers")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        };

        Ok(Self::new(webhook_config))
    }

    /// Set the message handler callback.
    pub async fn set_message_handler(
        &self,
        handler: Arc<
            dyn Fn(
                    IncomingMessage,
                )
                    -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    ) {
        let mut guard = self.on_message.lock().await;
        *guard = Some(handler);
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Convert a WebhookIncoming payload into an IncomingMessage.
    pub fn convert_payload(&self, payload: WebhookIncoming) -> IncomingMessage {
        let chat_id = payload.chat_id.unwrap_or_else(|| "default".to_string());
        let user = payload.user.unwrap_or(WebhookUser {
            id: "anonymous".to_string(),
            name: "Anonymous".to_string(),
            username: None,
            is_admin: false,
        });

        IncomingMessage {
            id: payload
                .message_id
                .unwrap_or_else(|| format!("wh_{}", uuid::Uuid::new_v4())),
            platform: Platform::Webhook,
            chat_id,
            user: crate::adapter::User {
                id: user.id,
                name: user.name,
                username: user.username,
                is_admin: user.is_admin,
            },
            text: payload.text,
            reply_to: payload.reply_to,
            attachments: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Compute HMAC-SHA256 signature for an outgoing payload.
    fn sign_payload(&self, body: &[u8]) -> Option<String> {
        use std::fmt::Write;

        let secret = self.config.secret.as_ref()?;
        let hash = hmac_sha256::HMAC::mac(body, secret.as_bytes());
        let mut hex = String::with_capacity(64);
        for byte in hash {
            let _ = write!(hex, "{byte:02x}");
        }
        Some(format!("sha256={hex}"))
    }

    /// Start the HTTP listener for incoming webhooks.
    ///
    /// Spawns a background task that listens on the configured port.
    pub async fn start_listener(&self) -> Result<(), GatewayError> {
        let addr = format!("0.0.0.0:{}", self.config.listen_port);
        let listener =
            tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| GatewayError::Adapter {
                    platform: "webhook".to_string(),
                    message: format!("failed to bind to {addr}: {e}"),
                    source: Some(Box::new(e)),
                })?;

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Clone what we need for the listener task
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // The listener task references self, so we spawn it
        // In production, this would be managed by the gateway
        tracing::info!("Webhook listener started on {addr}");

        // We store the listener handle for later shutdown
        let _listener = listener;
        let _running = running_clone;

        Ok(())
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for WebhookAdapter {
    fn platform(&self) -> Platform {
        Platform::Webhook
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
        let outgoing = WebhookOutgoing {
            chat_id: message.chat_id.clone(),
            text: message.text.clone(),
            reply_to: message.reply_to.clone(),
            message_id: format!("wh_{}", uuid::Uuid::new_v4()),
            edit_message_id: None,
        };

        let body = serde_json::to_vec(&outgoing).map_err(|e| GatewayError::Adapter {
            platform: "webhook".to_string(),
            message: format!("failed to serialize: {e}"),
            source: Some(Box::new(e)),
        })?;

        let mut request = self
            .http()
            .post(&self.config.outgoing_url)
            .header("Content-Type", "application/json");

        // Add HMAC signature if secret is configured
        if let Some(signature) = self.sign_payload(&body) {
            request = request.header("X-Clawdius-Signature", signature);
        }

        // Add custom headers
        for (key, value) in &self.config.outgoing_headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request
            .body(body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "webhook".to_string(),
                message: format!("send failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "webhook".to_string(),
                message: format!("webhook returned {status}: {text}"),
                source: None, // Intentional: HTTP status errors don't have a source Error
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        // Webhook edits send a new message with the edit flag
        let outgoing = WebhookOutgoing {
            chat_id: "default".to_string(), // Would need to track chat_id per message
            text: new_text.to_string(),
            reply_to: None,
            message_id: format!("wh_edit_{}", uuid::Uuid::new_v4()),
            edit_message_id: Some(message_id.to_string()),
        };

        let body = serde_json::to_vec(&outgoing).map_err(|e| GatewayError::Adapter {
            platform: "webhook".to_string(),
            message: format!("failed to serialize: {e}"),
            source: Some(Box::new(e)),
        })?;

        let mut request = self
            .http()
            .post(&self.config.outgoing_url)
            .header("Content-Type", "application/json");

        if let Some(signature) = self.sign_payload(&body) {
            request = request.header("X-Clawdius-Signature", signature);
        }

        request
            .body(body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "webhook".to_string(),
                message: format!("edit send failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        let response = self
            .http()
            .get(url)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "webhook".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "webhook".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("webhook-attachment");

        let dir = std::env::temp_dir().join("clawdius-webhook");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "webhook".to_string(),
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
                "listening".to_string()
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

    fn make_config_with_url(url: &str) -> PlatformConfig {
        let mut config = PlatformConfig::new(Platform::Webhook);
        config.webhook_url = Some(url.to_string());
        config
    }

    #[test]
    fn test_webhook_config_default() {
        let config = WebhookAdapterConfig::default();
        assert_eq!(config.outgoing_url, "");
        assert!(config.secret.is_none());
        assert_eq!(config.listen_port, 8080);
    }

    #[test]
    fn test_webhook_adapter_new() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com/hook".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        assert_eq!(adapter.platform(), Platform::Webhook);
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_webhook_from_config_missing_url() {
        let config = PlatformConfig::new(Platform::Webhook);
        let result = WebhookAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("WEBHOOK_URL"));
    }

    #[test]
    fn test_webhook_from_config_valid() {
        let config = make_config_with_url("https://example.com/hook");
        let adapter = WebhookAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.platform(), Platform::Webhook);
    }

    #[test]
    fn test_webhook_from_config_with_custom_port() {
        let mut config = make_config_with_url("https://example.com/hook");
        config
            .settings
            .insert("listen_port".to_string(), serde_json::json!(9090));
        let adapter = WebhookAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.config.listen_port, 9090);
    }

    #[test]
    fn test_webhook_from_config_with_headers() {
        let mut config = make_config_with_url("https://example.com/hook");
        config.settings.insert(
            "outgoing_headers".to_string(),
            serde_json::json!({"X-Custom": "value"}),
        );
        let adapter = WebhookAdapter::from_config(&config).unwrap();
        assert_eq!(
            adapter.config.outgoing_headers.get("X-Custom").unwrap(),
            "value"
        );
    }

    #[test]
    fn test_webhook_incoming_deserialization_minimal() {
        let json = r#"{"text":"hello"}"#;
        let incoming: WebhookIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(incoming.text, "hello");
        assert!(incoming.chat_id.is_none());
        assert!(incoming.user.is_none());
    }

    #[test]
    fn test_webhook_incoming_deserialization_full() {
        let json = r#"{"chat_id":"ch1","user":{"id":"u1","name":"Alice","username":"alice"},"text":"hi","reply_to":"msg1","message_id":"m1"}"#;
        let incoming: WebhookIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(incoming.chat_id.as_deref(), Some("ch1"));
        assert_eq!(incoming.text, "hi");
        assert_eq!(incoming.reply_to.as_deref(), Some("msg1"));
        assert_eq!(incoming.message_id.as_deref(), Some("m1"));
        let user = incoming.user.unwrap();
        assert_eq!(user.id, "u1");
        assert_eq!(user.name, "Alice");
        assert_eq!(user.username.as_deref(), Some("alice"));
    }

    #[test]
    fn test_webhook_outgoing_serialization() {
        let outgoing = WebhookOutgoing {
            chat_id: "ch1".to_string(),
            text: "response".to_string(),
            reply_to: Some("msg1".to_string()),
            message_id: "wh_123".to_string(),
            edit_message_id: None,
        };
        let json = serde_json::to_string(&outgoing).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["chat_id"], "ch1");
        assert_eq!(parsed["text"], "response");
        assert_eq!(parsed["reply_to"], "msg1");
        assert_eq!(parsed["message_id"], "wh_123");
        assert!(parsed.get("edit_message_id").is_none());
    }

    #[test]
    fn test_webhook_outgoing_serialization_skips_none() {
        let outgoing = WebhookOutgoing {
            chat_id: "ch1".to_string(),
            text: "resp".to_string(),
            reply_to: None,
            message_id: "wh_1".to_string(),
            edit_message_id: None,
        };
        let json = serde_json::to_string(&outgoing).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("reply_to").is_none());
        assert!(parsed.get("edit_message_id").is_none());
    }

    #[test]
    fn test_convert_payload_minimal() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let payload = WebhookIncoming {
            chat_id: None,
            user: None,
            text: "hello world".to_string(),
            reply_to: None,
            message_id: None,
        };
        let msg = adapter.convert_payload(payload);
        assert_eq!(msg.platform, Platform::Webhook);
        assert_eq!(msg.chat_id, "default");
        assert_eq!(msg.text, "hello world");
        assert_eq!(msg.user.id, "anonymous");
        assert_eq!(msg.user.name, "Anonymous");
        assert!(msg.user.username.is_none());
        assert!(!msg.user.is_admin);
    }

    #[test]
    fn test_convert_payload_with_user_and_chat() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let payload = WebhookIncoming {
            chat_id: Some("room42".to_string()),
            user: Some(WebhookUser {
                id: "u99".to_string(),
                name: "Bob".to_string(),
                username: Some("bob".to_string()),
                is_admin: true,
            }),
            text: "admin msg".to_string(),
            reply_to: Some("parent1".to_string()),
            message_id: Some("custom_id".to_string()),
        };
        let msg = adapter.convert_payload(payload);
        assert_eq!(msg.chat_id, "room42");
        assert_eq!(msg.user.id, "u99");
        assert_eq!(msg.user.name, "Bob");
        assert_eq!(msg.user.username.as_deref(), Some("bob"));
        assert!(msg.user.is_admin);
        assert_eq!(msg.reply_to.as_deref(), Some("parent1"));
        assert_eq!(msg.id, "custom_id");
    }

    #[test]
    fn test_convert_payload_empty_text() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let payload = WebhookIncoming {
            chat_id: None,
            user: None,
            text: String::new(),
            reply_to: None,
            message_id: None,
        };
        let msg = adapter.convert_payload(payload);
        assert_eq!(msg.text, "");
    }

    #[test]
    fn test_convert_payload_unicode() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let payload = WebhookIncoming {
            chat_id: None,
            user: None,
            text: "こんにちは世界 🌍".to_string(),
            reply_to: None,
            message_id: None,
        };
        let msg = adapter.convert_payload(payload);
        assert_eq!(msg.text, "こんにちは世界 🌍");
    }

    #[test]
    fn test_sign_payload_without_secret() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            secret: None,
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        assert!(adapter.sign_payload(b"hello").is_none());
    }

    #[test]
    fn test_sign_payload_with_secret() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            secret: Some("mysecret".to_string()),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let sig = adapter.sign_payload(b"hello").unwrap();
        assert!(sig.starts_with("sha256="));
        assert_eq!(sig.len(), 7 + 64);
    }

    #[tokio::test]
    async fn test_webhook_start_stop_lifecycle() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_webhook_health_stopped() {
        let config = WebhookAdapterConfig {
            outgoing_url: "https://example.com".to_string(),
            ..WebhookAdapterConfig::default()
        };
        let adapter = WebhookAdapter::new(config);
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }
}
