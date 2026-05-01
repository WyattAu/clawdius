//! Enhanced mock platform adapter for testing and development.
//!
//! This adapter simulates all platform operations in memory, providing:
//!
//! - **Recording**: All sent messages, edits, downloads are recorded for assertion
//! - **Simulation**: Inject fake incoming messages for testing handler logic
//! - **Configuration**: Controllable errors, latency, health state
//! - **Inspection**: Query recorded operations for test assertions
//!
//! # Example
//!
//! ```ignore
//! use clawdius_gateway::adapters::mock::MockPlatformAdapter;
//! use clawdius_gateway::adapter::{Platform, OutgoingMessage};
//!
//! let mock = MockPlatformAdapter::new(Platform::Discord);
//! mock.start().await?;
//!
//! // Simulate an incoming message
//! mock.inject_message("user1", "Hello, bot!", "channel1").await;
//!
//! // Assert the bot responded
//! let sent = mock.sent_messages().await;
//! assert_eq!(sent.len(), 1);
//! assert_eq!(sent[0].text, "Echo: Hello, bot!");
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
};
use crate::error::GatewayError;

/// Configuration for mock adapter behavior.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Whether to simulate network latency (in milliseconds).
    pub simulated_latency_ms: u64,

    /// Whether the adapter should report as unhealthy.
    pub force_unhealthy: bool,

    /// Whether `send_message` should fail.
    pub send_fails: bool,

    /// Whether `edit_message` should fail.
    pub edit_fails: bool,

    /// Whether `download_attachment` should fail.
    pub download_fails: bool,

    /// Custom error message for simulated failures.
    pub error_message: String,

    /// Maximum number of messages before send starts failing.
    pub max_messages: Option<usize>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            simulated_latency_ms: 0,
            force_unhealthy: false,
            send_fails: false,
            edit_fails: false,
            download_fails: false,
            error_message: "simulated failure".to_string(),
            max_messages: None,
        }
    }
}

/// A recorded operation for test inspection.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum RecordedOp {
    /// A message was sent.
    Sent(OutgoingMessage),
    /// A message was edited.
    Edit {
        message_id: String,
        new_text: String,
    },
    /// An attachment was downloaded.
    Download {
        url: String,
        path: std::path::PathBuf,
    },
}

/// Enhanced mock platform adapter for testing and development.
///
/// Stores all operations in memory for assertion in tests.
/// Can simulate incoming messages, errors, and latency.
pub struct MockPlatformAdapter {
    platform: Platform,
    running: std::sync::atomic::AtomicBool,
    sent_messages: Arc<tokio::sync::Mutex<Vec<OutgoingMessage>>>,
    edits: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    downloads: Arc<tokio::sync::Mutex<Vec<(String, std::path::PathBuf)>>>,
    all_ops: Arc<tokio::sync::Mutex<Vec<RecordedOp>>>,
    config: Arc<std::sync::RwLock<MockConfig>>,
    /// Callback for injected messages (wired by the gateway).
    on_incoming: tokio::sync::Mutex<
        Option<
            Arc<
                dyn Fn(IncomingMessage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                    + Send
                    + Sync,
            >,
        >,
    >,
}

impl MockPlatformAdapter {
    /// Create a new mock adapter for the given platform.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            running: std::sync::atomic::AtomicBool::new(false),
            sent_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            edits: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            downloads: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            all_ops: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            config: Arc::new(std::sync::RwLock::new(MockConfig::default())),
            on_incoming: tokio::sync::Mutex::new(None),
        }
    }

    /// Create a new mock adapter with custom configuration.
    #[must_use]
    pub fn with_config(platform: Platform, config: MockConfig) -> Self {
        Self {
            config: Arc::new(std::sync::RwLock::new(config)),
            ..Self::new(platform)
        }
    }

    /// Get a shared reference to sent messages for test assertions.
    #[must_use]
    pub fn sent_messages(&self) -> Arc<tokio::sync::Mutex<Vec<OutgoingMessage>>> {
        Arc::clone(&self.sent_messages)
    }

    /// Get a shared reference to edits for test assertions.
    #[must_use]
    pub fn edits(&self) -> Arc<tokio::sync::Mutex<Vec<(String, String)>>> {
        Arc::clone(&self.edits)
    }

    /// Get a shared reference to downloads for test assertions.
    #[must_use]
    pub fn downloads(&self) -> Arc<tokio::sync::Mutex<Vec<(String, std::path::PathBuf)>>> {
        Arc::clone(&self.downloads)
    }

    /// Get a shared reference to all recorded operations.
    #[must_use]
    pub fn all_ops(&self) -> Arc<tokio::sync::Mutex<Vec<RecordedOp>>> {
        Arc::clone(&self.all_ops)
    }

    /// Update the mock configuration.
    pub fn set_config(&self, config: MockConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get a copy of the current mock configuration.
    #[must_use]
    pub fn get_config(&self) -> MockConfig {
        self.config.read().unwrap().clone()
    }

    /// Simulate latency if configured.
    async fn simulate_latency(&self) {
        let latency = self.config.read().unwrap().simulated_latency_ms;
        if latency > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(latency)).await;
        }
    }

    /// Inject a simulated incoming message.
    ///
    /// This triggers the `on_incoming` callback if one has been set.
    /// Use this to test message handling without a real platform.
    pub async fn inject_message(
        &self,
        user_id: &str,
        text: &str,
        chat_id: &str,
    ) {
        let msg = IncomingMessage {
            id: format!("mock_{}", uuid::Uuid::new_v4()),
            platform: self.platform,
            chat_id: chat_id.to_string(),
            user: crate::adapter::User {
                id: user_id.to_string(),
                name: format!("TestUser_{user_id}"),
                username: Some(format!("testuser_{user_id}")),
                is_admin: false,
            },
            text: text.to_string(),
            reply_to: None,
            attachments: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let guard = self.on_incoming.lock().await;
        if let Some(ref handler) = *guard {
            handler(msg).await;
        }
    }

    /// Inject a simulated incoming message with a reply-to reference.
    pub async fn inject_reply(
        &self,
        user_id: &str,
        text: &str,
        chat_id: &str,
        reply_to: &str,
    ) {
        let msg = IncomingMessage {
            id: format!("mock_{}", uuid::Uuid::new_v4()),
            platform: self.platform,
            chat_id: chat_id.to_string(),
            user: crate::adapter::User {
                id: user_id.to_string(),
                name: format!("TestUser_{user_id}"),
                username: Some(format!("testuser_{user_id}")),
                is_admin: false,
            },
            text: text.to_string(),
            reply_to: Some(reply_to.to_string()),
            attachments: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let guard = self.on_incoming.lock().await;
        if let Some(ref handler) = *guard {
            handler(msg).await;
        }
    }

    /// Inject a simulated incoming message from an admin user.
    pub async fn inject_admin_message(
        &self,
        user_id: &str,
        text: &str,
        chat_id: &str,
    ) {
        let msg = IncomingMessage {
            id: format!("mock_{}", uuid::Uuid::new_v4()),
            platform: self.platform,
            chat_id: chat_id.to_string(),
            user: crate::adapter::User {
                id: user_id.to_string(),
                name: format!("AdminUser_{user_id}"),
                username: Some(format!("admin_{user_id}")),
                is_admin: true,
            },
            text: text.to_string(),
            reply_to: None,
            attachments: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let guard = self.on_incoming.lock().await;
        if let Some(ref handler) = *guard {
            handler(msg).await;
        }
    }

    /// Set the incoming message handler callback.
    pub async fn set_message_handler(
        &self,
        handler: Arc<
            dyn Fn(IncomingMessage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    ) {
        let mut guard = self.on_incoming.lock().await;
        *guard = Some(handler);
    }

    /// Reset all recorded state.
    pub async fn reset(&self) {
        self.sent_messages.lock().await.clear();
        self.edits.lock().await.clear();
        self.downloads.lock().await.clear();
        self.all_ops.lock().await.clear();
    }

    /// Get the count of sent messages.
    pub async fn sent_count(&self) -> usize {
        self.sent_messages.lock().await.len()
    }

    /// Get the count of edits.
    pub async fn edit_count(&self) -> usize {
        self.edits.lock().await.len()
    }

    /// Wait until at least N messages have been sent (with timeout).
    pub async fn wait_for_messages(&self, count: usize, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        loop {
            let current = self.sent_messages.lock().await.len();
            if current >= count {
                return true;
            }
            if start.elapsed().as_millis() > timeout_ms as u128 {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

#[async_trait]
impl PlatformAdapter for MockPlatformAdapter {
    fn platform(&self) -> Platform {
        self.platform
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
        self.simulate_latency().await;

        let (send_fails, error_message, max_messages) = {
            let config = self.config.read().unwrap();
            (config.send_fails, config.error_message.clone(), config.max_messages)
        };

        if send_fails {
            return Err(GatewayError::Adapter {
                platform: self.platform.as_str().to_string(),
                message: error_message,
                source: None,
            });
        }

        if let Some(max) = max_messages {
            let current = self.sent_messages.lock().await.len();
            if current >= max {
                return Err(GatewayError::Adapter {
                    platform: self.platform.as_str().to_string(),
                    message: format!("max messages ({max}) exceeded"),
                    source: None,
                });
            }
        }

        self.sent_messages.lock().await.push(message.clone());
        self.all_ops
            .lock()
            .await
            .push(RecordedOp::Sent(message));
        Ok(())
    }

    async fn edit_message(
        &self,
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        self.simulate_latency().await;

        let (edit_fails, error_message) = {
            let config = self.config.read().unwrap();
            (config.edit_fails, config.error_message.clone())
        };

        if edit_fails {
            return Err(GatewayError::Adapter {
                platform: self.platform.as_str().to_string(),
                message: error_message,
                source: None,
            });
        }

        self.edits
            .lock()
            .await
            .push((message_id.to_string(), new_text.to_string()));
        self.all_ops.lock().await.push(RecordedOp::Edit {
            message_id: message_id.to_string(),
            new_text: new_text.to_string(),
        });
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        self.simulate_latency().await;

        let (download_fails, error_message) = {
            let config = self.config.read().unwrap();
            (config.download_fails, config.error_message.clone())
        };

        if download_fails {
            return Err(GatewayError::Adapter {
                platform: self.platform.as_str().to_string(),
                message: error_message,
                source: None,
            });
        }

        let path = std::path::PathBuf::from(format!("/tmp/clawdius-mock/{}", url.split('/').last().unwrap_or("file")));
        self.downloads
            .lock()
            .await
            .push((url.to_string(), path.clone()));
        self.all_ops.lock().await.push(RecordedOp::Download {
            url: url.to_string(),
            path: path.clone(),
        });
        Ok(path)
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn health(&self) -> AdapterHealth {
        let config = self.config.read().unwrap();
        AdapterHealth {
            healthy: !config.force_unhealthy,
            message: if config.force_unhealthy {
                "simulated unhealthy".to_string()
            } else {
                "ok".to_string()
            },
            messages_processed: self.sent_messages.try_lock().map(|g| g.len() as u64).unwrap_or(0),
            errors: 0,
            last_message_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_send_and_record() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        mock.start().await.unwrap();

        let msg = OutgoingMessage::new(Platform::Discord, "ch1", "hello");
        mock.send_message(msg).await.unwrap();

        let sent = mock.sent_messages.lock().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].text, "hello");
    }

    #[tokio::test]
    async fn test_mock_edit_and_record() {
        let mock = MockPlatformAdapter::new(Platform::Telegram);
        mock.start().await.unwrap();

        mock.edit_message("msg1", "updated text").await.unwrap();

        let edits = mock.edits.lock().await;
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0], ("msg1".to_string(), "updated text".to_string()));
    }

    #[tokio::test]
    async fn test_mock_download_and_record() {
        let mock = MockPlatformAdapter::new(Platform::Slack);
        mock.start().await.unwrap();

        let path = mock.download_attachment("https://example.com/file.txt").await.unwrap();
        assert!(path.to_string_lossy().contains("file.txt"));

        let downloads = mock.downloads.lock().await;
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].0, "https://example.com/file.txt");
    }

    #[tokio::test]
    async fn test_mock_all_ops() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        mock.start().await.unwrap();

        mock.send_message(OutgoingMessage::new(Platform::Discord, "ch1", "hi"))
            .await
            .unwrap();
        mock.edit_message("m1", "bye").await.unwrap();

        let ops = mock.all_ops.lock().await;
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], RecordedOp::Sent(m) if m.text == "hi"));
        assert!(matches!(&ops[1], RecordedOp::Edit { message_id, .. } if message_id == "m1"));
    }

    #[tokio::test]
    async fn test_mock_send_fails() {
        let mut config = MockConfig::default();
        config.send_fails = true;
        config.error_message = "test error".to_string();

        let mock = MockPlatformAdapter::with_config(Platform::Discord, config);
        mock.start().await.unwrap();

        let result = mock
            .send_message(OutgoingMessage::new(Platform::Discord, "ch1", "hi"))
            .await;

        assert!(result.is_err());
        assert_eq!(mock.sent_count().await, 0);
    }

    #[tokio::test]
    async fn test_mock_max_messages() {
        let mut config = MockConfig::default();
        config.max_messages = Some(2);

        let mock = MockPlatformAdapter::with_config(Platform::Discord, config);
        mock.start().await.unwrap();

        mock.send_message(OutgoingMessage::new(Platform::Discord, "ch1", "a"))
            .await
            .unwrap();
        mock.send_message(OutgoingMessage::new(Platform::Discord, "ch1", "b"))
            .await
            .unwrap();
        let result = mock
            .send_message(OutgoingMessage::new(Platform::Discord, "ch1", "c"))
            .await;

        assert!(result.is_err());
        assert_eq!(mock.sent_count().await, 2);
    }

    #[tokio::test]
    async fn test_mock_force_unhealthy() {
        let mut config = MockConfig::default();
        config.force_unhealthy = true;

        let mock = MockPlatformAdapter::with_config(Platform::Discord, config);
        let health = mock.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "simulated unhealthy");
    }

    #[tokio::test]
    async fn test_mock_reset() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        mock.start().await.unwrap();

        mock.send_message(OutgoingMessage::new(Platform::Discord, "ch1", "a"))
            .await
            .unwrap();
        assert_eq!(mock.sent_count().await, 1);

        mock.reset().await;
        assert_eq!(mock.sent_count().await, 0);
    }

    #[tokio::test]
    async fn test_mock_stop_and_running() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        assert!(!mock.is_running());

        mock.start().await.unwrap();
        assert!(mock.is_running());

        mock.stop().await.unwrap();
        assert!(!mock.is_running());
    }

    #[tokio::test]
    async fn test_mock_inject_message() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        mock.start().await.unwrap();

        let received = Arc::new(tokio::sync::Mutex::new(None::<IncomingMessage>));
        let received_clone = Arc::clone(&received);

        mock.set_message_handler(Arc::new(move |msg| {
            let received = Arc::clone(&received_clone);
            Box::pin(async move {
                *received.lock().await = Some(msg);
            })
        }))
        .await;

        mock.inject_message("user1", "hello", "ch1").await;

        let received = received.lock().await;
        assert!(received.is_some());
        let msg = received.as_ref().unwrap();
        assert_eq!(msg.text, "hello");
        assert_eq!(msg.user.id, "user1");
        assert_eq!(msg.chat_id, "ch1");
    }

    #[tokio::test]
    async fn test_mock_inject_admin_message() {
        let mock = MockPlatformAdapter::new(Platform::Discord);
        mock.start().await.unwrap();

        let received = Arc::new(tokio::sync::Mutex::new(None::<IncomingMessage>));
        let received_clone = Arc::clone(&received);

        mock.set_message_handler(Arc::new(move |msg| {
            let received = Arc::clone(&received_clone);
            Box::pin(async move {
                *received.lock().await = Some(msg);
            })
        }))
        .await;

        mock.inject_admin_message("admin1", "admin command", "ch1").await;

        let received = received.lock().await;
        assert!(received.is_some());
        assert!(received.as_ref().unwrap().user.is_admin);
    }
}
