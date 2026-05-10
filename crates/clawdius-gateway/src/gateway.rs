//! Core message gateway — routes messages between platforms and the agent.
//!
//! [`MessageGateway`] is the central routing layer that:
//! 1. Receives [`IncomingMessage`]s from platform adapters
//! 2. Applies rate limiting and authorization
//! 3. Routes to the agent engine
//! 4. Formats and delivers responses back to the platform

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

#[cfg(test)]
use crate::adapter::OutgoingMessage;
use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, Platform, PlatformAdapter, PlatformConfig,
};
use crate::error::GatewayError;
use crate::formatter::ResponseFormatter;
use crate::rate_limit::RateLimiter;

// ─────────────────────────────────────────────────────────
// MessageGateway
// ─────────────────────────────────────────────────────────

/// Central message router that connects platform adapters to the agent.
///
/// ```text
/// Platform Adapter → IncomingMessage → Gateway → [rate limit, auth] → Agent
/// Agent → Response → Gateway → [format, chunk] → Platform Adapter → OutgoingMessage
/// ```
pub struct MessageGateway {
    /// Registered platform adapters, keyed by platform.
    adapters: RwLock<HashMap<Platform, Arc<dyn PlatformAdapter>>>,

    /// Platform configurations.
    configs: HashMap<Platform, PlatformConfig>,

    /// Rate limiter.
    rate_limiter: Arc<RateLimiter>,

    /// Response formatter.
    formatter: ResponseFormatter,

    /// Handler for processing incoming messages.
    /// This will be connected to the Clawdius agent engine.
    message_handler: RwLock<Option<Box<dyn MessageHandler>>>,
}

/// Trait for handling incoming messages (agent engine interface).
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message and return a response.
    async fn handle_message(&self, message: IncomingMessage) -> Result<String, GatewayError>;
}

impl MessageGateway {
    /// Create a new message gateway.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
            configs: HashMap::new(),
            rate_limiter: Arc::new(RateLimiter::default_limiter()),
            formatter: ResponseFormatter::new(),
            message_handler: RwLock::new(None),
        }
    }

    /// Create a gateway with a custom rate limiter.
    #[must_use]
    pub fn with_rate_limiter(max_requests: usize, window_secs: u64) -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
            configs: HashMap::new(),
            rate_limiter: Arc::new(RateLimiter::new(max_requests, window_secs)),
            formatter: ResponseFormatter::new(),
            message_handler: RwLock::new(None),
        }
    }

    /// Register a platform adapter.
    pub async fn register_adapter(
        &mut self,
        adapter: impl PlatformAdapter + 'static,
        config: PlatformConfig,
    ) {
        let platform = adapter.platform();
        self.configs.insert(platform, config);
        self.adapters
            .write()
            .await
            .insert(platform, Arc::new(adapter));
    }

    /// Get a registered adapter by platform.
    pub async fn get_adapter(&self, platform: Platform) -> Option<Arc<dyn PlatformAdapter>> {
        self.adapters.read().await.get(&platform).cloned()
    }

    /// List all registered platforms.
    pub async fn registered_platforms(&self) -> Vec<Platform> {
        self.adapters.read().await.keys().copied().collect()
    }

    /// Set the message handler (agent engine connection).
    pub async fn set_handler(&self, handler: Box<dyn MessageHandler>) {
        let mut h = self.message_handler.write().await;
        *h = Some(handler);
    }

    /// Process an incoming message from any platform adapter.
    ///
    /// This is the main entry point called by adapters when they
    /// receive a new message.
    ///
    /// # Errors
    ///
    /// Returns `Err(GatewayError)` if the platform is not configured,
    /// user is not authorized, rate limit is exceeded, or handler fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn handle_incoming(&self, message: IncomingMessage) -> Result<(), GatewayError> {
        let platform = message.platform;

        // 1. Check if platform is configured
        let config = self
            .configs
            .get(&platform)
            .ok_or_else(|| GatewayError::PlatformNotConfigured(platform.to_string()))?;

        if !config.enabled {
            return Ok(());
        }

        // 2. Check user authorization
        if !config.is_user_allowed(&message.user.id) {
            return Err(GatewayError::Unauthorized(format!(
                "user {} not allowed on platform {}",
                message.user.id, platform
            )));
        }

        // 3. Check rate limit
        self.rate_limiter
            .check(platform, &message.user.id)
            .map_err(|e| GatewayError::RateLimited {
                user_id: message.user.id.clone(),
                platform: platform.to_string(),
                retry_after_ms: e.retry_after_ms,
            })?;

        // 4. Route to handler
        let response = {
            let handler = self.message_handler.read().await;
            let Some(handler) = handler.as_ref() else {
                return Err(GatewayError::Agent(
                    "no message handler registered".to_string(),
                ));
            };

            handler.handle_message(message.clone()).await?
        };

        // 5. Format and send response
        // Reply to the incoming message (or its parent if it was a reply)
        let reply_target = message.reply_to.as_deref().unwrap_or(&message.id);
        let messages = self.formatter.format_response(
            platform,
            &message.chat_id,
            &response,
            Some(reply_target),
        );

        if let Some(adapter) = self.get_adapter(platform).await {
            for msg in messages {
                adapter.send_message(msg).await?;
            }
        }

        Ok(())
    }

    /// Send a message directly to a platform (for proactive notifications).
    ///
    /// # Errors
    ///
    /// Returns `Err(GatewayError)` if the platform is not configured or sending fails.
    pub async fn send_to_platform(
        &self,
        platform: Platform,
        chat_id: &str,
        text: &str,
    ) -> Result<(), GatewayError> {
        let adapter = self
            .get_adapter(platform)
            .await
            .ok_or_else(|| GatewayError::PlatformNotConfigured(platform.to_string()))?;

        let messages = self
            .formatter
            .format_response(platform, chat_id, text, None);
        for msg in messages {
            adapter.send_message(msg).await?;
        }

        Ok(())
    }

    /// Get health status for all registered adapters.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn health_status(&self) -> HashMap<Platform, AdapterHealth> {
        let adapters = self.adapters.read().await;
        let mut status = HashMap::with_capacity(adapters.len());
        for (&platform, adapter) in adapters.iter() {
            status.insert(platform, adapter.health());
        }
        status
    }

    /// Start all registered adapters.
    ///
    /// For each adapter, sets a no-op message callback then calls
    /// [`PlatformAdapter::start`].
    /// To wire up the full routing callback, use [`Self::start_all_arc`] instead.
    ///
    /// **Prefer [`Self::start_all_arc`] when you have an `Arc<MessageGateway>`.**
    #[allow(clippy::significant_drop_tightening)]
    pub async fn start_all(&self) -> Vec<(Platform, Result<(), GatewayError>)> {
        let adapters = self.adapters.read().await;
        let mut results = Vec::with_capacity(adapters.len());

        for (&platform, adapter) in adapters.iter() {
            let config = self.configs.get(&platform);
            if config.is_none_or(|c| !c.enabled) {
                continue;
            }

            let callback: MessageCallback =
                Arc::new(|_msg: IncomingMessage| Box::pin(std::future::ready(())));

            adapter.set_message_callback(callback);

            let result = adapter.start().await;
            results.push((platform, result));
        }
        results
    }

    /// Start all registered adapters using an `Arc` reference.
    ///
    /// This is the preferred method when you hold an `Arc<MessageGateway>`,
    /// as it avoids raw pointer usage.
    pub async fn start_all_arc(self: &Arc<Self>) -> Vec<(Platform, Result<(), GatewayError>)> {
        let gateway = Arc::clone(self);
        let adapters = self.adapters.read().await;
        let mut results = Vec::with_capacity(adapters.len());

        for (&platform, adapter) in adapters.iter() {
            let config = self.configs.get(&platform);
            if config.is_none_or(|c| !c.enabled) {
                continue;
            }

            let callback: MessageCallback = Arc::new({
                let gateway = Arc::clone(&gateway);
                move |msg: IncomingMessage| {
                    let gw = Arc::clone(&gateway);
                    Box::pin(async move {
                        if let Err(e) = gw.handle_incoming(msg).await {
                            tracing::error!(platform = %platform, error = %e, "handle_incoming failed");
                        }
                    })
                }
            });

            adapter.set_message_callback(callback);

            let result = adapter.start().await;
            results.push((platform, result));
        }
        results
    }

    /// Stop all registered adapters.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn stop_all(&self) -> Vec<(Platform, Result<(), GatewayError>)> {
        let adapters = self.adapters.read().await;
        let mut results = Vec::with_capacity(adapters.len());

        for (&platform, adapter) in adapters.iter() {
            let result = adapter.stop().await;
            results.push((platform, result));
        }
        results
    }

    /// Get the rate limiter (for inspection/testing).
    #[must_use]
    pub const fn rate_limiter(&self) -> &Arc<RateLimiter> {
        &self.rate_limiter
    }

    /// Get the formatter (for customization).
    #[must_use]
    pub const fn formatter(&self) -> &ResponseFormatter {
        &self.formatter
    }
}

impl Default for MessageGateway {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::User;

    /// A mock message handler that echoes back the message.
    struct EchoHandler;

    #[async_trait]
    impl MessageHandler for EchoHandler {
        async fn handle_message(&self, message: IncomingMessage) -> Result<String, GatewayError> {
            Ok(format!("Echo: {}", message.text))
        }
    }

    /// A mock adapter for testing.
    struct MockAdapter {
        platform: Platform,
        running: std::sync::atomic::AtomicBool,
        sent_messages: Arc<tokio::sync::Mutex<Vec<OutgoingMessage>>>,
        message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
    }

    impl MockAdapter {
        fn new(platform: Platform) -> Self {
            Self {
                platform,
                running: std::sync::atomic::AtomicBool::new(false),
                sent_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                message_callback: Arc::new(tokio::sync::Mutex::new(None)),
            }
        }

        fn sent_messages(&self) -> Arc<tokio::sync::Mutex<Vec<OutgoingMessage>>> {
            Arc::clone(&self.sent_messages)
        }
    }

    #[async_trait::async_trait]
    impl PlatformAdapter for MockAdapter {
        fn platform(&self) -> Platform {
            self.platform
        }

        fn set_message_callback(&self, callback: MessageCallback) {
            let guard = self.message_callback.clone();
            tokio::spawn(async move {
                let mut cb = guard.lock().await;
                *cb = Some(callback);
            });
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
            _message_id: &str,
            _new_text: &str,
        ) -> Result<(), GatewayError> {
            Ok(())
        }

        async fn download_attachment(
            &self,
            _url: &str,
        ) -> Result<std::path::PathBuf, GatewayError> {
            Err(GatewayError::Adapter {
                platform: self.platform.to_string(),
                message: "mock adapter does not support downloads".to_string(),
                source: None,
            })
        }

        fn is_running(&self) -> bool {
            self.running.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn health(&self) -> AdapterHealth {
            AdapterHealth::default()
        }
    }

    fn make_incoming(platform: Platform, user_id: &str, text: &str) -> IncomingMessage {
        IncomingMessage {
            id: "msg001".to_string(),
            platform,
            chat_id: "chat001".to_string(),
            user: User {
                id: user_id.to_string(),
                name: "Test User".to_string(),
                username: None,
                is_admin: false,
            },
            text: text.to_string(),
            reply_to: None,
            attachments: Vec::new(),
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get_adapter() {
        let mut gateway = MessageGateway::new();
        let adapter = MockAdapter::new(Platform::Telegram);
        let config = PlatformConfig::new(Platform::Telegram);

        gateway.register_adapter(adapter, config).await;

        assert!(gateway.get_adapter(Platform::Telegram).await.is_some());
        assert!(gateway.get_adapter(Platform::Discord).await.is_none());
    }

    #[tokio::test]
    async fn test_registered_platforms() {
        let mut gateway = MessageGateway::new();
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Discord),
                PlatformConfig::new(Platform::Discord),
            )
            .await;

        let platforms = gateway.registered_platforms().await;
        assert_eq!(platforms.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_incoming_with_handler() {
        let mut gateway = MessageGateway::new();
        let adapter = MockAdapter::new(Platform::Telegram);
        let sent = adapter.sent_messages();
        gateway
            .register_adapter(adapter, PlatformConfig::new(Platform::Telegram))
            .await;
        gateway.set_handler(Box::new(EchoHandler)).await;

        let msg = make_incoming(Platform::Telegram, "user1", "hello");
        let result = gateway.handle_incoming(msg).await;

        assert!(result.is_ok());

        // Check that the response was sent to the adapter
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 1);
        assert!(messages[0].text.contains("Echo: hello"));
    }

    #[tokio::test]
    async fn test_handle_incoming_no_handler() {
        let mut gateway = MessageGateway::new();
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;

        let msg = make_incoming(Platform::Telegram, "user1", "hello");
        let result = gateway.handle_incoming(msg).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_incoming_user_not_allowed() {
        let mut gateway = MessageGateway::new();
        let mut config = PlatformConfig::new(Platform::Telegram);
        config.allowed_users = vec!["admin".to_string()];
        gateway
            .register_adapter(MockAdapter::new(Platform::Telegram), config)
            .await;
        gateway.set_handler(Box::new(EchoHandler)).await;

        let msg = make_incoming(Platform::Telegram, "random_user", "hello");
        let result = gateway.handle_incoming(msg).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_incoming_rate_limited() {
        let mut gateway = MessageGateway::with_rate_limiter(2, 60);
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;
        gateway.set_handler(Box::new(EchoHandler)).await;

        // Send 2 messages (at limit)
        for _ in 0..2 {
            let msg = make_incoming(Platform::Telegram, "user1", "hello");
            assert!(gateway.handle_incoming(msg).await.is_ok());
        }

        // 3rd should be rate limited
        let msg = make_incoming(Platform::Telegram, "user1", "hello");
        let result = gateway.handle_incoming(msg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_and_stop_all() {
        let mut gateway = MessageGateway::new();
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Discord),
                PlatformConfig::new(Platform::Discord),
            )
            .await;

        let start_results = gateway.start_all().await;
        assert_eq!(start_results.len(), 2);
        for (_, result) in &start_results {
            assert!(result.is_ok());
        }

        let stop_results = gateway.stop_all().await;
        assert_eq!(stop_results.len(), 2);
    }

    #[tokio::test]
    async fn test_health_status() {
        let mut gateway = MessageGateway::new();
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;

        let status = gateway.health_status().await;
        assert!(status.contains_key(&Platform::Telegram));
        assert!(status[&Platform::Telegram].healthy);
    }

    // ─────────────────────────────────────────────────────────
    // Cross-platform integration tests
    // ─────────────────────────────────────────────────────────

    /// A handler that returns platform-specific responses.
    struct PlatformAwareHandler;

    #[async_trait]
    impl MessageHandler for PlatformAwareHandler {
        async fn handle_message(&self, message: IncomingMessage) -> Result<String, GatewayError> {
            Ok(format!(
                "[{}] {} says: {}",
                message.platform, message.user.name, message.text
            ))
        }
    }

    /// A handler that returns a long message to test chunking.
    struct LongResponseHandler;

    #[async_trait]
    impl MessageHandler for LongResponseHandler {
        async fn handle_message(&self, _message: IncomingMessage) -> Result<String, GatewayError> {
            // Return a response longer than Discord's 2000 char limit
            Ok("X".repeat(3000))
        }
    }

    #[tokio::test]
    async fn test_multi_platform_routing() {
        let mut gateway = MessageGateway::new();

        // Register 3 platforms
        let telegram = MockAdapter::new(Platform::Telegram);
        let discord = MockAdapter::new(Platform::Discord);
        let slack = MockAdapter::new(Platform::Slack);

        let telegram_sent = telegram.sent_messages();
        let discord_sent = discord.sent_messages();
        let slack_sent = slack.sent_messages();

        gateway
            .register_adapter(telegram, PlatformConfig::new(Platform::Telegram))
            .await;
        gateway
            .register_adapter(discord, PlatformConfig::new(Platform::Discord))
            .await;
        gateway
            .register_adapter(slack, PlatformConfig::new(Platform::Slack))
            .await;

        gateway.set_handler(Box::new(PlatformAwareHandler)).await;

        // Send messages from different platforms
        let telegram_msg = make_incoming(Platform::Telegram, "u1", "hello from telegram");
        let discord_msg = make_incoming(Platform::Discord, "u2", "hello from discord");
        let slack_msg = make_incoming(Platform::Slack, "u3", "hello from slack");

        gateway.handle_incoming(telegram_msg).await.unwrap();
        gateway.handle_incoming(discord_msg).await.unwrap();
        gateway.handle_incoming(slack_msg).await.unwrap();

        // Verify each adapter received its own platform's response
        let t_msgs = telegram_sent.lock().await;
        let d_msgs = discord_sent.lock().await;
        let s_msgs = slack_sent.lock().await;

        assert_eq!(t_msgs.len(), 1);
        assert!(t_msgs[0].text.contains("telegram"));
        assert!(t_msgs[0].text.contains("hello from telegram"));

        assert_eq!(d_msgs.len(), 1);
        assert!(d_msgs[0].text.contains("discord"));
        assert!(d_msgs[0].text.contains("hello from discord"));

        assert_eq!(s_msgs.len(), 1);
        assert!(s_msgs[0].text.contains("slack"));
        assert!(s_msgs[0].text.contains("hello from slack"));
    }

    #[tokio::test]
    async fn test_long_response_chunked_per_platform() {
        let mut gateway = MessageGateway::new();

        // Discord has 2000 char limit → should chunk
        let discord = MockAdapter::new(Platform::Discord);
        let discord_sent = discord.sent_messages();
        gateway
            .register_adapter(discord, PlatformConfig::new(Platform::Discord))
            .await;

        // Slack has 40000 char limit → should NOT chunk
        let slack = MockAdapter::new(Platform::Slack);
        let slack_sent = slack.sent_messages();
        gateway
            .register_adapter(slack, PlatformConfig::new(Platform::Slack))
            .await;

        gateway.set_handler(Box::new(LongResponseHandler)).await;

        // Send same message to Discord → should be chunked
        let discord_msg = make_incoming(Platform::Discord, "u1", "chunk me");
        gateway.handle_incoming(discord_msg).await.unwrap();

        let d_msgs = discord_sent.lock().await;
        assert!(
            d_msgs.len() > 1,
            "Discord should chunk 3000 chars into multiple messages"
        );

        // Send same message to Slack → should NOT be chunked
        let slack_msg = make_incoming(Platform::Slack, "u1", "chunk me");
        gateway.handle_incoming(slack_msg).await.unwrap();

        let s_msgs = slack_sent.lock().await;
        assert_eq!(s_msgs.len(), 1, "Slack should NOT chunk 3000 chars");
    }

    #[tokio::test]
    async fn test_rate_limit_per_platform_independent() {
        let mut gateway = MessageGateway::with_rate_limiter(2, 60);

        let telegram = MockAdapter::new(Platform::Telegram);
        let discord = MockAdapter::new(Platform::Discord);

        gateway
            .register_adapter(telegram, PlatformConfig::new(Platform::Telegram))
            .await;
        gateway
            .register_adapter(discord, PlatformConfig::new(Platform::Discord))
            .await;

        gateway.set_handler(Box::new(EchoHandler)).await;

        // Exhaust Telegram rate limit
        for _ in 0..2 {
            let msg = make_incoming(Platform::Telegram, "u1", "hi");
            assert!(gateway.handle_incoming(msg).await.is_ok());
        }
        let msg = make_incoming(Platform::Telegram, "u1", "hi");
        assert!(gateway.handle_incoming(msg).await.is_err());

        // Discord should still work (independent rate limit)
        let msg = make_incoming(Platform::Discord, "u1", "hi");
        assert!(gateway.handle_incoming(msg).await.is_ok());
    }

    #[tokio::test]
    async fn test_send_to_platform_proactive() {
        let mut gateway = MessageGateway::new();

        let telegram = MockAdapter::new(Platform::Telegram);
        let telegram_sent = telegram.sent_messages();
        gateway
            .register_adapter(telegram, PlatformConfig::new(Platform::Telegram))
            .await;

        // Proactively send a message without an incoming trigger
        gateway
            .send_to_platform(Platform::Telegram, "chat123", "Proactive alert!")
            .await
            .unwrap();

        let msgs = telegram_sent.lock().await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "Proactive alert!");
        assert_eq!(msgs[0].chat_id, "chat123");
    }

    #[tokio::test]
    async fn test_all_nine_platforms_registered() {
        let mut gateway = MessageGateway::new();

        let platforms = [
            Platform::Telegram,
            Platform::Discord,
            Platform::Slack,
            Platform::Matrix,
            Platform::Signal,
            Platform::Teams,
            Platform::WhatsApp,
            Platform::RocketChat,
            Platform::Webhook,
        ];

        for &platform in &platforms {
            gateway
                .register_adapter(MockAdapter::new(platform), PlatformConfig::new(platform))
                .await;
        }

        let registered = gateway.registered_platforms().await;
        assert_eq!(registered.len(), 9);

        let health = gateway.health_status().await;
        assert_eq!(health.len(), 9);

        // Start all
        let results = gateway.start_all().await;
        assert_eq!(results.len(), 9);
        for (_, result) in &results {
            assert!(result.is_ok());
        }

        // Stop all
        let results = gateway.stop_all().await;
        assert_eq!(results.len(), 9);
    }

    #[tokio::test]
    async fn test_reply_to_preserved_in_response() {
        let mut gateway = MessageGateway::new();

        let telegram = MockAdapter::new(Platform::Telegram);
        let telegram_sent = telegram.sent_messages();
        gateway
            .register_adapter(telegram, PlatformConfig::new(Platform::Telegram))
            .await;

        gateway.set_handler(Box::new(EchoHandler)).await;

        // Incoming message with reply_to
        let mut msg = make_incoming(Platform::Telegram, "u1", "thanks!");
        msg.reply_to = Some("parent_msg_123".to_string());

        gateway.handle_incoming(msg).await.unwrap();

        let sent = telegram_sent.lock().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].reply_to.as_deref(), Some("parent_msg_123"));
    }

    #[tokio::test]
    async fn test_disabled_platform_ignored() {
        let mut gateway = MessageGateway::new();
        let mut config = PlatformConfig::new(Platform::Telegram);
        config.enabled = false;
        gateway
            .register_adapter(MockAdapter::new(Platform::Telegram), config)
            .await;
        gateway.set_handler(Box::new(EchoHandler)).await;

        let msg = make_incoming(Platform::Telegram, "user1", "hello");
        let result = gateway.handle_incoming(msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_to_unconfigured_platform() {
        let gateway = MessageGateway::new();
        let result = gateway
            .send_to_platform(Platform::Discord, "chat1", "hi")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_status_empty_gateway() {
        let gateway = MessageGateway::new();
        let status = gateway.health_status().await;
        assert!(status.is_empty());
    }

    #[tokio::test]
    async fn test_start_all_empty_gateway() {
        let gateway = MessageGateway::new();
        let results = gateway.start_all().await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_stop_all_empty_gateway() {
        let gateway = MessageGateway::new();
        let results = gateway.stop_all().await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_gateway_with_rate_limiter_custom() {
        let gateway = MessageGateway::with_rate_limiter(100, 300);
        assert_eq!(
            gateway
                .rate_limiter()
                .current_count(Platform::Telegram, "u1"),
            0
        );
    }

    #[tokio::test]
    async fn test_default_formatter_accessible() {
        let gateway = MessageGateway::new();
        assert!(gateway.formatter().preserve_code_blocks);
    }

    /// A handler that always fails.
    struct FailHandler;

    #[async_trait]
    impl MessageHandler for FailHandler {
        async fn handle_message(&self, _message: IncomingMessage) -> Result<String, GatewayError> {
            Err(GatewayError::Agent("handler failure".to_string()))
        }
    }

    #[tokio::test]
    async fn test_handler_error_propagates() {
        let mut gateway = MessageGateway::new();
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;
        gateway.set_handler(Box::new(FailHandler)).await;

        let msg = make_incoming(Platform::Telegram, "user1", "hello");
        let result = gateway.handle_incoming(msg).await;
        assert!(result.is_err());
    }

    /// A handler that returns an empty response.
    struct EmptyHandler;

    #[async_trait]
    impl MessageHandler for EmptyHandler {
        async fn handle_message(&self, _message: IncomingMessage) -> Result<String, GatewayError> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn test_empty_response_still_sent() {
        let mut gateway = MessageGateway::new();
        let adapter = MockAdapter::new(Platform::Telegram);
        let sent = adapter.sent_messages();
        gateway
            .register_adapter(adapter, PlatformConfig::new(Platform::Telegram))
            .await;
        gateway.set_handler(Box::new(EmptyHandler)).await;

        let msg = make_incoming(Platform::Telegram, "user1", "trigger");
        gateway.handle_incoming(msg).await.unwrap();

        let messages = sent.lock().await;
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_start_all_skips_disabled() {
        let mut gateway = MessageGateway::new();
        let mut config = PlatformConfig::new(Platform::Telegram);
        config.enabled = false;
        gateway
            .register_adapter(MockAdapter::new(Platform::Telegram), config)
            .await;
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Discord),
                PlatformConfig::new(Platform::Discord),
            )
            .await;

        let results = gateway.start_all().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, Platform::Discord);
    }

    #[tokio::test]
    async fn test_multiple_users_independent_rate_limits() {
        let mut gateway = MessageGateway::with_rate_limiter(1, 60);
        gateway
            .register_adapter(
                MockAdapter::new(Platform::Telegram),
                PlatformConfig::new(Platform::Telegram),
            )
            .await;
        gateway.set_handler(Box::new(EchoHandler)).await;

        let msg1 = make_incoming(Platform::Telegram, "user_a", "hi");
        assert!(gateway.handle_incoming(msg1).await.is_ok());

        let msg2 = make_incoming(Platform::Telegram, "user_a", "hi again");
        assert!(gateway.handle_incoming(msg2).await.is_err());

        let msg3 = make_incoming(Platform::Telegram, "user_b", "hello");
        assert!(gateway.handle_incoming(msg3).await.is_ok());
    }

    #[tokio::test]
    async fn test_replace_handler() {
        let mut gateway = MessageGateway::new();
        let adapter = MockAdapter::new(Platform::Telegram);
        let sent = adapter.sent_messages();
        gateway
            .register_adapter(adapter, PlatformConfig::new(Platform::Telegram))
            .await;

        gateway.set_handler(Box::new(EchoHandler)).await;
        let msg = make_incoming(Platform::Telegram, "u1", "first");
        gateway.handle_incoming(msg).await.unwrap();
        {
            let msgs = sent.lock().await;
            assert!(msgs[0].text.contains("Echo: first"));
        }

        gateway.set_handler(Box::new(PlatformAwareHandler)).await;
        let msg = make_incoming(Platform::Telegram, "u1", "second");
        gateway.handle_incoming(msg).await.unwrap();
        {
            let msgs = sent.lock().await;
            assert!(msgs[1].text.contains("[telegram]"));
        }
    }
}
