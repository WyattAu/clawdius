//! Platform adapter trait and message types.
//!
//! Each chat platform (Telegram, Discord, Slack, etc.) implements
//! [`PlatformAdapter`] to translate between platform-specific formats
//! and Clawdius's unified message types.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::GatewayError;

// ─────────────────────────────────────────────────────────
// Message types
// ─────────────────────────────────────────────────────────

/// A message received from a chat platform, normalized into a unified format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    /// Unique message identifier (platform-specific).
    pub id: String,

    /// The platform that sent this message.
    pub platform: Platform,

    /// The conversation/chat identifier on the platform.
    pub chat_id: String,

    /// The user who sent the message.
    pub user: User,

    /// The text content of the message.
    pub text: String,

    /// Optional reply-to message ID (for threaded conversations).
    pub reply_to: Option<String>,

    /// Optional file attachments.
    pub attachments: Vec<Attachment>,

    /// Timestamp when the message was sent.
    pub timestamp: DateTime<Utc>,

    /// Platform-specific metadata (e.g., Discord message flags, Telegram forward info).
    pub metadata: HashMap<String, serde_json::Value>,
}

/// An outgoing message to be sent to a chat platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// The platform to send to.
    pub platform: Platform,

    /// The conversation/chat identifier on the platform.
    pub chat_id: String,

    /// The text content to send.
    pub text: String,

    /// Whether this is a streamed chunk (partial update) or a complete message.
    pub is_chunk: bool,

    /// If `is_chunk`, the stream position for ordering.
    pub stream_position: Option<u64>,

    /// Optional reply-to message ID.
    pub reply_to: Option<String>,

    /// Optional file attachments to include.
    pub attachments: Vec<Attachment>,

    /// Platform-specific metadata (e.g., Markdown parse mode, embed flags).
    pub metadata: HashMap<String, serde_json::Value>,
}

impl OutgoingMessage {
    /// Create a complete (non-chunked) outgoing message.
    pub fn new(platform: Platform, chat_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            platform,
            chat_id: chat_id.into(),
            text: text.into(),
            is_chunk: false,
            stream_position: None,
            reply_to: None,
            attachments: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a streamed chunk of an outgoing message.
    pub fn chunk(
        platform: Platform,
        chat_id: impl Into<String>,
        text: impl Into<String>,
        position: u64,
    ) -> Self {
        Self {
            platform,
            chat_id: chat_id.into(),
            text: text.into(),
            is_chunk: true,
            stream_position: Some(position),
            reply_to: None,
            attachments: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set this message as a reply to another message.
    #[must_use]
    pub fn with_reply_to(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Add a metadata field.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// A user on a chat platform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct User {
    /// Platform-specific user identifier.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Optional username/handle.
    pub username: Option<String>,

    /// Whether this user is an admin/owner of the chat.
    pub is_admin: bool,
}

/// A file attachment in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Original filename.
    pub filename: String,

    /// MIME type.
    pub mime_type: String,

    /// URL to download the attachment (platform-specific).
    pub url: String,

    /// Size in bytes.
    pub size: usize,
}

/// Supported chat platforms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum Platform {
    Telegram,
    Discord,
    Slack,
    Matrix,
    Signal,
    Teams,
    WhatsApp,
    RocketChat,
    Webhook,
}

impl Platform {
    /// Get the platform identifier string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Slack => "slack",
            Self::Matrix => "matrix",
            Self::Signal => "signal",
            Self::Teams => "teams",
            Self::WhatsApp => "whatsapp",
            Self::RocketChat => "rocketchat",
            Self::Webhook => "webhook",
        }
    }

    /// Parse a platform identifier string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "telegram" => Some(Self::Telegram),
            "discord" => Some(Self::Discord),
            "slack" => Some(Self::Slack),
            "matrix" => Some(Self::Matrix),
            "signal" => Some(Self::Signal),
            "teams" => Some(Self::Teams),
            "whatsapp" => Some(Self::WhatsApp),
            "rocketchat" => Some(Self::RocketChat),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }

    /// Maximum message size in characters for this platform.
    #[must_use]
    pub fn max_message_length(&self) -> usize {
        match self {
            Self::Telegram => 4096,
            Self::Discord | Self::Signal => 2000,
            Self::Slack => 40_000,
            Self::Matrix => 60_000,
            Self::Teams | Self::RocketChat => 20_000,
            Self::WhatsApp => 65_536,
            Self::Webhook => 1_000_000,
        }
    }

    /// Whether this platform supports Markdown formatting.
    #[must_use]
    pub fn supports_markdown(&self) -> bool {
        matches!(
            self,
            Self::Discord | Self::Slack | Self::Matrix | Self::Telegram | Self::RocketChat
        )
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─────────────────────────────────────────────────────────
// Platform configuration
// ─────────────────────────────────────────────────────────

/// Configuration for a platform adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// The platform type.
    pub platform: Platform,

    /// Whether this platform adapter is enabled.
    pub enabled: bool,

    /// Platform-specific API token / bot token.
    pub api_token: Option<String>,

    /// Optional webhook URL (for outgoing webhooks).
    pub webhook_url: Option<String>,

    /// Optional webhook secret for signature verification.
    pub webhook_secret: Option<String>,

    /// Optional allowed user IDs (empty = allow all).
    pub allowed_users: Vec<String>,

    /// Optional admin user IDs.
    pub admin_users: Vec<String>,

    /// Platform-specific settings.
    pub settings: HashMap<String, serde_json::Value>,
}

impl PlatformConfig {
    /// Create a minimal config for a platform.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            enabled: true,
            api_token: None,
            webhook_url: None,
            webhook_secret: None,
            allowed_users: Vec::new(),
            admin_users: Vec::new(),
            settings: HashMap::new(),
        }
    }

    /// Create config with an API token.
    #[must_use]
    pub fn with_token(platform: Platform, token: impl Into<String>) -> Self {
        Self {
            api_token: Some(token.into()),
            ..Self::new(platform)
        }
    }

    /// Check if a user is allowed to interact with this adapter.
    #[must_use]
    pub fn is_user_allowed(&self, user_id: &str) -> bool {
        if self.allowed_users.is_empty() {
            return true;
        }
        self.allowed_users.iter().any(|u| u == user_id)
    }

    /// Check if a user is an admin.
    #[must_use]
    pub fn is_user_admin(&self, user_id: &str) -> bool {
        self.admin_users.iter().any(|u| u == user_id)
    }
}

// ─────────────────────────────────────────────────────────
// PlatformAdapter trait
// ─────────────────────────────────────────────────────────

/// Callback type for delivering incoming messages from adapters to the gateway.
pub type MessageCallback = Arc<
    dyn Fn(IncomingMessage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// Trait for connecting Clawdius to a chat platform.
///
/// Each platform (Telegram, Discord, Slack, etc.) implements this trait
/// to handle incoming messages and send outgoing responses.
///
/// # Lifecycle
///
/// ```text
/// new(config) → set_message_callback(cb) → start() → [receive messages, send responses] → stop()
/// ```
///
/// # Implementation
///
/// Adapters should be implemented as async, long-running tasks that
/// use the platform's SDK to receive webhooks or poll for updates.
/// The adapter translates platform-specific formats to/from the
/// unified [`IncomingMessage`]/[`OutgoingMessage`] types.
///
/// When an incoming message is received, the adapter must call the
/// message callback set via [`set_message_callback`](PlatformAdapter::set_message_callback)
/// to deliver it to the gateway for processing.
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// The platform this adapter handles.
    fn platform(&self) -> Platform;

    /// Set the callback for delivering incoming messages to the gateway.
    ///
    /// The gateway calls this before [`start`](PlatformAdapter::start).
    /// Adapters must call this callback for each incoming message.
    ///
    /// Uses interior mutability so it can be called through `Arc<dyn>`.
    fn set_message_callback(&self, callback: MessageCallback);

    /// Start the adapter's event loop.
    ///
    /// This should block (or run in a loop) until [`PlatformAdapter::stop`] is called
    /// or a fatal error occurs.
    ///
    /// Incoming messages are delivered via the callback set by
    /// [`set_message_callback`](PlatformAdapter::set_message_callback).
    async fn start(&self) -> Result<(), GatewayError>;

    /// Stop the adapter's event loop.
    async fn stop(&self) -> Result<(), GatewayError>;

    /// Send a message to the platform.
    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError>;

    /// Edit a previously sent message (for streaming updates).
    ///
    /// Returns `Ok(())` if the edit was successful, or an error if
    /// the platform doesn't support editing or the message wasn't found.
    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError>;

    /// Download an attachment by URL.
    ///
    /// Returns the downloaded file path in a temp directory.
    async fn download_attachment(&self, url: &str) -> Result<PathBuf, GatewayError>;

    /// Check if the adapter is currently running.
    fn is_running(&self) -> bool;

    /// Get the adapter's health status.
    fn health(&self) -> AdapterHealth;
}

/// Health status of a platform adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    /// Whether the adapter is connected and operational.
    pub healthy: bool,

    /// Human-readable status message.
    pub message: String,

    /// Number of messages processed since start.
    pub messages_processed: u64,

    /// Number of errors since start.
    pub errors: u64,

    /// Unix timestamp of the last successful message send.
    pub last_message_at: Option<DateTime<Utc>>,
}

impl Default for AdapterHealth {
    fn default() -> Self {
        Self {
            healthy: true,
            message: "ok".to_string(),
            messages_processed: 0,
            errors: 0,
            last_message_at: None,
        }
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_as_str() {
        assert_eq!(Platform::Telegram.as_str(), "telegram");
        assert_eq!(Platform::Discord.as_str(), "discord");
    }

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from_str("telegram"), Some(Platform::Telegram));
        assert_eq!(Platform::from_str("DISCORD"), Some(Platform::Discord));
        assert_eq!(Platform::from_str("unknown"), None);
    }

    #[test]
    fn test_platform_max_message_length() {
        assert!(Platform::Telegram.max_message_length() <= 4096);
        assert!(Platform::Discord.max_message_length() <= 2000);
        assert!(Platform::Slack.max_message_length() > Platform::Discord.max_message_length());
    }

    #[test]
    fn test_outgoing_message_new() {
        let msg = OutgoingMessage::new(Platform::Telegram, "chat123", "hello");
        assert_eq!(msg.platform, Platform::Telegram);
        assert_eq!(msg.chat_id, "chat123");
        assert_eq!(msg.text, "hello");
        assert!(!msg.is_chunk);
    }

    #[test]
    fn test_outgoing_message_chunk() {
        let msg = OutgoingMessage::chunk(Platform::Discord, "chat456", "partial", 1);
        assert!(msg.is_chunk);
        assert_eq!(msg.stream_position, Some(1));
    }

    #[test]
    fn test_outgoing_message_builder() {
        let msg = OutgoingMessage::new(Platform::Slack, "chat789", "reply")
            .with_reply_to("msg001")
            .with_metadata("parse_mode", serde_json::json!("full"));

        assert_eq!(msg.reply_to, Some("msg001".to_string()));
        assert_eq!(msg.metadata.get("parse_mode").unwrap(), "full");
    }

    #[test]
    fn test_platform_config_user_allowlist() {
        let mut config = PlatformConfig::new(Platform::Telegram);
        config.allowed_users = vec!["user1".to_string(), "user2".to_string()];

        assert!(config.is_user_allowed("user1"));
        assert!(config.is_user_allowed("user2"));
        assert!(!config.is_user_allowed("user3"));
    }

    #[test]
    fn test_platform_config_empty_allowlist() {
        let config = PlatformConfig::new(Platform::Telegram);
        assert!(config.is_user_allowed("anyone"));
    }

    #[test]
    fn test_platform_config_admin() {
        let mut config = PlatformConfig::new(Platform::Discord);
        config.admin_users = vec!["admin1".to_string()];

        assert!(config.is_user_admin("admin1"));
        assert!(!config.is_user_admin("user1"));
    }

    #[test]
    fn test_platform_config_with_token() {
        let config = PlatformConfig::with_token(Platform::Telegram, "bot123:token");
        assert_eq!(config.api_token, Some("bot123:token".to_string()));
        assert!(config.enabled);
    }

    #[test]
    fn test_adapter_health_default() {
        let health = AdapterHealth::default();
        assert!(health.healthy);
        assert_eq!(health.messages_processed, 0);
    }
}
