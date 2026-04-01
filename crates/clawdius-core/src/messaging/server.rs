use std::collections::HashSet;
use std::net::IpAddr;

use super::auth::{ApiAuthenticator, AuthResult};
use super::protocol::NormalizedMessage;
use super::types::{MessagingError, Platform};
use super::webhook_receiver::{VerificationResult, WebhookReceiver, WebhookRequest};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub struct WebhookRoute {
    pub platform: Platform,
    pub path: String,
    pub methods: Vec<HttpMethod>,
}

#[derive(Debug, Clone)]
pub struct WebhookServerConfig {
    pub host: String,
    pub port: u16,
    pub routes: Vec<WebhookRoute>,
    pub api_authenticator: ApiAuthenticator,
    pub cors_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub max_request_size_bytes: usize,
    /// Optional IP allowlist. If non-empty, only requests from these CIDRs are accepted.
    pub ip_allowlist: Vec<String>,
}

impl Default for WebhookServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            routes: Self::default_routes(),
            api_authenticator: ApiAuthenticator::new(),
            cors_origins: Vec::new(),
            rate_limit_per_minute: 60,
            max_request_size_bytes: 1_000_000,
            ip_allowlist: Vec::new(),
        }
    }
}

impl WebhookServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_routes() -> Vec<WebhookRoute> {
        vec![
            WebhookRoute {
                platform: Platform::Telegram,
                path: "/webhook/telegram".into(),
                methods: vec![HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::Discord,
                path: "/webhook/discord".into(),
                methods: vec![HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::Matrix,
                path: "/webhook/matrix/:room_id".into(),
                methods: vec![HttpMethod::Put, HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::Slack,
                path: "/webhook/slack".into(),
                methods: vec![HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::RocketChat,
                path: "/webhook/rocketchat".into(),
                methods: vec![HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::Signal,
                path: "/webhook/signal".into(),
                methods: vec![HttpMethod::Post],
            },
            WebhookRoute {
                platform: Platform::WhatsApp,
                path: "/webhook/whatsapp".into(),
                methods: vec![HttpMethod::Get, HttpMethod::Post],
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct WebhookResponse {
    pub status: u16,
    pub body: String,
    pub content_type: String,
}

impl WebhookResponse {
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            content_type: "application/json".to_string(),
        }
    }

    pub fn error(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            content_type: "application/json".to_string(),
        }
    }
}

/// Result from `WebhookServer::process_webhook`.
///
/// `Rejected` means auth/signature/parse failed — send the response as-is.
/// `Parsed` means the message was successfully extracted and can be
/// routed to `MessagingGateway::process_message()`.
pub enum WebhookResult {
    /// Authentication, signature, or parse failed. Send the response back.
    Rejected(WebhookResponse),
    /// Successfully parsed into a normalized message.
    Parsed(NormalizedMessage),
}

pub struct WebhookServer {
    config: WebhookServerConfig,
    receiver: WebhookReceiver,
}

fn cidr_contains_ip(cidr: &str, ip: &str) -> bool {
    if !cidr.contains('/') {
        return cidr == ip;
    }

    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        tracing::warn!(cidr = %cidr, "Invalid CIDR format, skipping");
        return false;
    }
    let network_ip = parts[0].trim();
    let prefix_len: u8 = match parts[1].trim().parse() {
        Ok(n) => n,
        Err(_) => {
            tracing::warn!(cidr = %cidr, "Invalid CIDR prefix, skipping");
            return false;
        },
    };

    let network_parts: Vec<u8> = network_ip
        .split('.')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let ip_parts: Vec<u8> = ip
        .split('.')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if network_parts.len() != 4 || ip_parts.len() != 4 {
        return false;
    }

    let full_octets = (prefix_len / 8) as usize;
    let remaining_bits = prefix_len % 8;

    for i in 0..full_octets {
        if ip_parts[i] != network_parts[i] {
            return false;
        }
    }

    if remaining_bits > 0 && full_octets < 4 {
        let mask = (!(1u8 << (8 - remaining_bits))) & 0xFF;
        if ip_parts[full_octets] & mask != network_parts[full_octets] & mask {
            return false;
        }
    }

    true
}

impl WebhookServer {
    pub fn new(config: WebhookServerConfig) -> Self {
        Self {
            config,
            receiver: WebhookReceiver::new(),
        }
    }

    pub fn with_receiver(config: WebhookServerConfig, receiver: WebhookReceiver) -> Self {
        Self { config, receiver }
    }

    pub fn config(&self) -> &WebhookServerConfig {
        &self.config
    }

    pub fn build_routes(&self) -> &[WebhookRoute] {
        &self.config.routes
    }

    pub fn is_ip_allowed(&self, remote_ip: Option<&str>) -> bool {
        let allowlist = &self.config.ip_allowlist;
        if allowlist.is_empty() {
            return true;
        }
        let ip_str = match remote_ip {
            Some(s) => s.trim(),
            None => return true,
        };

        let parsed: IpAddr = match ip_str.parse() {
            Ok(addr) => addr,
            Err(_) => return true,
        };

        if parsed.is_ipv6() {
            tracing::debug!(ip = %parsed, "IPv6 address — CIDR allowlist only supports IPv4");
            return true;
        }

        for cidr in allowlist {
            if cidr_contains_ip(cidr.trim(), ip_str) {
                return true;
            }
        }
        false
    }

    pub fn handle_webhook(
        &self,
        request: &WebhookRequest,
        api_key: Option<&str>,
    ) -> WebhookResponse {
        match self.process_webhook(request, api_key) {
            WebhookResult::Rejected(response) => response,
            WebhookResult::Parsed(msg) => {
                let body = serde_json::json!({
                    "status": "ok",
                    "message_id": msg.id,
                    "platform": msg.platform.as_str(),
                    "user_id": msg.user.platform_user_id,
                })
                .to_string();
                WebhookResponse::ok(body)
            },
        }
    }

    /// Authenticate, verify signature, and parse a webhook request.
    ///
    /// Unlike `handle_webhook` (which returns a ready-to-send response),
    /// this returns a `WebhookResult::Parsed(NormalizedMessage)` on success,
    /// allowing the caller to route the message to `MessagingGateway` or
    /// other processing before responding.
    ///
    /// Returns `WebhookResult::Rejected(WebhookResponse)` for auth failures,
    /// signature mismatches, or parse errors — the response should be sent
    /// back to the caller as-is.
    pub fn process_webhook(
        &self,
        request: &WebhookRequest,
        api_key: Option<&str>,
    ) -> WebhookResult {
        // 1. API key authentication
        match self
            .config
            .api_authenticator
            .validate(request.platform, api_key)
        {
            AuthResult::Authenticated { .. } => {},
            AuthResult::MissingKey => {
                return WebhookResult::Rejected(WebhookResponse::error(
                    401,
                    r#"{"error":"Missing API key"}"#,
                ));
            },
            AuthResult::InvalidKey => {
                return WebhookResult::Rejected(WebhookResponse::error(
                    403,
                    r#"{"error":"Invalid API key"}"#,
                ));
            },
        }

        // 2. Platform registration check
        if !self.receiver.is_registered(request.platform) {
            let platforms: HashSet<String> = self
                .receiver
                .registered_platforms()
                .iter()
                .map(|p| p.to_string())
                .collect();
            let registered_list = if platforms.is_empty() {
                "none".to_string()
            } else {
                platforms.into_iter().collect::<Vec<_>>().join(", ")
            };
            return WebhookResult::Rejected(WebhookResponse::error(
                400,
                format!(
                    r#"{{"error":"Platform '{}' not registered","registered_platforms":"{}"}}"#,
                    request.platform, registered_list
                ),
            ));
        }

        // 3. Signature verification
        match self.receiver.verify_signature(request) {
            VerificationResult::Verified => {},
            VerificationResult::InvalidSignature => {
                return WebhookResult::Rejected(WebhookResponse::error(
                    401,
                    r#"{"error":"Invalid webhook signature"}"#,
                ));
            },
            VerificationResult::MissingCredentials => {
                return WebhookResult::Rejected(WebhookResponse::error(
                    401,
                    r#"{"error":"Missing webhook credentials"}"#,
                ));
            },
            VerificationResult::UnsupportedPlatform => {
                return WebhookResult::Rejected(WebhookResponse::error(
                    400,
                    r#"{"error":"Unsupported platform"}"#,
                ));
            },
        }

        // 4. Body parsing
        match self.receiver.parse_webhook_body(request) {
            Ok(msg) => WebhookResult::Parsed(msg),
            Err(e) => {
                let error_body = serde_json::json!({
                    "status": "error",
                    "error": e.to_string(),
                })
                .to_string();
                let response = match e {
                    MessagingError::AuthenticationFailed(_) => {
                        WebhookResponse::error(401, error_body)
                    },
                    MessagingError::ParseError(_) => WebhookResponse::error(400, error_body),
                    _ => WebhookResponse::error(500, error_body),
                };
                WebhookResult::Rejected(response)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::webhook_receiver::{TelegramWebhookConfig, WebhookConfig};
    use super::*;

    fn default_config() -> WebhookServerConfig {
        WebhookServerConfig::new()
    }

    #[test]
    fn default_routes_cover_all_platforms() {
        let routes = WebhookServerConfig::default_routes();
        let platforms: HashSet<Platform> = routes.iter().map(|r| r.platform).collect();
        assert!(platforms.contains(&Platform::Telegram));
        assert!(platforms.contains(&Platform::Discord));
        assert!(platforms.contains(&Platform::Matrix));
        assert!(platforms.contains(&Platform::Slack));
        assert!(platforms.contains(&Platform::RocketChat));
        assert!(platforms.contains(&Platform::Signal));
        assert!(platforms.contains(&Platform::WhatsApp));
        assert_eq!(platforms.len(), 7);
    }

    #[test]
    fn default_config_values() {
        let config = default_config();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.rate_limit_per_minute, 60);
        assert_eq!(config.max_request_size_bytes, 1_000_000);
        assert!(config.cors_origins.is_empty());
        assert_eq!(config.routes.len(), 7);
    }

    #[test]
    fn whatsapp_route_has_get_and_post() {
        let routes = WebhookServerConfig::default_routes();
        let wa = routes
            .iter()
            .find(|r| r.platform == Platform::WhatsApp)
            .unwrap();
        assert!(wa.methods.contains(&HttpMethod::Get));
        assert!(wa.methods.contains(&HttpMethod::Post));
    }

    #[test]
    fn matrix_route_has_put_and_post() {
        let routes = WebhookServerConfig::default_routes();
        let mx = routes
            .iter()
            .find(|r| r.platform == Platform::Matrix)
            .unwrap();
        assert!(mx.methods.contains(&HttpMethod::Put));
        assert!(mx.methods.contains(&HttpMethod::Post));
    }

    #[test]
    fn build_routes_returns_config_routes() {
        let server = WebhookServer::new(default_config());
        let routes = server.build_routes();
        assert_eq!(routes.len(), 7);
    }

    #[test]
    fn handle_webhook_missing_auth_returns_401() {
        let server = WebhookServer::new(default_config());
        let req = WebhookRequest::new(Platform::Telegram, b"{}".to_vec());
        let resp = server.handle_webhook(&req, None);
        assert_eq!(resp.status, 401);
    }

    #[test]
    fn handle_webhook_invalid_key_returns_403() {
        let server = WebhookServer::new(default_config());
        let req = WebhookRequest::new(Platform::Telegram, b"{}".to_vec());
        let resp = server.handle_webhook(&req, Some("bad_key"));
        assert_eq!(resp.status, 403);
    }

    #[test]
    fn handle_webhook_valid_auth_unregistered_platform_returns_400() {
        let mut config = default_config();
        config.api_authenticator.add_global_key("test_key".into());
        let server = WebhookServer::new(config);
        let req = WebhookRequest::new(Platform::Telegram, b"{}".to_vec());
        let resp = server.handle_webhook(&req, Some("test_key"));
        assert_eq!(resp.status, 400);
        assert!(resp.body.contains("not registered"));
    }

    #[test]
    fn handle_webhook_registered_platform_missing_signature_returns_401() {
        let mut receiver = WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "tg_secret".into(),
            }),
            None,
        );

        let mut config = default_config();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "api_key".into());

        let server = WebhookServer::with_receiver(config, receiver);
        let req = WebhookRequest::new(Platform::Telegram, b"{}".to_vec());
        let resp = server.handle_webhook(&req, Some("api_key"));
        assert_eq!(resp.status, 401);
    }

    #[test]
    fn handle_webhook_full_pipeline_ok() {
        let mut receiver = WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "tg_secret".into(),
            }),
            None,
        );

        let mut config = default_config();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "api_key".into());

        let server = WebhookServer::with_receiver(config, receiver);

        let body = serde_json::json!({
            "update_id": 1,
            "message": {
                "message_id": 10,
                "from": { "id": 42, "is_bot": false, "first_name": "A" },
                "chat": { "id": 42 },
                "text": "hello",
                "date": 1700000000
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Telegram, body)
            .with_query_param("secret_token", "tg_secret");

        let resp = server.handle_webhook(&req, Some("api_key"));
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("ok"));
    }

    #[test]
    fn webhook_response_ok() {
        let resp = WebhookResponse::ok(r#"{"status":"ok"}"#);
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "application/json");
    }

    #[test]
    fn webhook_response_error() {
        let resp = WebhookResponse::error(500, "oops");
        assert_eq!(resp.status, 500);
    }

    #[test]
    fn config_returns_reference() {
        let config = default_config();
        let server = WebhookServer::new(config);
        assert_eq!(server.config().port, 8080);
    }

    #[test]
    fn test_ip_allowlist_empty_allows_all() {
        let server = WebhookServer::new(WebhookServerConfig::default());
        assert!(server.is_ip_allowed(Some("1.2.3.4")));
        assert!(server.is_ip_allowed(None));
    }

    #[test]
    fn test_ip_allowlist_blocks_unlisted() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["10.0.0.0/8".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("10.1.2.3")));
        assert!(!server.is_ip_allowed(Some("192.168.1.1")));
    }

    #[test]
    fn test_ip_allowlist_exact_ip() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["1.2.3.4".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("1.2.3.4")));
        assert!(!server.is_ip_allowed(Some("1.2.3.5")));
    }

    #[test]
    fn test_ip_allowlist_cidr_24() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["192.168.1.0/24".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("192.168.1.0")));
        assert!(server.is_ip_allowed(Some("192.168.1.255")));
        assert!(server.is_ip_allowed(Some("192.168.1.128")));
        assert!(!server.is_ip_allowed(Some("192.168.2.1")));
    }

    #[test]
    fn test_ip_allowlist_cidr_16() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["172.16.0.0/16".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("172.16.5.5")));
        assert!(!server.is_ip_allowed(Some("172.17.0.0")));
    }

    #[test]
    fn test_ip_allowlist_malformed_ip_allowed() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["10.0.0.0/8".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("not-an-ip")));
    }

    #[test]
    fn test_ip_allowlist_ipv6_passthrough() {
        let mut config = WebhookServerConfig::default();
        config.ip_allowlist = vec!["10.0.0.0/8".to_string()];
        let server = WebhookServer::new(config);
        assert!(server.is_ip_allowed(Some("::1")));
    }

    #[test]
    fn test_cidr_contains_ip_exact() {
        assert!(cidr_contains_ip("1.2.3.4", "1.2.3.4"));
        assert!(!cidr_contains_ip("1.2.3.4", "1.2.3.5"));
    }

    #[test]
    fn test_cidr_contains_ip_prefix_24() {
        assert!(cidr_contains_ip("192.168.1.0/24", "192.168.1.100"));
        assert!(!cidr_contains_ip("192.168.1.0/24", "192.168.2.100"));
    }

    #[test]
    fn test_cidr_contains_ip_prefix_32() {
        assert!(cidr_contains_ip("10.0.0.1/32", "10.0.0.1"));
        assert!(!cidr_contains_ip("10.0.0.1/32", "10.0.0.2"));
    }

    #[test]
    fn test_cidr_contains_ip_prefix_0() {
        assert!(cidr_contains_ip("0.0.0.0/0", "255.255.255.255"));
    }

    #[test]
    fn process_webhook_rejected_on_missing_auth() {
        let server = WebhookServer::new(default_config());
        let req = WebhookRequest::new(Platform::Telegram, b"{}".to_vec());
        let result = server.process_webhook(&req, None);
        assert!(matches!(result, WebhookResult::Rejected(_)));
        let resp = match result {
            WebhookResult::Rejected(r) => r,
            _ => panic!("expected Rejected"),
        };
        assert_eq!(resp.status, 401);
    }

    #[test]
    fn process_webhook_parsed_on_success() {
        let mut receiver = WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "tg_secret".into(),
            }),
            None,
        );

        let mut config = default_config();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "api_key".into());

        let server = WebhookServer::with_receiver(config, receiver);

        let body = serde_json::json!({
            "update_id": 1,
            "message": {
                "message_id": 10,
                "from": { "id": 42, "is_bot": false, "first_name": "A" },
                "chat": { "id": 99 },
                "text": "/clawd status",
                "date": 1700000000
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Telegram, body)
            .with_query_param("secret_token", "tg_secret");

        let result = server.process_webhook(&req, Some("api_key"));
        assert!(matches!(result, WebhookResult::Parsed(_)));
        let msg = match result {
            WebhookResult::Parsed(m) => m,
            _ => panic!("expected Parsed"),
        };
        assert_eq!(msg.platform, Platform::Telegram);
        assert_eq!(msg.user.platform_user_id, "42");
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.chat_id(), "99");
    }
}
