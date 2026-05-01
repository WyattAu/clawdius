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

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
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
}

impl WhatsAppAdapter {
    /// Create a new WhatsApp adapter.
    #[must_use]
    pub fn new(
        access_token: impl Into<String>,
        phone_number_id: impl Into<String>,
    ) -> Self {
        Self {
            api_url: "https://graph.facebook.com/v21.0".to_string(),
            access_token: access_token.into(),
            phone_number_id: phone_number_id.into(),
            verify_token: None,
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
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
            .ok_or_else(|| {
                GatewayError::Config("WHATSAPP_ACCESS_TOKEN not set".to_string())
            })?;

        let phone_number_id = config
            .settings
            .get("phone_number_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                GatewayError::Config("WHATSAPP_PHONE_NUMBER_ID not set".to_string())
            })?;

        let mut adapter = Self::new(access_token, phone_number_id);
        adapter.verify_token = config
            .webhook_secret
            .clone();
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
                source: None,
            });
        }

        if let Some(ref verify_token) = self.verify_token {
            if token != verify_token {
                return Err(GatewayError::Adapter {
                    platform: "whatsapp".to_string(),
                    message: "webhook verification token mismatch".to_string(),
                    source: None,
                });
            }
        }

        Ok(challenge.to_string())
    }

    /// Parse an incoming WhatsApp webhook payload into `IncomingMessages`.
    pub fn parse_webhook_payload(
        &self,
        body: &serde_json::Value,
    ) -> Vec<IncomingMessage> {
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
                    let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");

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
                    let (user_id, user_name) = contacts
                        .first()
                        .map_or_else(
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
        let url = format!(
            "{}/messages",
            self.api_url.trim_end_matches('/')
        );

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
                source: None,
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(
        &self,
        _message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        // WhatsApp does NOT support message editing.
        // Send as a new message.
        let fallback = OutgoingMessage::new(
            Platform::WhatsApp,
            "fallback", // Caller should provide the correct chat_id
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
            errors: self
                .error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            last_message_at: None,
        }
    }
}
