//! Notification Gateway
//!
//! Multi-channel notification system for alerts and signals.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result type for notification operations.
pub type Result<T> = std::result::Result<T, NotificationError>;

/// Notification error types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationError {
    /// Failed to send notification
    SendFailed(String),
    /// Channel is unavailable
    ChannelUnavailable(String),
    /// Invalid configuration
    InvalidConfig(String),
}

impl std::fmt::Display for NotificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SendFailed(msg) => write!(f, "Send failed: {msg}"),
            Self::ChannelUnavailable(name) => write!(f, "Channel unavailable: {name}"),
            Self::InvalidConfig(msg) => write!(f, "Invalid config: {msg}"),
        }
    }
}

impl std::error::Error for NotificationError {}

/// Trait for notification channels.
#[async_trait]
pub trait NotificationChannel: Send + Sync {
    /// Sends a notification message.
    async fn send(&self, message: &str) -> Result<()>;

    /// Returns the channel name.
    fn name(&self) -> &str;
}

/// Webhook-based notification channel.
pub struct WebhookChannel {
    name: String,
    url: String,
    client: reqwest::Client,
}

impl WebhookChannel {
    /// Creates a new webhook channel.
    #[must_use]
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl NotificationChannel for WebhookChannel {
    async fn send(&self, message: &str) -> Result<()> {
        let response = self
            .client
            .post(&self.url)
            .json(&serde_json::json!({ "text": message }))
            .send()
            .await
            .map_err(|e| NotificationError::SendFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(NotificationError::SendFailed(format!(
                "HTTP {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Matrix notification channel.
pub struct MatrixChannel {
    name: String,
    homeserver: String,
    room_id: String,
    access_token: String,
    client: reqwest::Client,
}

impl MatrixChannel {
    /// Creates a new Matrix channel.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        homeserver: impl Into<String>,
        room_id: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            homeserver: homeserver.into(),
            room_id: room_id.into(),
            access_token: access_token.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl NotificationChannel for MatrixChannel {
    async fn send(&self, message: &str) -> Result<()> {
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message?access_token={}",
            self.homeserver, self.room_id, self.access_token
        );

        let txn_id = uuid::Uuid::new_v4().to_string();
        let url = format!("{url}&txn_id={txn_id}");

        let response = self
            .client
            .put(&url)
            .json(&serde_json::json!({
                "msgtype": "m.text",
                "body": message
            }))
            .send()
            .await
            .map_err(|e| NotificationError::SendFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(NotificationError::SendFailed(format!(
                "HTTP {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Notification gateway that broadcasts to multiple channels.
pub struct NotificationGateway {
    channels: Arc<RwLock<Vec<Box<dyn NotificationChannel>>>>,
}

impl NotificationGateway {
    /// Creates a new notification gateway.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Adds a notification channel.
    pub async fn add_channel(&self, channel: Box<dyn NotificationChannel>) {
        self.channels.write().await.push(channel);
    }

    /// Broadcasts a message to all channels.
    pub async fn broadcast(&self, message: &str) -> Vec<(String, Result<()>)> {
        let channels = self.channels.read().await;
        let mut results = Vec::new();

        for channel in channels.iter() {
            let name = channel.name().to_string();
            let result = channel.send(message).await;
            results.push((name, result));
        }

        results
    }

    /// Returns the number of registered channels.
    pub async fn channel_count(&self) -> usize {
        self.channels.read().await.len()
    }
}

impl Default for NotificationGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockChannel {
        name: String,
        should_fail: bool,
    }

    #[async_trait]
    impl NotificationChannel for MockChannel {
        async fn send(&self, _message: &str) -> Result<()> {
            if self.should_fail {
                Err(NotificationError::SendFailed("mock failure".to_string()))
            } else {
                Ok(())
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_notification_gateway() {
        let gateway = NotificationGateway::new();
        gateway
            .add_channel(Box::new(MockChannel {
                name: "test1".to_string(),
                should_fail: false,
            }))
            .await;
        gateway
            .add_channel(Box::new(MockChannel {
                name: "test2".to_string(),
                should_fail: false,
            }))
            .await;

        assert_eq!(gateway.channel_count().await, 2);
    }
}
