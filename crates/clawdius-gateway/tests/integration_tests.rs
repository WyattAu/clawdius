use std::collections::HashMap;
use std::sync::Arc;

use clawdius_gateway::adapter::{
    AdapterHealth, MessageCallback, Platform, PlatformAdapter, PlatformConfig,
};
use clawdius_gateway::gateway::MessageGateway;
use clawdius_gateway::rate_limit::RateLimiter;

#[test]
fn test_platform_from_str_telegram() {
    assert_eq!(Platform::from_str("telegram"), Some(Platform::Telegram));
}

#[test]
fn test_platform_from_str_discord() {
    assert_eq!(Platform::from_str("discord"), Some(Platform::Discord));
}

#[test]
fn test_platform_from_str_slack() {
    assert_eq!(Platform::from_str("slack"), Some(Platform::Slack));
}

#[test]
fn test_platform_from_str_matrix() {
    assert_eq!(Platform::from_str("matrix"), Some(Platform::Matrix));
}

#[test]
fn test_platform_from_str_webhook() {
    assert_eq!(Platform::from_str("webhook"), Some(Platform::Webhook));
}

#[test]
fn test_platform_from_str_signal() {
    assert_eq!(Platform::from_str("signal"), Some(Platform::Signal));
}

#[test]
fn test_platform_from_str_teams() {
    assert_eq!(Platform::from_str("teams"), Some(Platform::Teams));
}

#[test]
fn test_platform_from_str_whatsapp() {
    assert_eq!(Platform::from_str("whatsapp"), Some(Platform::WhatsApp));
}

#[test]
fn test_platform_from_str_rocketchat() {
    assert_eq!(Platform::from_str("rocketchat"), Some(Platform::RocketChat));
}

#[test]
fn test_platform_from_str_invalid() {
    assert_eq!(Platform::from_str("nonexistent"), None);
}

#[test]
fn test_platform_from_str_case_insensitive() {
    assert_eq!(Platform::from_str("TELEGRAM"), Some(Platform::Telegram));
    assert_eq!(Platform::from_str("Discord"), Some(Platform::Discord));
}

#[test]
fn test_platform_as_str_roundtrip() {
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
    for platform in &platforms {
        assert_eq!(Platform::from_str(platform.as_str()), Some(*platform));
    }
}

#[test]
fn test_platform_display() {
    assert_eq!(format!("{}", Platform::Telegram), "telegram");
    assert_eq!(format!("{}", Platform::Discord), "discord");
    assert_eq!(format!("{}", Platform::Slack), "slack");
}

#[test]
fn test_platform_max_message_length() {
    assert_eq!(Platform::Telegram.max_message_length(), 4096);
    assert_eq!(Platform::Discord.max_message_length(), 2000);
    assert_eq!(Platform::Slack.max_message_length(), 40_000);
    assert_eq!(Platform::Matrix.max_message_length(), 60_000);
    assert_eq!(Platform::Teams.max_message_length(), 20_000);
    assert_eq!(Platform::WhatsApp.max_message_length(), 65_536);
    assert_eq!(Platform::Webhook.max_message_length(), 1_000_000);
}

#[test]
fn test_platform_supports_markdown() {
    assert!(Platform::Discord.supports_markdown());
    assert!(Platform::Slack.supports_markdown());
    assert!(Platform::Matrix.supports_markdown());
    assert!(Platform::Telegram.supports_markdown());
    assert!(Platform::RocketChat.supports_markdown());
}

#[test]
fn test_rate_limiter_allows_under_limit() {
    let limiter = RateLimiter::new(10, 60);
    assert!(limiter.check(Platform::Telegram, "tenant1").is_ok());
}

#[test]
fn test_rate_limiter_allows_multiple_under_limit() {
    let limiter = RateLimiter::new(5, 60);
    for _ in 0..5 {
        assert!(limiter.check(Platform::Telegram, "tenant1").is_ok());
    }
}

#[test]
fn test_rate_limiter_blocks_over_limit() {
    let limiter = RateLimiter::new(3, 60);
    limiter.check(Platform::Telegram, "tenant1").ok();
    limiter.check(Platform::Telegram, "tenant1").ok();
    limiter.check(Platform::Telegram, "tenant1").ok();
    assert!(limiter.check(Platform::Telegram, "tenant1").is_err());
}

#[test]
fn test_rate_limiter_separate_tenants() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check(Platform::Telegram, "tenant1").ok();
    assert!(limiter.check(Platform::Telegram, "tenant2").is_ok());
}

#[test]
fn test_rate_limiter_separate_platforms() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check(Platform::Telegram, "user1").ok();
    assert!(limiter.check(Platform::Discord, "user1").is_ok());
}

#[test]
fn test_rate_limiter_error_has_retry_after() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check(Platform::Telegram, "user1").ok();
    let err = limiter.check(Platform::Telegram, "user1").unwrap_err();
    assert!(err.retry_after_ms > 0);
}

#[test]
fn test_rate_limiter_current_count() {
    let limiter = RateLimiter::new(10, 60);
    assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 0);
    limiter.check(Platform::Telegram, "user1").ok();
    assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 1);
    limiter.check(Platform::Telegram, "user1").ok();
    assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 2);
}

#[test]
fn test_rate_limiter_reset() {
    let limiter = RateLimiter::new(2, 60);
    limiter.check(Platform::Telegram, "user1").ok();
    limiter.check(Platform::Telegram, "user1").ok();
    assert!(limiter.check(Platform::Telegram, "user1").is_err());
    limiter.reset(Platform::Telegram, "user1");
    assert!(limiter.check(Platform::Telegram, "user1").is_ok());
}

#[test]
fn test_rate_limiter_clear_all() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check(Platform::Telegram, "u1").ok();
    limiter.check(Platform::Discord, "u2").ok();
    limiter.clear_all();
    assert!(limiter.check(Platform::Telegram, "u1").is_ok());
    assert!(limiter.check(Platform::Discord, "u2").is_ok());
}

#[test]
fn test_rate_limiter_default() {
    let limiter = RateLimiter::default_limiter();
    for _ in 0..20 {
        assert!(limiter.check(Platform::Telegram, "user1").is_ok());
    }
    assert!(limiter.check(Platform::Telegram, "user1").is_err());
}

struct MockAdapter {
    platform: Platform,
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
}

impl MockAdapter {
    fn new(platform: Platform) -> Self {
        Self {
            platform,
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
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

    async fn start(&self) -> Result<(), clawdius_gateway::GatewayError> {
        Ok(())
    }
    async fn stop(&self) -> Result<(), clawdius_gateway::GatewayError> {
        Ok(())
    }
    async fn send_message(
        &self,
        _message: clawdius_gateway::adapter::OutgoingMessage,
    ) -> Result<(), clawdius_gateway::GatewayError> {
        Ok(())
    }
    async fn edit_message(
        &self,
        _message_id: &str,
        _new_text: &str,
    ) -> Result<(), clawdius_gateway::GatewayError> {
        Ok(())
    }
    async fn download_attachment(
        &self,
        _url: &str,
    ) -> Result<std::path::PathBuf, clawdius_gateway::GatewayError> {
        Err(clawdius_gateway::GatewayError::Adapter {
            platform: self.platform.to_string(),
            message: "mock".to_string(),
            source: None,
        })
    }
    fn is_running(&self) -> bool {
        false
    }
    fn health(&self) -> AdapterHealth {
        AdapterHealth::default()
    }
}

#[tokio::test]
async fn test_gateway_new() {
    let gateway = MessageGateway::new();
    let platforms = gateway.registered_platforms().await;
    assert!(platforms.is_empty());
}

#[tokio::test]
async fn test_gateway_register_adapter() {
    let mut gateway = MessageGateway::new();
    gateway
        .register_adapter(
            MockAdapter::new(Platform::Webhook),
            PlatformConfig::new(Platform::Webhook),
        )
        .await;
    let platforms = gateway.registered_platforms().await;
    assert_eq!(platforms.len(), 1);
    assert!(platforms.contains(&Platform::Webhook));
}

#[tokio::test]
async fn test_gateway_register_multiple_adapters() {
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
async fn test_gateway_health_empty() {
    let gateway = MessageGateway::new();
    let health = gateway.health_status().await;
    assert!(health.is_empty());
}

#[tokio::test]
async fn test_gateway_health_with_adapter() {
    let mut gateway = MessageGateway::new();
    gateway
        .register_adapter(
            MockAdapter::new(Platform::Webhook),
            PlatformConfig::new(Platform::Webhook),
        )
        .await;
    let health = gateway.health_status().await;
    assert!(health.contains_key(&Platform::Webhook));
    assert!(health[&Platform::Webhook].healthy);
}

#[tokio::test]
async fn test_gateway_get_adapter() {
    let mut gateway = MessageGateway::new();
    gateway
        .register_adapter(
            MockAdapter::new(Platform::Telegram),
            PlatformConfig::new(Platform::Telegram),
        )
        .await;
    assert!(gateway.get_adapter(Platform::Telegram).await.is_some());
    assert!(gateway.get_adapter(Platform::Discord).await.is_none());
}

#[tokio::test]
async fn test_gateway_send_to_unconfigured_platform() {
    let gateway = MessageGateway::new();
    let result = gateway
        .send_to_platform(Platform::Discord, "chat1", "hi")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_gateway_start_stop_all_empty() {
    let gateway = MessageGateway::new();
    let start_results = gateway.start_all().await;
    assert!(start_results.is_empty());
    let stop_results = gateway.stop_all().await;
    assert!(stop_results.is_empty());
}

#[test]
fn test_platform_config_new() {
    let config = PlatformConfig::new(Platform::Telegram);
    assert!(config.enabled);
    assert!(config.api_token.is_none());
    assert!(config.allowed_users.is_empty());
}

#[test]
fn test_platform_config_with_token() {
    let config = PlatformConfig::with_token(Platform::Telegram, "my-token");
    assert_eq!(config.api_token, Some("my-token".to_string()));
    assert!(config.enabled);
}

#[test]
fn test_platform_config_user_allowlist() {
    let mut config = PlatformConfig::new(Platform::Telegram);
    config.allowed_users = vec!["user1".to_string()];
    assert!(config.is_user_allowed("user1"));
    assert!(!config.is_user_allowed("user2"));
}

#[test]
fn test_platform_config_empty_allowlist_allows_all() {
    let config = PlatformConfig::new(Platform::Telegram);
    assert!(config.is_user_allowed("anyone"));
}

#[test]
fn test_platform_config_admin_check() {
    let mut config = PlatformConfig::new(Platform::Discord);
    config.admin_users = vec!["admin1".to_string()];
    assert!(config.is_user_admin("admin1"));
    assert!(!config.is_user_admin("user2"));
}

#[test]
fn test_adapter_health_default() {
    let health = AdapterHealth::default();
    assert!(health.healthy);
    assert_eq!(health.message, "ok");
    assert_eq!(health.messages_processed, 0);
    assert_eq!(health.errors, 0);
    assert!(health.last_message_at.is_none());
}
