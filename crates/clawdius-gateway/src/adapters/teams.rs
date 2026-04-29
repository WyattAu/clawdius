//! Microsoft Teams platform adapter using Bot Framework REST API.
//!
//! Connects Clawdius to Microsoft Teams via the Bot Framework Direct Line
//! or the Bot Framework REST API. Handles incoming messages from Teams
//! channels and delivers responses.
//!
//! # Setup
//!
//! 1. Register a bot in Azure Active Directory
//! 2. Create a Bot Channels Registration in Azure
//! 3. Set the Microsoft App ID and App Password in config
//! 4. Configure the Teams channel in the Azure Portal
//!
//! # Features
//!
//! - Text message sending and receiving
//! - Adaptive Card support (future)
//! - Proactive messaging (future)
//! - Teams-specific channel data extraction
//!
//! # Limitations
//!
//! - Message editing is limited in Teams (edits are visible but
//!   Teams doesn't notify users of edits)
//! - Requires Azure infrastructure for production use

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Teams adapter implementation using Bot Framework REST API.
pub struct TeamsAdapter {
    /// Bot Framework base URL.
    service_url: String,
    /// Microsoft App ID.
    app_id: String,
    /// Microsoft App Password.
    app_password: String,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// Cached OAuth access token.
    token: tokio::sync::Mutex<Option<String>>,
    /// Shared HTTP client.
    http: std::sync::OnceLock<reqwest::Client>,
}

impl TeamsAdapter {
    /// Create a new Teams adapter.
    #[must_use]
    pub fn new(
        service_url: impl Into<String>,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
    ) -> Self {
        Self {
            service_url: service_url.into(),
            app_id: app_id.into(),
            app_password: app_password.into(),
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            token: tokio::sync::Mutex::new(None),
            http: std::sync::OnceLock::new(),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let app_id = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("TEAMS_APP_ID not set".to_string()))?;

        let app_password = config
            .settings
            .get("app_password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                GatewayError::Config("TEAMS_APP_PASSWORD not set".to_string())
            })?;

        let service_url = config
            .settings
            .get("service_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://smba.trafficmanager.net/amer")
            .to_string();

        Ok(Self::new(service_url, app_id, app_password))
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Obtain an OAuth access token from the Bot Framework.
    async fn get_access_token(&self) -> Result<String, GatewayError> {
        // Check cache first
        {
            let cached = self.token.lock().await;
            if let Some(ref token) = *cached {
                return Ok(token.clone());
            }
        }

        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "client_id": self.app_id,
            "client_secret": self.app_password,
            "scope": "https://api.botframework.com/.default",
        });

        let response = self
            .http()
            .post("https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=client_credentials&client_id={}&client_secret={}&scope={}",
                self.app_id,
                self.app_password,
                "https://api.botframework.com/.default"
            ))
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("auth failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("failed to parse auth response: {e}"),
                source: Some(Box::new(e)),
            })?;

        let token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: "no access_token in auth response".to_string(),
                source: None,
            })?
            .to_string();

        // Cache the token
        let mut cached = self.token.lock().await;
        *cached = Some(token.clone());

        Ok(token)
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for TeamsAdapter {
    fn platform(&self) -> Platform {
        Platform::Teams
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
        let token = self.get_access_token().await?;

        // message_id format: "service_url:conversation_id"
        let (service_url, conversation_id) = if message.metadata.contains_key("service_url") {
            (
                message.metadata["service_url"].as_str().unwrap_or(&self.service_url).to_string(),
                message.chat_id.clone(),
            )
        } else {
            // Try splitting from chat_id if it contains service_url
            let parts: Vec<&str> = message.chat_id.splitn(2, ':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (self.service_url.clone(), message.chat_id.clone())
            }
        };

        let body = serde_json::json!({
            "type": "message",
            "from": {
                "id": self.app_id,
            },
            "conversation": {
                "id": conversation_id,
            },
            "text": message.text,
        });

        let url = format!("{service_url}/v3/conversations/{conversation_id}/activities");

        let response = self
            .http()
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("send failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "teams".to_string(),
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
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        // message_id format: "service_url:conversation_id:activity_id"
        let parts: Vec<&str> = message_id.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!(
                    "invalid message_id format (expected service_url:conversation_id:activity_id): {message_id}"
                ),
                source: None,
            });
        }

        let (service_url, conversation_id, activity_id) =
            (parts[0], parts[1], parts[2]);

        let token = self.get_access_token().await?;

        let url = format!(
            "{service_url}/v3/conversations/{conversation_id}/activities/{activity_id}"
        );

        let body = serde_json::json!({
            "type": "message",
            "text": new_text,
        });

        let response = self
            .http()
            .put(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("edit failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("edit failed with {status}: {text}"),
                source: None,
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        let token = self.get_access_token().await?;

        let response = self
            .http()
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "teams".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("teams-attachment");

        let dir = std::env::temp_dir().join("clawdius-teams");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "teams".to_string(),
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
