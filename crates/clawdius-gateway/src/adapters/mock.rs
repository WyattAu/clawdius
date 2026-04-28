//! Mock platform adapter for testing and development.
//!
//! Stores sent messages in memory for assertion in tests.
//! Simulates all platform operations without any network calls.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
};
use crate::error::GatewayError;

/// Mock adapter that records all operations in memory.
pub struct MockPlatformAdapter {
    platform: Platform,
    running: std::sync::atomic::AtomicBool,
    sent_messages: Arc<tokio::sync::Mutex<Vec<OutgoingMessage>>>,
    edits: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    downloads: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
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
    pub fn downloads(&self) -> Arc<tokio::sync::Mutex<Vec<(String, String)>>> {
        Arc::clone(&self.downloads)
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
        self.sent_messages.lock().await.push(message);
        Ok(())
    }

    async fn edit_message(
        &self,
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        self.edits
            .lock()
            .await
            .push((message_id.to_string(), new_text.to_string()));
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        self.downloads
            .lock()
            .await
            .push((url.to_string(), "mock-download".to_string()));
        Ok(std::path::PathBuf::from("/tmp/clawdius-mock"))
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::default()
    }
}
