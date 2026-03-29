//! Protocol Module
//!
//! This module defines the core protocol types for message normalization
//! across all messaging platforms.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::Platform;

/// Authenticated user information from a messaging platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    /// Platform-specific user ID
    pub platform_user_id: String,
    /// Display name (if available)
    pub display_name: Option<String>,
    /// Username/handle (if available)
    pub username: Option<String>,
    /// Whether user is verified on the platform
    pub is_verified: bool,
    /// Whether user is a bot
    pub is_bot: bool,
}

impl AuthenticatedUser {
    /// Creates a new authenticated user
    pub fn new(platform_user_id: impl Into<String>) -> Self {
        Self {
            platform_user_id: platform_user_id.into(),
            display_name: None,
            username: None,
            is_verified: false,
            is_bot: false,
        }
    }

    /// Sets the display name
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Sets the username
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Marks the user as verified
    pub fn verified(mut self) -> Self {
        self.is_verified = true;
        self
    }

    /// Marks the user as a bot
    pub fn bot(mut self) -> Self {
        self.is_bot = true;
        self
    }
}

/// Normalized message format
///
/// All platform messages are normalized to this format before processing.
/// This ensures platform-agnostic command processing while preserving
/// essential metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMessage {
    /// Unique message identifier (UUID v4)
    pub id: String,

    /// Source platform
    pub platform: Platform,

    /// Authenticated user information
    pub user: AuthenticatedUser,

    /// Message content (text)
    pub content: String,

    /// Message timestamp (UTC)
    pub timestamp: DateTime<Utc>,

    /// Platform-specific metadata
    pub metadata: PlatformMetadata,
}

impl NormalizedMessage {
    /// Creates a new normalized message
    pub fn new(
        id: impl Into<String>,
        platform: Platform,
        user: AuthenticatedUser,
        content: impl Into<String>,
        timestamp: DateTime<Utc>,
        metadata: PlatformMetadata,
    ) -> Self {
        Self {
            id: id.into(),
            platform,
            user,
            content: content.into(),
            timestamp,
            metadata,
        }
    }

    /// Creates a new message with auto-generated ID and timestamp
    #[must_use]
    pub fn new_auto(
        platform: Platform,
        user: AuthenticatedUser,
        content: impl Into<String>,
        metadata: PlatformMetadata,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            platform,
            user,
            content: content.into(),
            timestamp: Utc::now(),
            metadata,
        }
    }

    /// Extract the chat/room/channel ID from platform-specific metadata.
    ///
    /// Returns a string suitable for use as the `chat_id` parameter in
    /// `MessagingGateway::process_message()` and `MessagingChannel::send_message()`.
    pub fn chat_id(&self) -> String {
        match &self.metadata {
            PlatformMetadata::Telegram { chat_id, .. } => chat_id.to_string(),
            PlatformMetadata::Discord { channel_id, .. } => channel_id.to_string(),
            PlatformMetadata::Matrix { room_id, .. } => room_id.clone(),
            PlatformMetadata::Signal { group_id, .. } => group_id.clone().unwrap_or_default(),
            PlatformMetadata::WhatsApp { phone_number, .. } => phone_number.clone(),
            PlatformMetadata::RocketChat { room_id, .. } => room_id.clone(),
            PlatformMetadata::Slack { channel_id, .. } => channel_id.clone(),
            PlatformMetadata::Webhook { .. } => String::new(),
        }
    }
}

/// Platform-specific metadata for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlatformMetadata {
    #[serde(rename = "telegram")]
    Telegram {
        chat_id: i64,
        message_id: i64,
        reply_to_message_id: Option<i64>,
    },
    #[serde(rename = "discord")]
    Discord {
        guild_id: u64,
        channel_id: u64,
        message_id: u64,
        referenced_message_id: Option<u64>,
    },
    #[serde(rename = "matrix")]
    Matrix {
        room_id: String,
        event_id: String,
        sender: String,
    },
    #[serde(rename = "signal")]
    Signal {
        group_id: Option<String>,
        timestamp: u64,
    },
    #[serde(rename = "whatsapp")]
    WhatsApp {
        phone_number: String,
        message_id: String,
        business_account_id: bool,
    },
    #[serde(rename = "rocket_chat")]
    RocketChat {
        room_id: String,
        message_id: String,
        user_id: String,
    },
    #[serde(rename = "slack")]
    Slack {
        team_id: String,
        channel_id: String,
        thread_ts: Option<String>,
        parent_message_ts: Option<String>,
    },
    #[serde(rename = "webhook")]
    Webhook {
        source_ip: String,
        headers: std::collections::HashMap<String, String>,
    },
}

impl Default for PlatformMetadata {
    fn default() -> Self {
        Self::Webhook {
            source_ip: String::new(),
            headers: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_user_builder() {
        let user = AuthenticatedUser::new("12345")
            .with_display_name("Test User")
            .with_username("testuser")
            .verified();

        assert_eq!(user.platform_user_id, "12345");
        assert_eq!(user.display_name, Some("Test User".to_string()));
        assert_eq!(user.username, Some("testuser".to_string()));
        assert!(user.is_verified);
        assert!(!user.is_bot);
    }

    #[test]
    fn test_normalized_message_auto() {
        let user = AuthenticatedUser::new("user-1");
        let metadata = PlatformMetadata::Telegram {
            chat_id: 12345,
            message_id: 67890,
            reply_to_message_id: None,
        };

        let msg = NormalizedMessage::new_auto(Platform::Telegram, user, "Hello, world!", metadata);

        assert!(!msg.id.is_empty());
        assert_eq!(msg.content, "Hello, world!");
    }

    #[test]
    fn test_chat_id_extraction() {
        // Telegram
        let msg = NormalizedMessage::new_auto(
            Platform::Telegram,
            AuthenticatedUser::new("42"),
            "hi",
            PlatformMetadata::Telegram {
                chat_id: 100,
                message_id: 1,
                reply_to_message_id: None,
            },
        );
        assert_eq!(msg.chat_id(), "100");

        // Discord
        let msg = NormalizedMessage::new_auto(
            Platform::Discord,
            AuthenticatedUser::new("42"),
            "hi",
            PlatformMetadata::Discord {
                guild_id: 1,
                channel_id: 999,
                message_id: 1,
                referenced_message_id: None,
            },
        );
        assert_eq!(msg.chat_id(), "999");

        // Matrix
        let msg = NormalizedMessage::new_auto(
            Platform::Matrix,
            AuthenticatedUser::new("@user:example.com"),
            "hi",
            PlatformMetadata::Matrix {
                room_id: "!room:example.com".into(),
                event_id: "$event".into(),
                sender: "@user:example.com".into(),
            },
        );
        assert_eq!(msg.chat_id(), "!room:example.com");

        // Slack
        let msg = NormalizedMessage::new_auto(
            Platform::Slack,
            AuthenticatedUser::new("U123"),
            "hi",
            PlatformMetadata::Slack {
                team_id: "T1".into(),
                channel_id: "C456".into(),
                thread_ts: None,
                parent_message_ts: None,
            },
        );
        assert_eq!(msg.chat_id(), "C456");

        // Signal (no group)
        let msg = NormalizedMessage::new_auto(
            Platform::Signal,
            AuthenticatedUser::new("+1234"),
            "hi",
            PlatformMetadata::Signal {
                group_id: None,
                timestamp: 0,
            },
        );
        assert_eq!(msg.chat_id(), "");

        // Signal (with group)
        let msg = NormalizedMessage::new_auto(
            Platform::Signal,
            AuthenticatedUser::new("+1234"),
            "hi",
            PlatformMetadata::Signal {
                group_id: Some("group1".into()),
                timestamp: 0,
            },
        );
        assert_eq!(msg.chat_id(), "group1");
    }
}
