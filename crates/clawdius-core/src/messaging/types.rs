//! Messaging Gateway Types
//!
//! Core types for the multi-platform messaging gateway system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, MessagingError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagingError {
    SendFailed(String),
    ChannelUnavailable(String),
    InvalidConfig(String),
    InvalidCommandFormat { command: String, expected: String },
    Unauthorized { user_id: String, action: String },
    RateLimited { retry_after_secs: u64 },
    SessionNotFound(String),
    ParseError(String),
    AuthenticationFailed(String),
    ChannelNotSupported(String),
    MessageTooLong { length: usize, max: usize },
}

impl std::fmt::Display for MessagingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SendFailed(msg) => write!(f, "Send failed: {msg}"),
            Self::ChannelUnavailable(name) => write!(f, "Channel unavailable: {name}"),
            Self::InvalidConfig(msg) => write!(f, "Invalid config: {msg}"),
            Self::InvalidCommandFormat { command, expected } => {
                write!(f, "Invalid command '{command}' (expected: {expected})")
            }
            Self::Unauthorized { user_id, action } => {
                write!(f, "User '{user_id}' not authorized for '{action}'")
            }
            Self::RateLimited { retry_after_secs } => {
                write!(f, "Rate limited, retry after {retry_after_secs}s")
            }
            Self::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            Self::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {msg}"),
            Self::ChannelNotSupported(platform) => write!(f, "Channel not supported: {platform}"),
            Self::MessageTooLong { length, max } => {
                write!(f, "Message too long ({length} > {max})")
            }
        }
    }
}

impl std::error::Error for MessagingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Telegram,
    Discord,
    Matrix,
    Signal,
    RocketChat,
    WhatsApp,
    Slack,
    Webhook,
}

impl Platform {
    pub const fn command_prefix(&self) -> &'static str {
        match self {
            Self::Telegram => "/clawd ",
            Self::Discord => "/clawd ",
            Self::Matrix => "!clawd ",
            Self::Signal => "/clawd ",
            Self::RocketChat => "/clawd ",
            Self::WhatsApp => "/clawd ",
            Self::Slack => "/clawd ",
            Self::Webhook => "",
        }
    }

    pub const fn max_message_length(&self) -> usize {
        match self {
            Self::Telegram => 4096,
            Self::Discord => 2000,
            Self::Matrix => 65536,
            Self::Signal => 2000,
            Self::RocketChat => 5000,
            Self::WhatsApp => 4096,
            Self::Slack => 4000,
            Self::Webhook => usize::MAX,
        }
    }

    pub const fn supports_markdown(&self) -> bool {
        matches!(
            self,
            Self::Telegram | Self::Discord | Self::Matrix | Self::Slack
        )
    }

    pub const fn supports_threads(&self) -> bool {
        matches!(self, Self::Discord | Self::Slack | Self::Matrix)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Matrix => "matrix",
            Self::Signal => "signal",
            Self::RocketChat => "rocketchat",
            Self::WhatsApp => "whatsapp",
            Self::Slack => "slack",
            Self::Webhook => "webhook",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Platform {
    type Err = MessagingError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "telegram" => Ok(Self::Telegram),
            "discord" => Ok(Self::Discord),
            "matrix" => Ok(Self::Matrix),
            "signal" => Ok(Self::Signal),
            "rocketchat" | "rocket_chat" | "rocket-chat" => Ok(Self::RocketChat),
            "whatsapp" => Ok(Self::WhatsApp),
            "slack" => Ok(Self::Slack),
            "webhook" => Ok(Self::Webhook),
            _ => Err(MessagingError::ChannelNotSupported(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUserId {
    pub platform: Platform,
    pub user_id: String,
}

impl Default for PlatformUserId {
    fn default() -> Self {
        Self {
            platform: Platform::Webhook,
            user_id: String::new(),
        }
    }
}

impl PlatformUserId {
    pub fn new(platform: Platform, user_id: impl Into<String>) -> Self {
        Self {
            platform,
            user_id: user_id.into(),
        }
    }

    pub fn composite_key(&self) -> String {
        format!("{}:{}", self.platform.as_str(), self.user_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: Uuid,
    pub platform: Platform,
    pub user: PlatformUserId,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub reply_to: Option<Uuid>,
    pub thread_id: Option<String>,
}

impl IncomingMessage {
    pub fn new(platform: Platform, user: PlatformUserId, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            platform,
            user,
            content: content.into(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            reply_to: None,
            thread_id: None,
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn with_reply_to(mut self, msg_id: Uuid) -> Self {
        self.reply_to = Some(msg_id);
        self
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub id: Uuid,
    pub platform: Platform,
    pub content: String,
    pub reply_to: Option<Uuid>,
    pub thread_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub chunk_index: Option<usize>,
    pub total_chunks: Option<usize>,
}

impl OutgoingMessage {
    pub fn new(platform: Platform, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            platform,
            content: content.into(),
            reply_to: None,
            thread_id: None,
            metadata: HashMap::new(),
            chunk_index: None,
            total_chunks: None,
        }
    }

    pub fn reply_to(mut self, msg_id: Uuid) -> Self {
        self.reply_to = Some(msg_id);
        self
    }

    pub fn in_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    pub fn as_chunk(mut self, index: usize, total: usize) -> Self {
        self.chunk_index = Some(index);
        self.total_chunks = Some(total);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageChunk {
    pub content: String,
    pub index: usize,
    pub total: usize,
    pub is_last: bool,
}

impl MessageChunk {
    pub fn new(content: String, index: usize, total: usize) -> Self {
        Self {
            content,
            index,
            total,
            is_last: index == total - 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCategory {
    Status,
    Session,
    Timeline,
    Generate,
    Analyze,
    Config,
    Admin,
    Help,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCommand {
    pub raw: String,
    pub category: CommandCategory,
    pub action: String,
    pub args: Vec<String>,
    pub flags: HashMap<String, String>,
}

impl ParsedCommand {
    pub fn new(
        raw: impl Into<String>,
        category: CommandCategory,
        action: impl Into<String>,
    ) -> Self {
        Self {
            raw: raw.into(),
            category,
            action: action.into(),
            args: Vec::new(),
            flags: HashMap::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_flags(mut self, flags: HashMap<String, String>) -> Self {
        self.flags = flags;
        self
    }

    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(|s| s.as_str())
    }

    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(|s| s.as_str())
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessagingSession {
    pub id: Uuid,
    pub platform_user: PlatformUserId,
    pub clawdius_session_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub message_count: u64,
    pub state: SessionState,
    pub permissions: PermissionSet,
}

impl MessagingSession {
    /// Creates a new messaging session
    pub fn new(platform_user: PlatformUserId) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            platform_user,
            clawdius_session_id: None,
            created_at: now,
            last_activity: now,
            message_count: 0,
            state: SessionState::Active,
            permissions: PermissionSet::new(),
        }
    }

    /// Creates a new session with admin permissions
    pub fn new_admin(platform_user: PlatformUserId) -> Self {
        let mut session = Self::new(platform_user);
        session.permissions = PermissionSet::admin();
        session
    }

    /// Updates the last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
        self.message_count += 1;
    }

    /// Links to a Clawdius session
    pub fn link_clawdius_session(&mut self, session_id: Uuid) {
        self.clawdius_session_id = Some(session_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Active,
    Idle,
    Compacted,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionSet {
    pub can_generate: bool,
    pub can_analyze: bool,
    pub can_modify_files: bool,
    pub can_execute: bool,
    pub can_admin: bool,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self {
            can_generate: true,
            can_analyze: true,
            can_modify_files: false,
            can_execute: false,
            can_admin: false,
        }
    }

    pub fn admin() -> Self {
        Self {
            can_generate: true,
            can_analyze: true,
            can_modify_files: true,
            can_execute: true,
            can_admin: true,
        }
    }

    pub fn read_only() -> Self {
        Self {
            can_generate: false,
            can_analyze: true,
            can_modify_files: false,
            can_execute: false,
            can_admin: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_capacity: u32,
    pub tokens_per_refill: u32,
    pub refill_interval_ms: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 20,
            burst_capacity: 10,
            tokens_per_refill: 1,
            refill_interval_ms: 3000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub platform: Platform,
    pub enabled: bool,
    pub rate_limit: RateLimitConfig,
    pub command_whitelist: Option<Vec<String>>,
    pub admin_users: Vec<String>,
}

impl ChannelConfig {
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            enabled: true,
            rate_limit: RateLimitConfig::default(),
            command_whitelist: None,
            admin_users: Vec::new(),
        }
    }
}
