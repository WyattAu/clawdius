//! Messaging Channel Trait and Adapters
//!
//! Defines the interface for messaging platform channels and provides
//! implementations for various messaging platforms.

use async_trait::async_trait;

use crate::messaging::types::{Platform, Result};

mod discord;
mod matrix;
mod rocketchat;
mod signal;
mod slack;
mod telegram;
mod whatsapp;

// Re-export channel implementations
pub use discord::DiscordChannel;
pub use matrix::MatrixChannel;
pub use rocketchat::RocketChatChannel;
pub use signal::SignalChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use whatsapp::WhatsAppChannel;

pub use crate::messaging::oauth::OAuthTokenStore;

/// Trait for messaging platform channels
///
/// All messaging platform adapters must implement this trait to integrate
/// with the messaging gateway.
///
/// # OAuth Token Support
///
/// Channel adapters that support [`OAuthTokenStore`] expose a
/// `with_token_store` builder method. When a token store is provided, the
/// adapter calls [`OAuthTokenStore::get_bot_token`] before each API request
/// and uses the stored token (falling back to the static token if the store
/// is absent or returns `None`).
#[async_trait]
pub trait MessagingChannel: Send + Sync {
    /// Returns the platform this channel handles
    fn platform(&self) -> Platform;

    /// Sends a message to a recipient
    ///
    /// # Arguments
    /// * `recipient` - Platform-specific recipient identifier (chat_id, channel_id, room_id, etc.)
    /// * `text` - Message content to send
    ///
    /// # Returns
    /// Platform-specific message ID on success
    async fn send_message(&self, recipient: &str, text: &str) -> Result<String>;

    /// Sends multiple message chunks
    ///
    /// Default implementation sends chunks sequentially. Override for platform-specific
    /// batching behavior.
    async fn send_chunks(&self, recipient: &str, chunks: &[String]) -> Result<Vec<String>> {
        let mut results = Vec::new();
        for chunk in chunks {
            let id = self.send_message(recipient, chunk).await?;
            results.push(id);
        }
        Ok(results)
    }

    /// Edits an existing message (for streaming progressive updates).
    ///
    /// Default implementation falls back to sending a new message.
    /// Override for platforms that support in-place message editing.
    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let _ = message_id;
        self.send_message(chat_id, new_text).await
    }

    /// Whether this platform supports in-place message editing.
    fn supports_edit(&self) -> bool {
        false
    }

    /// Checks if the channel is connected and authenticated
    ///
    /// Default returns `true`. Override to implement actual connectivity checks.
    async fn is_connected(&self) -> bool {
        true
    }
}

/// Mock channel for testing and development.
///
/// Returns a fake UUID message ID for every `send_message` call.
/// Suitable for use without real platform credentials.
pub struct MockChannel {
    platform: Platform,
}

impl MockChannel {
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
}

#[async_trait]
impl MessagingChannel for MockChannel {
    fn platform(&self) -> Platform {
        self.platform
    }

    async fn send_message(&self, _recipient: &str, _text: &str) -> Result<String> {
        Ok(uuid::Uuid::new_v4().to_string())
    }

    async fn edit_message(
        &self,
        _chat_id: &str,
        message_id: &str,
        _new_text: &str,
    ) -> Result<String> {
        Ok(format!("edited-{}", message_id))
    }

    fn supports_edit(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_platforms_have_channels() {
        let platforms = [
            Platform::Telegram,
            Platform::Discord,
            Platform::Matrix,
            Platform::Signal,
            Platform::WhatsApp,
            Platform::RocketChat,
            Platform::Slack,
            Platform::Webhook,
        ];

        for platform in platforms {
            let channel = MockChannel::new(platform);
            assert_eq!(channel.platform(), platform);
        }
    }
}
