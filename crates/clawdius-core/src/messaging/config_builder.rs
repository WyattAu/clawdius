//! Configuration Builder
//!
//! Converts the TOML-deserializable [`crate::config::MessagingConfig`] into
//! the runtime types required by the webhook server and gateway:
//!
//! - [`WebhookServerConfig`](super::server::WebhookServerConfig)
//! - [`WebhookReceiver`](super::webhook_receiver::WebhookReceiver)
//! - [`ApiAuthenticator`](super::auth::ApiAuthenticator)
//!
//! # Usage
//!
//! ```rust,ignore
//! let config = Config::load_or_default();
//! let (server_config, receiver, auth) =
//!     build_webhook_infrastructure(&config.messaging);
//! ```

use crate::config::{MessagingConfig, WebhookPlatformConfig};

use super::auth::ApiAuthenticator;
use super::server::{WebhookServer, WebhookServerConfig};
use super::types::Platform;
use super::webhook_receiver::{
    DiscordWebhookConfig, MatrixWebhookConfig, RocketChatWebhookConfig, SignalWebhookConfig,
    SlackWebhookConfig, TelegramWebhookConfig, WebhookConfig, WebhookReceiver,
    WhatsAppWebhookConfig,
};

/// Result of [`build_webhook_infrastructure`].
pub struct WebhookInfrastructure {
    /// Pre-configured server config.
    pub server_config: WebhookServerConfig,
    /// Pre-configured webhook receiver with platform credentials.
    pub receiver: WebhookReceiver,
    /// Pre-configured API authenticator with keys.
    pub api_authenticator: ApiAuthenticator,
}

/// Build all runtime webhook infrastructure from a [`MessagingConfig`].
///
/// Returns the three components that `clawdius-server` needs:
/// 1. `WebhookServerConfig` — host, port, rate limits, routes
/// 2. `WebhookReceiver` — platform signature verification credentials
/// 3. `ApiAuthenticator` — global + per-platform API keys
///
/// Any missing optional fields are silently skipped (the server simply won't
/// accept requests from that platform).
pub fn build_webhook_infrastructure(msg: &MessagingConfig) -> WebhookInfrastructure {
    let mut api_authenticator = ApiAuthenticator::new();

    // Global API keys
    for key in &msg.global_api_keys {
        api_authenticator.add_global_key(key.clone());
    }

    // Per-platform API keys (key = platform name in snake_case)
    for (platform_str, keys) in &msg.api_keys {
        if let Some(platform) = parse_platform(platform_str) {
            for key in keys {
                api_authenticator.add_platform_key(platform, key.clone());
            }
        }
    }

    // Build webhook receiver from platform credentials
    let mut receiver = WebhookReceiver::new();
    for (platform_str, platform_cfg) in &msg.platforms {
        if let Some(_platform) = parse_platform(platform_str) {
            if let Some(webhook_cfg) = platform_to_webhook_config(platform_cfg) {
                receiver.register_platform(webhook_cfg, None);
            }
        }
    }

    let server_config = WebhookServerConfig {
        host: msg.host.clone(),
        port: msg.port,
        rate_limit_per_minute: msg.rate_limit_per_minute,
        max_request_size_bytes: msg.max_request_size_bytes,
        cors_origins: msg.cors_origins.clone(),
        routes: WebhookServerConfig::default_routes(),
        api_authenticator: api_authenticator.clone(),
        ip_allowlist: msg.ip_allowlist.clone(),
    };

    WebhookInfrastructure {
        server_config,
        receiver,
        api_authenticator,
    }
}

/// Create a fully-wired [`WebhookServer`] from [`MessagingConfig`].
///
/// This is a convenience wrapper around [`build_webhook_infrastructure`] that
/// combines the three components into a single `WebhookServer`.
pub fn build_webhook_server(msg: &MessagingConfig) -> WebhookServer {
    let infra = build_webhook_infrastructure(msg);
    WebhookServer::with_receiver(infra.server_config, infra.receiver)
}

/// Parse a platform name string (snake_case) into a [`Platform`].
fn parse_platform(s: &str) -> Option<Platform> {
    match s {
        "telegram" => Some(Platform::Telegram),
        "discord" => Some(Platform::Discord),
        "matrix" => Some(Platform::Matrix),
        "signal" => Some(Platform::Signal),
        "rocketchat" | "rocket_chat" => Some(Platform::RocketChat),
        "whatsapp" => Some(Platform::WhatsApp),
        "slack" => Some(Platform::Slack),
        _ => None,
    }
}

/// Convert a [`WebhookPlatformConfig`] into a [`WebhookConfig`] enum.
///
/// Returns `None` if the platform config doesn't have the required fields.
fn platform_to_webhook_config(cfg: &WebhookPlatformConfig) -> Option<WebhookConfig> {
    // Try to detect which platform this config is for based on which
    // fields are present. We need the platform name from the caller
    // to disambiguate — but this function is only called when we already
    // know the platform from the HashMap key. So we check ALL fields and
    // return the first match.

    if let Some(secret_token) = &cfg.secret_token {
        if secret_token.is_empty() {
            return None;
        }
        return Some(WebhookConfig::Telegram(TelegramWebhookConfig {
            secret_token: secret_token.clone(),
        }));
    }

    if let Some(public_key_pem) = &cfg.public_key_pem {
        if !public_key_pem.is_empty() {
            return Some(WebhookConfig::Discord(DiscordWebhookConfig {
                public_key_pem: public_key_pem.clone(),
            }));
        }
    }

    if let (Some(access_token), Some(homeserver_base_url)) =
        (&cfg.access_token, &cfg.homeserver_base_url)
    {
        if !access_token.is_empty() && !homeserver_base_url.is_empty() {
            return Some(WebhookConfig::Matrix(MatrixWebhookConfig {
                access_token: access_token.clone(),
                homeserver_base_url: homeserver_base_url.clone(),
            }));
        }
    }

    if let Some(signing_secret) = &cfg.signing_secret {
        if !signing_secret.is_empty() {
            return Some(WebhookConfig::Slack(SlackWebhookConfig {
                signing_secret: signing_secret.clone(),
            }));
        }
    }

    if let (Some(token), Some(user_id)) = (&cfg.token, &cfg.user_id) {
        if !token.is_empty() && !user_id.is_empty() {
            return Some(WebhookConfig::RocketChat(RocketChatWebhookConfig {
                token: token.clone(),
                user_id: user_id.clone(),
            }));
        }
    }

    if let Some(verification_token) = &cfg.verification_token {
        if !verification_token.is_empty() {
            return Some(WebhookConfig::Signal(SignalWebhookConfig {
                verification_token: verification_token.clone(),
            }));
        }
    }

    if let (Some(verify_token), Some(app_secret)) = (&cfg.verify_token, &cfg.app_secret) {
        if !verify_token.is_empty() && !app_secret.is_empty() {
            return Some(WebhookConfig::WhatsApp(WhatsAppWebhookConfig {
                verify_token: verify_token.clone(),
                app_secret: app_secret.clone(),
            }));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_msg_config() -> MessagingConfig {
        MessagingConfig::default()
    }

    #[test]
    fn empty_config_builds_valid_infrastructure() {
        let msg = empty_msg_config();
        let infra = build_webhook_infrastructure(&msg);

        assert_eq!(infra.server_config.host, "0.0.0.0");
        assert_eq!(infra.server_config.port, 8080);
        assert_eq!(infra.server_config.rate_limit_per_minute, 60);
        assert_eq!(infra.server_config.routes.len(), 7);
    }

    #[test]
    fn global_api_keys_are_loaded() {
        let msg = MessagingConfig {
            global_api_keys: vec!["master-key".to_string()],
            ..empty_msg_config()
        };
        let infra = build_webhook_infrastructure(&msg);

        assert!(matches!(
            infra
                .api_authenticator
                .validate(Platform::Telegram, Some("master-key")),
            super::super::auth::AuthResult::Authenticated { .. }
        ));
        assert!(matches!(
            infra
                .api_authenticator
                .validate(Platform::Discord, Some("master-key")),
            super::super::auth::AuthResult::Authenticated { .. }
        ));
    }

    #[test]
    fn platform_api_keys_are_scoped() {
        let mut api_keys = HashMap::new();
        api_keys.insert("telegram".to_string(), vec!["tg-only".to_string()]);

        let msg = MessagingConfig {
            api_keys,
            ..empty_msg_config()
        };
        let infra = build_webhook_infrastructure(&msg);

        assert!(matches!(
            infra
                .api_authenticator
                .validate(Platform::Telegram, Some("tg-only")),
            super::super::auth::AuthResult::Authenticated { .. }
        ));
        assert!(matches!(
            infra
                .api_authenticator
                .validate(Platform::Discord, Some("tg-only")),
            super::super::auth::AuthResult::InvalidKey
        ));
    }

    #[test]
    fn telegram_platform_credentials_register() {
        let mut platforms = HashMap::new();
        platforms.insert(
            "telegram".to_string(),
            WebhookPlatformConfig {
                secret_token: Some("tg-secret".to_string()),
                ..Default::default()
            },
        );

        let msg = MessagingConfig {
            platforms,
            ..empty_msg_config()
        };
        let infra = build_webhook_infrastructure(&msg);

        assert!(infra.receiver.is_registered(Platform::Telegram));
        assert!(!infra.receiver.is_registered(Platform::Discord));
    }

    #[test]
    fn parse_platform_all_variants() {
        assert_eq!(parse_platform("telegram"), Some(Platform::Telegram));
        assert_eq!(parse_platform("discord"), Some(Platform::Discord));
        assert_eq!(parse_platform("matrix"), Some(Platform::Matrix));
        assert_eq!(parse_platform("signal"), Some(Platform::Signal));
        assert_eq!(parse_platform("rocketchat"), Some(Platform::RocketChat));
        assert_eq!(parse_platform("rocket_chat"), Some(Platform::RocketChat));
        assert_eq!(parse_platform("whatsapp"), Some(Platform::WhatsApp));
        assert_eq!(parse_platform("slack"), Some(Platform::Slack));
        assert_eq!(parse_platform("unknown"), None);
    }

    #[test]
    fn is_configured_false_when_empty() {
        let msg = empty_msg_config();
        assert!(!msg.is_configured());
    }

    #[test]
    fn is_configured_true_with_global_key() {
        let msg = MessagingConfig {
            global_api_keys: vec!["key".to_string()],
            ..empty_msg_config()
        };
        assert!(msg.is_configured());
    }

    #[test]
    fn is_configured_true_with_platform_creds() {
        let mut platforms = HashMap::new();
        platforms.insert(
            "telegram".to_string(),
            WebhookPlatformConfig {
                secret_token: Some("s".to_string()),
                ..Default::default()
            },
        );
        let msg = MessagingConfig {
            platforms,
            ..empty_msg_config()
        };
        assert!(msg.is_configured());
    }

    #[test]
    fn build_webhook_server_convenience() {
        let msg = empty_msg_config();
        let server = build_webhook_server(&msg);
        assert_eq!(server.config().host, "0.0.0.0");
        assert_eq!(server.config().port, 8080);
    }

    #[test]
    fn full_toml_roundtrip() {
        let toml_str = r#"
[messaging]
host = "127.0.0.1"
port = 9090
global_api_keys = ["super-key"]

[messaging.api_keys]
telegram = ["tg-key-1", "tg-key-2"]
discord = ["dc-key"]

[messaging.platforms.telegram]
secret_token = "my-tg-secret"
bot_token = "123456:ABC-DEF"

[messaging.platforms.discord]
discord_bot_token = "discord-bot-token"

[messaging.platforms.matrix]
access_token = "syt_abc123"
homeserver_base_url = "https://matrix.org"

[messaging.platforms.slack]
signing_secret = "slack-secret"
slack_bot_token = "xoxb-slack-bot"

[messaging.platforms.rocketchat]
token = "rc-token"
user_id = "rc-user"
server_url = "https://chat.example.com"

[messaging.platforms.signal]
verification_token = "signal-verify"
signal_api_url = "http://localhost:8080"
signal_number = "+1234567890"

[messaging.platforms.whatsapp]
verify_token = "wa-verify"
app_secret = "wa-secret"
phone_number_id = "123456789"
whatsapp_access_token = "wa-token"
"#;

        let full: toml::Value = toml::from_str(toml_str).unwrap();
        let msg: MessagingConfig =
            serde_json::from_value(serde_json::to_value(&full["messaging"]).unwrap()).unwrap();

        assert_eq!(msg.host, "127.0.0.1");
        assert_eq!(msg.port, 9090);
        assert_eq!(msg.global_api_keys, vec!["super-key"]);
        assert_eq!(
            msg.api_keys.get("telegram").unwrap(),
            &vec!["tg-key-1", "tg-key-2"]
        );
        assert_eq!(
            msg.platforms
                .get("telegram")
                .unwrap()
                .secret_token
                .as_deref(),
            Some("my-tg-secret")
        );
        assert_eq!(
            msg.platforms.get("telegram").unwrap().bot_token.as_deref(),
            Some("123456:ABC-DEF")
        );
        assert_eq!(
            msg.platforms
                .get("discord")
                .unwrap()
                .discord_bot_token
                .as_deref(),
            Some("discord-bot-token")
        );
        assert_eq!(
            msg.platforms
                .get("slack")
                .unwrap()
                .slack_bot_token
                .as_deref(),
            Some("xoxb-slack-bot")
        );

        let infra = build_webhook_infrastructure(&msg);
        assert_eq!(infra.server_config.host, "127.0.0.1");
        assert_eq!(infra.server_config.port, 9090);
        assert!(infra.receiver.is_registered(Platform::Telegram));
        assert!(infra.receiver.is_registered(Platform::Matrix));
        assert!(!infra.receiver.is_registered(Platform::Discord));
    }
}
