//! WhatsApp Business API platform adapter.
//!
//! Connects Clawdius to WhatsApp via the Meta Cloud API (WhatsApp Business
//! Platform). Requires a WhatsApp Business account and Meta developer setup.
//!
//! # Setup
//!
//! 1. Create a Meta Developer account at <https://developers.facebook.com>
//! 2. Create a WhatsApp Business Platform app
//! 3. Get a permanent access token and phone number ID
//! 4. Set in config: `WHATSAPP_ACCESS_TOKEN` and `WHATSAPP_PHONE_NUMBER_ID`
//! 5. Configure the webhook in Meta Developer Portal
//!
//! # Features
//!
//! - Text message sending and receiving
//! - Interactive messages (future)
//! - Media message support
//! - Message template support (future)
//!
//! # Limitations
//!
//! - WhatsApp does NOT support message editing
//! - Messages must be responded to within 24 hours (window policy)
//! - Rate limited by Meta's API tier

use std::sync::Arc;

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// WhatsApp adapter using Meta Cloud API.
pub struct WhatsAppAdapter {
    /// Meta API base URL.
    api_url: String,
    /// WhatsApp Business access token.
    access_token: String,
    /// Phone number ID for sending messages.
    #[allow(dead_code)]
    phone_number_id: String,
    /// Verify token for webhook verification.
    verify_token: Option<String>,
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

impl WhatsAppAdapter {
    /// Create a new WhatsApp adapter.
    #[must_use]
    pub fn new(access_token: impl Into<String>, phone_number_id: impl Into<String>) -> Self {
        Self {
            api_url: "https://graph.facebook.com/v21.0".to_string(),
            access_token: access_token.into(),
            phone_number_id: phone_number_id.into(),
            verify_token: None,
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create from a `PlatformConfig`.
    ///
    /// # Errors
    ///
    /// Returns `Err(GatewayError)` if required config fields are missing.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let access_token = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("WHATSAPP_ACCESS_TOKEN not set".to_string()))?;

        let phone_number_id = config
            .settings
            .get("phone_number_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::Config("WHATSAPP_PHONE_NUMBER_ID not set".to_string()))?;

        let mut adapter = Self::new(access_token, phone_number_id);
        adapter.verify_token = config.webhook_secret.clone();
        Ok(adapter)
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Verify a webhook challenge from Meta.
    ///
    /// Returns the challenge string if verification succeeds, or an error.
    pub fn verify_webhook(
        &self,
        mode: &str,
        token: &str,
        challenge: &str,
    ) -> Result<String, GatewayError> {
        if mode != "subscribe" {
            return Err(GatewayError::Adapter {
                platform: "whatsapp".to_string(),
                message: format!("invalid webhook mode: {mode}"),
                source: None, // Intentional: no source Error available
            });
        }

        if let Some(ref verify_token) = self.verify_token {
            if token != verify_token {
                return Err(GatewayError::Adapter {
                    platform: "whatsapp".to_string(),
                    message: "webhook verification token mismatch".to_string(),
                    source: None, // Intentional: no source Error available
                });
            }
        }

        Ok(challenge.to_string())
    }

    /// Parse an incoming WhatsApp webhook payload into `IncomingMessages`.
    pub fn parse_webhook_payload(&self, body: &serde_json::Value) -> Vec<IncomingMessage> {
        let mut messages = Vec::new();

        // Navigate the WhatsApp webhook structure
        let Some(entries) = body.get("entry").and_then(|e| e.as_array()) else {
            return messages;
        };

        for entry in entries {
            let Some(changes) = entry.get("changes").and_then(|c| c.as_array()) else {
                continue;
            };

            for change in changes {
                let Some(value) = change.get("value") else {
                    continue;
                };

                let contacts = value
                    .get("contacts")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();

                let messages_arr = value
                    .get("messages")
                    .and_then(|m| m.as_array())
                    .cloned()
                    .unwrap_or_default();

                for msg in &messages_arr {
                    let msg_type = msg
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");

                    let text = if msg_type == "text" {
                        msg.get("text")
                            .and_then(|t| t.get("body"))
                            .and_then(|b| b.as_str())
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        String::new()
                    };

                    // Extract sender info from contacts
                    let (user_id, user_name) = contacts.first().map_or_else(
                        || ("unknown".to_string(), "Unknown".to_string()),
                        |c| {
                            let wa_id = c
                                .get("wa_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let name = c
                                .get("profile")
                                .and_then(|p| p.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("Unknown")
                                .to_string();
                            (wa_id, name)
                        },
                    );

                    let message_id = msg
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or_default()
                        .to_string();

                    let timestamp = msg
                        .get("timestamp")
                        .and_then(|t| t.as_str())
                        .and_then(|ts| ts.parse::<i64>().ok())
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                        .unwrap_or_else(chrono::Utc::now);

                    messages.push(IncomingMessage {
                        id: message_id,
                        platform: Platform::WhatsApp,
                        chat_id: user_id.clone(),
                        user: crate::adapter::User {
                            id: user_id,
                            name: user_name,
                            username: None,
                            is_admin: false,
                        },
                        text,
                        reply_to: None,
                        attachments: Vec::new(),
                        timestamp,
                        metadata: {
                            let mut meta = std::collections::HashMap::new();
                            meta.insert("msg_type".to_string(), serde_json::json!(msg_type));
                            meta
                        },
                    });
                }
            }
        }

        messages
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for WhatsAppAdapter {
    fn platform(&self) -> Platform {
        Platform::WhatsApp
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
        let url = format!("{}/messages", self.api_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": message.chat_id,
            "type": "text",
            "text": {
                "body": message.text,
                "preview_url": false,
            },
        });

        let response = self
            .http()
            .post(&url)
            .query(&[("access_token", &self.access_token)])
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "whatsapp".to_string(),
                message: format!("send failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "whatsapp".to_string(),
                message: format!("send failed with {status}: {text}"),
                source: None, // Intentional: HTTP status errors don't have a source Error
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        // WhatsApp does NOT support message editing.
        // Send as a new message. Extract chat_id from message_id (format: "chat_id:msg_id").
        let chat_id = message_id.split(':').next().unwrap_or_else(|| {
            tracing::warn!("whatsapp edit_message: no chat_id in message_id");
            ""
        });
        let fallback = OutgoingMessage::new(
            Platform::WhatsApp,
            chat_id,
            format!("(corrected) {new_text}"),
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
                platform: "whatsapp".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "whatsapp".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("whatsapp-attachment");

        let dir = std::env::temp_dir().join("clawdius-whatsapp");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "whatsapp".to_string(),
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
    fn test_whatsapp_adapter_new() {
        let adapter = WhatsAppAdapter::new("my-token", "phone-id-123");
        assert_eq!(adapter.platform(), Platform::WhatsApp);
        assert_eq!(adapter.access_token, "my-token");
        assert_eq!(adapter.phone_number_id, "phone-id-123");
        assert_eq!(adapter.api_url, "https://graph.facebook.com/v21.0");
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_whatsapp_from_config_missing_access_token() {
        let config = PlatformConfig::new(Platform::WhatsApp);
        let result = WhatsAppAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("WHATSAPP_ACCESS_TOKEN"));
    }

    #[test]
    fn test_whatsapp_from_config_missing_phone_number_id() {
        let mut config = PlatformConfig::new(Platform::WhatsApp);
        config.api_token = Some("my-token".to_string());
        let result = WhatsAppAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("WHATSAPP_PHONE_NUMBER_ID"));
    }

    #[test]
    fn test_whatsapp_from_config_valid() {
        let mut config = PlatformConfig::new(Platform::WhatsApp);
        config.api_token = Some("my-token".to_string());
        config.settings.insert(
            "phone_number_id".to_string(),
            serde_json::json!("phone-123"),
        );
        let adapter = WhatsAppAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.access_token, "my-token");
        assert_eq!(adapter.phone_number_id, "phone-123");
    }

    #[test]
    fn test_whatsapp_from_config_with_verify_token() {
        let mut config = PlatformConfig::new(Platform::WhatsApp);
        config.api_token = Some("my-token".to_string());
        config.settings.insert(
            "phone_number_id".to_string(),
            serde_json::json!("phone-123"),
        );
        config.webhook_secret = Some("verify-secret".to_string());
        let adapter = WhatsAppAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.verify_token.as_deref(), Some("verify-secret"));
    }

    #[test]
    fn test_whatsapp_verify_webhook_valid() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let result = adapter.verify_webhook("subscribe", "any-token", "challenge-123");
        assert_eq!(result.unwrap(), "challenge-123");
    }

    #[test]
    fn test_whatsapp_verify_webhook_invalid_mode() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let result = adapter.verify_webhook("invalid_mode", "any-token", "challenge");
        assert!(result.is_err());
    }

    #[test]
    fn test_whatsapp_verify_webhook_token_mismatch() {
        let mut adapter = WhatsAppAdapter::new("token", "phone-id");
        adapter.verify_token = Some("correct-secret".to_string());
        let result = adapter.verify_webhook("subscribe", "wrong-secret", "challenge");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_whatsapp_verify_webhook_no_verify_token_set() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let result = adapter.verify_webhook("subscribe", "anything", "challenge-42");
        assert_eq!(result.unwrap(), "challenge-42");
    }

    #[test]
    fn test_whatsapp_parse_webhook_empty_body() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let messages = adapter.parse_webhook_payload(&serde_json::json!({}));
        assert!(messages.is_empty());
    }

    #[test]
    fn test_whatsapp_parse_webhook_text_message() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "contacts": [{"wa_id": "551199999", "profile": {"name": "Alice"}}],
                        "messages": [{
                            "type": "text",
                            "text": {"body": "hello whatsapp"},
                            "id": "wamid_123",
                            "timestamp": "1700000000"
                        }]
                    }
                }]
            }]
        });
        let messages = adapter.parse_webhook_payload(&body);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].platform, Platform::WhatsApp);
        assert_eq!(messages[0].text, "hello whatsapp");
        assert_eq!(messages[0].user.id, "551199999");
        assert_eq!(messages[0].user.name, "Alice");
        assert_eq!(messages[0].id, "wamid_123");
    }

    #[test]
    fn test_whatsapp_parse_webhook_non_text_message() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "contacts": [{"wa_id": "551199999", "profile": {"name": "Bob"}}],
                        "messages": [{
                            "type": "image",
                            "id": "wamid_img",
                            "timestamp": "1700000000"
                        }]
                    }
                }]
            }]
        });
        let messages = adapter.parse_webhook_payload(&body);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "");
        assert_eq!(messages[0].metadata.get("msg_type").unwrap(), "image");
    }

    #[test]
    fn test_whatsapp_parse_webhook_no_contacts() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "type": "text",
                            "text": {"body": "from unknown"},
                            "id": "wamid_x",
                            "timestamp": "1700000000"
                        }]
                    }
                }]
            }]
        });
        let messages = adapter.parse_webhook_payload(&body);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].user.id, "unknown");
        assert_eq!(messages[0].user.name, "Unknown");
    }

    #[test]
    fn test_whatsapp_parse_webhook_multiple_messages() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "contacts": [{"wa_id": "5511", "profile": {"name": "A"}}],
                        "messages": [
                            {"type": "text", "text": {"body": "first"}, "id": "m1", "timestamp": "1700000000"},
                            {"type": "text", "text": {"body": "second"}, "id": "m2", "timestamp": "1700000001"}
                        ]
                    }
                }]
            }]
        });
        let messages = adapter.parse_webhook_payload(&body);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "first");
        assert_eq!(messages[1].text, "second");
    }

    #[test]
    fn test_whatsapp_parse_webhook_unicode_message() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "contacts": [{"wa_id": "5511", "profile": {"name": "ユーザー"}}],
                        "messages": [{
                            "type": "text",
                            "text": {"body": "こんにちは 🎉"},
                            "id": "wamid_u",
                            "timestamp": "1700000000"
                        }]
                    }
                }]
            }]
        });
        let messages = adapter.parse_webhook_payload(&body);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "こんにちは 🎉");
        assert_eq!(messages[0].user.name, "ユーザー");
    }

    #[test]
    fn test_whatsapp_send_message_json_format() {
        let msg = OutgoingMessage::new(Platform::WhatsApp, "551199999", "hello");
        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": msg.chat_id,
            "type": "text",
            "text": {
                "body": msg.text,
                "preview_url": false,
            },
        });
        assert_eq!(body["messaging_product"], "whatsapp");
        assert_eq!(body["to"], "551199999");
        assert_eq!(body["type"], "text");
        assert_eq!(body["text"]["body"], "hello");
        assert_eq!(body["text"]["preview_url"], false);
    }

    #[test]
    fn test_whatsapp_edit_message_prefix() {
        let new_text = "corrected version";
        let prefix = format!("(corrected) {new_text}");
        assert!(prefix.starts_with("(corrected)"));
        assert!(prefix.contains("corrected version"));
    }

    #[tokio::test]
    async fn test_whatsapp_start_stop_lifecycle() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_whatsapp_health_stopped() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }

    #[tokio::test]
    async fn test_whatsapp_health_running() {
        let adapter = WhatsAppAdapter::new("token", "phone-id");
        adapter.start().await.unwrap();
        let health = adapter.health();
        assert!(health.healthy);
        assert_eq!(health.message, "connected");
    }
}
