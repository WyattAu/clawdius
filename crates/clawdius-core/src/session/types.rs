//! Session types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Unique session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Create a new random session ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    /// Session title (auto-generated or user-set)
    pub title: Option<String>,
    /// Messages in the session
    pub messages: Vec<Message>,
    /// Session metadata
    pub meta: SessionMeta,
    /// Token usage tracking
    pub token_usage: TokenUsage,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            title: None,
            messages: Vec::new(),
            meta: SessionMeta::default(),
            token_usage: TokenUsage::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create with provider and model
    pub fn with_provider_model(provider: String, model: String) -> Self {
        let mut session = Self::new();
        session.meta.provider = Some(provider);
        session.meta.model = Some(model);
        session
    }

    /// Add a message to the session
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Get the last message
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get messages by role
    pub fn messages_by_role(&self, role: MessageRole) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(move |m| m.role == role)
    }

    /// Calculate total tokens used
    pub fn total_tokens(&self) -> usize {
        self.token_usage.total()
    }

    /// Touch the session (update timestamp)
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMeta {
    /// LLM provider
    pub provider: Option<String>,
    /// Model used
    pub model: Option<String>,
    /// Working directory when session started
    pub working_dir: Option<String>,
    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Custom metadata
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A message in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: Uuid,
    /// Message role
    pub role: MessageRole,
    /// Message content
    pub content: MessageContent,
    /// Token count for this message
    pub tokens: Option<usize>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Tool calls (if any)
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Metadata
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: MessageContent::Text(content.into()),
            tokens: None,
            created_at: Utc::now(),
            tool_calls: Vec::new(),
            metadata: serde_json::Map::new(),
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: MessageContent::Text(content.into()),
            tokens: None,
            created_at: Utc::now(),
            tool_calls: Vec::new(),
            metadata: serde_json::Map::new(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::System,
            content: MessageContent::Text(content.into()),
            tokens: None,
            created_at: Utc::now(),
            tool_calls: Vec::new(),
            metadata: serde_json::Map::new(),
        }
    }

    /// Get text content
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool result
    Tool,
}

/// Message content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content
    Text(String),
    /// Multi-part content (text + images)
    MultiPart(Vec<ContentPart>),
}

/// Part of a multi-part message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    /// Text part
    Text {
        /// The text content
        text: String,
    },
    /// Image part
    Image {
        /// Image source
        source: ImageSource,
    },
}

/// Image source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ImageSource {
    /// Base64 encoded image
    Base64 {
        /// Media type (e.g., "image/png")
        media_type: String,
        /// Base64 encoded data
        data: String,
    },
    /// URL to image
    Url {
        /// Image URL
        url: String,
    },
}

/// Tool call in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments
    pub arguments: serde_json::Value,
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens used
    pub input: usize,
    /// Output tokens used
    pub output: usize,
    /// Cached tokens (if any)
    pub cached: usize,
}

impl TokenUsage {
    /// Total tokens used
    pub fn total(&self) -> usize {
        self.input + self.output
    }

    /// Add usage from another
    pub fn add(&mut self, other: &TokenUsage) {
        self.input += other.input;
        self.output += other.output;
        self.cached += other.cached;
    }
}
