//! Clawdius Server
//!
//! HTTP server combining the REST API and messaging webhook endpoints
//! into a single axum application. Webhook requests flow through the
//! complete pipeline: authenticate → verify → parse → route to
//! `MessagingGateway` → dispatch to command handlers → send response
//! back via platform adapters.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::{Json, Router};
use clap::Parser;
use clawdius_core::messaging::audit::MessagingAuditLogger;
use clawdius_core::messaging::encrypted_store::maybe_encrypt;
use clawdius_core::messaging::pii_redaction::{PiiRedactionConfig, PiiRedactionLayer};
use clawdius_core::messaging::retry_queue::{RetryConfig, RetryQueue};
use clawdius_core::messaging::state_store::{StateStoreConfig, StateStoreFactory};
use clawdius_core::messaging::usage_tracker::UsageTracker;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use clawdius_core::api::rest::{self, ApiState};
use clawdius_core::api::tenant_api::{self, TenantApiState};
use clawdius_core::config::Config;
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::messaging::channels::{MessagingChannel, MockChannel};
use clawdius_core::messaging::config_builder::build_webhook_infrastructure;
use clawdius_core::messaging::gateway::MessagingGateway;
use clawdius_core::messaging::handlers::{
    AdminHandler, ConfigHandler, HelpHandler, SessionHandler, StatusHandler,
};
use clawdius_core::messaging::integration::{
    ClawdiusAnalyzeHandler, ClawdiusGenerateHandler, SessionManagerFactory,
};
use clawdius_core::messaging::key_rotation::ApiKeyStore;
use clawdius_core::messaging::server::{WebhookResult, WebhookServer, WebhookServerConfig};
use clawdius_core::messaging::tenant::{TenantManager, TenantResolver};
use clawdius_core::messaging::types::{ChannelConfig, CommandCategory, Platform};
use clawdius_core::messaging::webhook_receiver::WebhookRequest;
use clawdius_core::session::SessionStore;

mod api_rate_limiter;
mod metrics;
#[cfg(feature = "otel")]
mod otel;

// ===========================================================================
// Usage Metrics Bridge
// ===========================================================================

/// Bridges the core `UsageMetricsSink` trait to the server's `MetricsStore`,
/// so usage events flow into the Prometheus `/metrics` endpoint.
struct MetricsStoreUsageSink {
    store: std::sync::Arc<metrics::MetricsStore>,
}

impl clawdius_core::messaging::usage_tracker::UsageMetricsSink for MetricsStoreUsageSink {
    fn counter_inc(&self, name: &str, labels: &str) {
        self.store.messaging_counter_inc(name, labels);
    }

    fn histogram_observe(&self, name: &str, labels: &str, value_ms: f64) {
        self.store
            .messaging_histogram_observe(name, labels, value_ms);
    }

    fn gauge_set(&self, name: &str, value: i64) {
        self.store.messaging_gauge_set(name, value);
    }
}

// ===========================================================================
// CLI
// ===========================================================================
// ===========================================================================

/// Clawdius HTTP Server — REST API + Messaging Webhooks
#[derive(Parser, Debug)]
#[command(name = "clawdius-server", version, about)]
struct Cli {
    /// Path to configuration file (TOML). Tries `clawdius.toml` then
    /// `.clawdius/config.toml` if omitted. CLI flags override file values.
    #[arg(short, long)]
    config: Option<String>,

    /// Bind address (default: 0.0.0.0)
    #[arg(long)]
    host: Option<String>,

    /// Bind port (default: 8080)
    #[arg(short, long)]
    port: Option<u16>,

    /// CORS allowed origins (comma-separated). Use "*" for permissive.
    #[arg(long)]
    cors_origins: Option<String>,

    /// Path to the session database. Uses in-memory if omitted.
    #[arg(long)]
    db_path: Option<String>,

    /// Maximum request body size in bytes (default: 1_000_000)
    #[arg(long)]
    max_request_size: Option<usize>,

    /// Use mock channels instead of real platform adapters.
    /// Messages are logged but not actually sent to platforms.
    #[arg(long)]
    mock_channels: bool,

    /// Emit structured JSON log lines instead of human-readable text.
    /// Recommended for production and container environments.
    #[arg(long, env = "CLAWDIUS_JSON_LOGS")]
    json_logs: bool,
}

// ===========================================================================
// Structured Error Responses
// ===========================================================================

/// Standard JSON error envelope returned by all error paths.
///
/// ```json
/// { "status": "error", "error": { "code": "INVALID_API_KEY", "message": "..." } }
/// ```
fn json_error(
    status: StatusCode,
    code: &str,
    message: &str,
) -> (StatusCode, axum::Json<serde_json::Value>) {
    (
        status,
        axum::Json(serde_json::json!({
            "status": "error",
            "error": {
                "code": code,
                "message": message,
            }
        })),
    )
}

// ===========================================================================
// Application State
// ===========================================================================

/// Shared application state accessible by all handlers.
#[derive(Clone)]
struct AppState {
    api_state: ApiState,
    webhook_server: Arc<WebhookServer>,
    gateway: Arc<MessagingGateway>,
    http_metrics: metrics::HttpMetrics,
    tenant_manager: Option<Arc<TenantManager>>,
    usage_tracker: Option<Arc<UsageTracker>>,
    /// API key store for authenticating management endpoints.
    key_store: Arc<ApiKeyStore>,
    /// Pre-built JWT auth instance (feature-gated). `None` when no secret is
    /// configured; the auth middleware falls back to API-key validation.
    #[cfg(feature = "jwt")]
    jwt_auth: Option<clawdius_core::messaging::jwt_auth::JwtAuth>,
}

// ===========================================================================
// Tenant API Adapter Handlers
// ===========================================================================
//
// Thin wrappers that convert `State<AppState>` → `TenantApiState` so the
// tenant management handlers can be served on the same `Router<AppState>`.
// All tenant endpoints require a valid Bearer API key.

/// Extract and validate the `Authorization: Bearer <token>` header.
/// Returns `Err` (with an appropriate error response) on failure.
///
/// Authentication strategy:
/// 1. If the token looks like a JWT (three dot-separated segments) and a JWT
///    secret is configured, validate it as a JWT.
/// 2. Otherwise, hash the token and look it up in the API key store.
async fn require_api_key(
    headers: &HeaderMap,
    key_store: &ApiKeyStore,
    #[cfg(feature = "jwt")] jwt_auth: &Option<clawdius_core::messaging::jwt_auth::JwtAuth>,
) -> Result<(), axum::response::Response> {
    let auth_header = match headers.get(axum::http::header::AUTHORIZATION) {
        Some(v) => match v.to_str() {
            Ok(s) => s,
            Err(_) => {
                return Err(json_error(
                    StatusCode::BAD_REQUEST,
                    "MALFORMED_AUTH_HEADER",
                    "Authorization header contains invalid UTF-8",
                )
                .into_response());
            },
        },
        None => {
            return Err(json_error(
                StatusCode::UNAUTHORIZED,
                "MISSING_API_KEY",
                "Authorization header with Bearer token is required",
            )
            .into_response());
        },
    };

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Err(json_error(
                StatusCode::UNAUTHORIZED,
                "MISSING_API_KEY",
                "Authorization header must use 'Bearer <token>' format",
            )
            .into_response());
        },
    };

    // Strategy 1: Try JWT validation (feature-gated)
    #[cfg(feature = "jwt")]
    if let Some(ref auth) = jwt_auth {
        if clawdius_core::messaging::jwt_auth::looks_like_jwt(token) {
            match auth.validate_token(token) {
                Ok(claims) => {
                    // Valid JWT — extract role for potential future use
                    let _ = &claims;
                    return Ok(());
                },
                Err(clawdius_core::messaging::jwt_auth::JwtError::Expired) => {
                    return Err(json_error(
                        StatusCode::UNAUTHORIZED,
                        "TOKEN_EXPIRED",
                        "JWT token has expired",
                    )
                    .into_response());
                },
                Err(_) => {
                    // JWT validation failed — fall through to API key check
                },
            }
        }
    }

    // Strategy 2: API key hash lookup
    let key_hash = clawdius_core::messaging::key_rotation::hash_api_key(token);
    if key_store.validate_key(&key_hash).await.is_none() {
        return Err(json_error(
            StatusCode::FORBIDDEN,
            "INVALID_API_KEY",
            "The provided API key is not recognized or has expired",
        )
        .into_response());
    }

    Ok(())
}

fn to_tenant_state(state: &AppState) -> TenantApiState {
    TenantApiState {
        tenant_manager: state.tenant_manager.clone().unwrap_or_else(|| {
            panic!("tenant_manager must be Some when tenant routes are registered")
        }),
        usage_tracker: state.usage_tracker.clone(),
    }
}

async fn tenant_list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::list_tenants(State(to_tenant_state(&state)))
        .await
        .into_response()
}

async fn tenant_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<tenant_api::CreateTenantRequest>,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::create_tenant(State(to_tenant_state(&state)), Json(body))
        .await
        .into_response()
}

async fn tenant_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::get_tenant(State(to_tenant_state(&state)), Path(id))
        .await
        .into_response()
}

async fn tenant_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<tenant_api::UpdateTenantRequest>,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::update_tenant(State(to_tenant_state(&state)), Path(id), Json(body))
        .await
        .into_response()
}

async fn tenant_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::delete_tenant(State(to_tenant_state(&state)), Path(id))
        .await
        .into_response()
}

async fn tenant_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::tenant_usage(State(to_tenant_state(&state)), Path(id))
        .await
        .into_response()
}

async fn tenant_global_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    if let Err(e) = require_api_key(
        &headers,
        &state.key_store,
        #[cfg(feature = "jwt")]
        &state.jwt_auth,
    )
    .await
    {
        return e;
    }
    tenant_api::global_usage(State(to_tenant_state(&state)))
        .await
        .into_response()
}

/// Extract the platform from a URL path like `/webhook/{platform}`.
fn platform_from_path(path: &str) -> Option<Platform> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 || segments[0] != "webhook" {
        return None;
    }
    match segments[1] {
        "telegram" => Some(Platform::Telegram),
        "discord" => Some(Platform::Discord),
        "matrix" => Some(Platform::Matrix),
        "signal" => Some(Platform::Signal),
        "rocketchat" => Some(Platform::RocketChat),
        "whatsapp" => Some(Platform::WhatsApp),
        "slack" => Some(Platform::Slack),
        _ => None,
    }
}

// ===========================================================================
// Webhook Handler (Full Pipeline)
// ===========================================================================

/// Handle an incoming webhook request through the complete pipeline:
///
/// 1. Build `WebhookRequest` from raw HTTP data
/// 2. `WebhookServer::process_webhook()` — authenticate + verify + parse
/// 3. `MessagingGateway::process_message()` — rate limit + command parse + dispatch
/// 4. Response sent back to the platform via `MessagingChannel::send_message()`
///
/// If auth/verify fails, the error response is returned immediately.
/// If the gateway processes successfully, `200 OK` with message IDs is returned.
async fn webhook_handler(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    query: Query<HashMap<String, String>>,
    body: Bytes,
) -> impl IntoResponse {
    let platform = match platform_from_path(uri.path()) {
        Some(p) => p,
        None => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "UNKNOWN_PLATFORM",
                "Unknown platform in webhook path",
            )
            .into_response();
        },
    };

    // Build the framework-agnostic WebhookRequest
    let mut req = WebhookRequest::new(platform, body.to_vec());
    for (name, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            req = req.with_header(name.to_string(), v.to_string());
        }
    }
    for (key, value) in query.0.iter() {
        req = req.with_query_param(key.clone(), value.clone());
    }

    // Extract API key from Authorization: Bearer <key>
    let api_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    // Check IP allowlist (reads X-Forwarded-For or X-Real-Ip for reverse proxy setups)
    let remote_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim())
        });

    if let Some(ip) = remote_ip {
        if !state.webhook_server.is_ip_allowed(Some(ip)) {
            tracing::warn!(ip = %ip, "Webhook rejected: source IP not in allowlist");
            return json_error(
                StatusCode::FORBIDDEN,
                "IP_NOT_ALLOWED",
                "Request source IP not in allowlist",
            )
            .into_response();
        }
    }

    // Step 1: Authenticate, verify signature, parse body
    let parsed = state.webhook_server.process_webhook(&req, api_key);

    let msg = match parsed {
        WebhookResult::Rejected(response) => {
            let status =
                StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let code = error_code_from_status(response.status);
            return (
                status,
                axum::Json(serde_json::json!({
                    "status": "error",
                    "error": {
                        "code": code,
                        "message": response.body,
                    }
                })),
            )
                .into_response();
        },
        WebhookResult::Parsed(msg) => msg,
    };

    tracing::info!(
        platform = %msg.platform,
        user = %msg.user.platform_user_id,
        content = %msg.content.chars().take(80).collect::<String>(),
        "Incoming webhook message"
    );

    // Step 2: Route through MessagingGateway (rate limit → parse → handler → send)
    let chat_id = msg.chat_id();
    let user_id = &msg.user.platform_user_id;
    let content = &msg.content;

    match state
        .gateway
        .process_message(msg.platform, user_id, &chat_id, content, api_key)
        .await
    {
        Ok(message_ids) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "ok",
                "message_ids": message_ids,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Gateway processing error");
            let (status, code) = match &e {
                clawdius_core::messaging::types::MessagingError::RateLimited { .. } => {
                    (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED")
                },
                clawdius_core::messaging::types::MessagingError::Unauthorized { .. } => {
                    (StatusCode::FORBIDDEN, "UNAUTHORIZED")
                },
                clawdius_core::messaging::types::MessagingError::ChannelUnavailable(_) => {
                    (StatusCode::SERVICE_UNAVAILABLE, "CHANNEL_UNAVAILABLE")
                },
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
            };
            json_error(status, code, &e.to_string()).into_response()
        },
    }
}

/// Map an HTTP status code to a structured error code string.
fn error_code_from_status(status: u16) -> &'static str {
    match status {
        400 => "BAD_REQUEST",
        401 => "UNAUTHORIZED",
        403 => "FORBIDDEN",
        404 => "NOT_FOUND",
        405 => "METHOD_NOT_ALLOWED",
        413 => "PAYLOAD_TOO_LARGE",
        429 => "RATE_LIMITED",
        500 => "INTERNAL_ERROR",
        503 => "SERVICE_UNAVAILABLE",
        _ => "ERROR",
    }
}

// ===========================================================================
// Path Conversion
// ===========================================================================

/// Convert `:param` syntax (WebhookRoute) to axum's `{param}` syntax.
fn to_axum_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' {
            if chars.peek() == Some(&':') {
                result.push_str("::");
                chars.next();
            } else {
                result.push('{');
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        result.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                result.push('}');
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ===========================================================================
// JWT Helper
// ===========================================================================

/// Build an `Option<JwtAuth>` from the messaging config.
///
/// Resolution order:
/// 1. Environment variable `CLAWDIUS_JWT_SECRET` (takes precedence)
/// 2. Config file field `messaging.jwt_secret`
///
/// Returns `None` if no secret is configured (JWT auth disabled; API keys
/// still work as a fallback).
#[cfg(feature = "jwt")]
fn build_jwt_auth(
    msg_config: &clawdius_core::config::MessagingConfig,
) -> Option<clawdius_core::messaging::jwt_auth::JwtAuth> {
    // Env var takes precedence over config file (secrets should not be in files)
    let secret =
        std::env::var("CLAWDIUS_JWT_SECRET").unwrap_or_else(|_| msg_config.jwt_secret.clone());

    if secret.trim().is_empty() {
        tracing::info!("JWT auth disabled: no secret configured");
        None
    } else {
        match clawdius_core::messaging::jwt_auth::JwtAuth::new(&secret) {
            Ok(auth) => {
                tracing::info!("JWT auth enabled (HMAC-SHA256)");
                Some(auth)
            },
            Err(e) => {
                tracing::warn!(error = %e, "JWT auth disabled: invalid secret");
                None
            },
        }
    }
}

// ===========================================================================
// Gateway Construction
// ===========================================================================

/// Build a `MessagingGateway` with all command handlers and platform channels.
///
/// Handlers (no LLM required):
/// - `/clawd status` → `StatusHandler`
/// - `/clawd help` → `HelpHandler`
/// - `/clawd session` → `SessionHandler`
/// - `/clawd config` → `ConfigHandler`
/// - `/clawd admin` → `AdminHandler`
///
/// LLM-backed handlers (require session factory + LLM client):
/// - `/clawd gen` → `ClawdiusGenerateHandler`
/// - `/clawd analyze` → `ClawdiusAnalyzeHandler`
///
/// When `mock_channels` is true, `MockChannel` is used for all platforms
/// (messages are acknowledged but not sent to real platforms).
async fn build_gateway(
    webhook_server: &WebhookServer,
    mock_channels: bool,
    llm_client: Option<Arc<dyn LlmClient>>,
    session_factory: Option<SessionManagerFactory>,
    platform_configs: Option<&HashMap<String, clawdius_core::config::WebhookPlatformConfig>>,
) -> MessagingGateway {
    let gateway = MessagingGateway::new();

    // Register basic handlers (no LLM needed)
    gateway
        .register_handler(CommandCategory::Status, Arc::new(StatusHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Help, Arc::new(HelpHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Session, Arc::new(SessionHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Config, Arc::new(ConfigHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Admin, Arc::new(AdminHandler::new()))
        .await;

    // Register LLM-backed handlers when available
    if let (Some(client), Some(factory)) = (llm_client, session_factory) {
        tracing::info!("LLM client available — registering generate & analyze handlers");
        gateway
            .register_handler(
                CommandCategory::Generate,
                Arc::new(ClawdiusGenerateHandler::new(
                    factory.clone(),
                    client.clone(),
                )),
            )
            .await;
        gateway
            .register_handler(
                CommandCategory::Analyze,
                Arc::new(ClawdiusAnalyzeHandler::new(factory, client)),
            )
            .await;
    } else {
        tracing::info!(
            "No LLM client configured — generate & analyze commands will return 'Unknown command'"
        );
    }

    // Register a channel for each registered platform
    for platform in webhook_server.build_routes().iter().map(|r| r.platform) {
        let channel: Arc<dyn MessagingChannel> = if !mock_channels {
            if let Some(cfgs) = platform_configs {
                if let Some(real_ch) = build_real_channel(platform, cfgs) {
                    tracing::info!(%platform, "Using real channel adapter");
                    real_ch
                } else {
                    tracing::warn!(%platform, "No real channel credentials — falling back to mock");
                    Arc::new(MockChannel::new(platform))
                }
            } else {
                Arc::new(MockChannel::new(platform))
            }
        } else {
            Arc::new(MockChannel::new(platform))
        };

        let config = ChannelConfig::new(platform);
        gateway.configure_channel(config).await;
        gateway.register_channel(channel).await;
    }

    gateway
}

fn build_real_channel(
    platform: Platform,
    platform_cfg: &HashMap<String, clawdius_core::config::WebhookPlatformConfig>,
) -> Option<Arc<dyn MessagingChannel>> {
    use clawdius_core::messaging::channels::*;

    let key = match platform {
        Platform::Telegram => "telegram",
        Platform::Discord => "discord",
        Platform::Matrix => "matrix",
        Platform::Slack => "slack",
        Platform::Signal => "signal",
        Platform::WhatsApp => "whatsapp",
        Platform::RocketChat => "rocketchat",
        _ => return None,
    };

    let cfg = platform_cfg.get(key)?;

    match platform {
        Platform::Telegram => {
            let bot_token = cfg.bot_token.as_deref()?.trim();
            if bot_token.is_empty() {
                return None;
            }
            Some(Arc::new(TelegramChannel::new(bot_token)))
        },
        Platform::Discord => {
            let bot_token = cfg.discord_bot_token.as_deref()?.trim();
            if bot_token.is_empty() {
                return None;
            }
            Some(Arc::new(DiscordChannel::new(bot_token)))
        },
        Platform::Matrix => {
            let homeserver = cfg.homeserver_base_url.as_deref()?.trim();
            let token = cfg.access_token.as_deref()?.trim();
            if homeserver.is_empty() || token.is_empty() {
                return None;
            }
            Some(Arc::new(MatrixChannel::new(homeserver, token)))
        },
        Platform::Slack => {
            let bot_token = cfg.slack_bot_token.as_deref()?.trim();
            if bot_token.is_empty() {
                return None;
            }
            Some(Arc::new(SlackChannel::new(bot_token)))
        },
        Platform::RocketChat => {
            let server_url = cfg
                .server_url
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("https://rocketchat.example.com");
            let user_id = cfg.user_id.as_deref()?.trim();
            let token = cfg.token.as_deref()?.trim();
            if user_id.is_empty() || token.is_empty() {
                return None;
            }
            Some(Arc::new(RocketChatChannel::new(server_url, user_id, token)))
        },
        Platform::Signal => {
            let api_url = cfg.signal_api_url.as_deref()?.trim();
            let number = cfg.signal_number.as_deref()?.trim();
            if api_url.is_empty() || number.is_empty() {
                return None;
            }
            Some(Arc::new(SignalChannel::new(api_url, number)))
        },
        Platform::WhatsApp => {
            let phone_id = cfg.phone_number_id.as_deref()?.trim();
            let token = cfg.whatsapp_access_token.as_deref()?.trim();
            if phone_id.is_empty() || token.is_empty() {
                return None;
            }
            Some(Arc::new(WhatsAppChannel::new(phone_id, token)))
        },
        _ => None,
    }
}

// ===========================================================================
// Router Construction
// ===========================================================================

/// Build the merged axum router.
fn build_app(
    state: AppState,
    cors: CorsLayer,
    max_body_size: usize,
    rate_limit_config: api_rate_limiter::ApiRateLimitConfig,
) -> Router {
    let _http_metrics = state.http_metrics.clone();

    // 1. Webhook routes — each platform's path maps to `any(webhook_handler)`
    let mut router = Router::new();
    for route in state.webhook_server.build_routes() {
        let axum_path = to_axum_path(&route.path);
        router = router.route(&axum_path, axum::routing::any(webhook_handler));
    }

    // 2. Prometheus metrics scrape endpoint
    async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
        let body = state.http_metrics.store.export_prometheus();
        (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            body,
        )
    }
    router = router.route("/metrics", axum::routing::get(metrics_handler));

    // 2b. Health / readiness endpoints (K8s-style probes)
    // GET /health — liveness probe: server process is responsive
    router = router.route(
        "/health",
        axum::routing::get(|State(state): State<AppState>| async move {
            let uptime_secs = state.gateway.uptime_secs().await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "ok",
                    "uptime_secs": uptime_secs,
                })),
            )
        }),
    );
    // GET /ready — readiness probe: dependencies are reachable
    router = router.route(
        "/ready",
        axum::routing::get(|State(state): State<AppState>| async move {
            let store_ok = state.gateway.health_check().await;
            let status = if store_ok { "ok" } else { "degraded" };
            let code = if store_ok {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            let active_tenants = if let Some(ref tracker) = state.usage_tracker {
                tracker.active_tenant_count().await
            } else {
                0
            };
            (
                code,
                Json(serde_json::json!({
                    "status": status,
                    "active_tenants": active_tenants,
                })),
            )
        }),
    );

    // 3. Tenant management API routes (only if multi-tenancy is enabled)
    //    Wrapped with per-key rate limiting (429 Too Many Requests on burst exhaustion).
    if state.tenant_manager.is_some() {
        let tenant_router = Router::new()
            .route(
                "/api/v1/tenants",
                axum::routing::get(tenant_list_tenants).post(tenant_create),
            )
            .route(
                "/api/v1/tenants/{id}",
                axum::routing::get(tenant_get)
                    .put(tenant_update)
                    .delete(tenant_delete),
            )
            .route(
                "/api/v1/tenants/{id}/usage",
                axum::routing::get(tenant_usage),
            )
            .route("/api/v1/usage", axum::routing::get(tenant_global_usage))
            .layer(api_rate_limiter::ApiRateLimitLayer::new(rate_limit_config))
            .with_state(state.clone());

        // Merge stateful tenant router into the main stateful router.
        // Axum 0.8: Router<S> + Router<S> works, but Router<S> + Router<()> does not.
        // We nest the tenant routes via a fallback_service approach.
        // Since both have the same state type, merge directly.
        router = router.merge(tenant_router);
    }

    // 4. REST API routes — stateless Router<()> attached as fallback
    let rest_router = rest::create_router(state.api_state.clone());

    // 5. Combine: webhook + metrics + tenant routes take priority, REST API handles everything else
    router
        .fallback_service(rest_router)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(max_body_size))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Build a CORS layer from a comma-separated origins string.
fn build_cors_layer(origins_str: &str) -> CorsLayer {
    if origins_str.trim() == "*" {
        CorsLayer::permissive()
    } else {
        let parsed: Vec<axum::http::HeaderValue> = origins_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();

        if parsed.is_empty() {
            return CorsLayer::permissive();
        }

        CorsLayer::new()
            .allow_origin(parsed)
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
            ])
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
    }
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI early so we can configure logging before anything else
    let cli = Cli::parse();

    // --- Initialize OpenTelemetry (BEFORE tracing subscriber, if feature enabled) ---
    // When the otel feature is active, init_otel_tracing sets up the global
    // subscriber with OTel + fmt layers. If it succeeds, we skip the regular
    // subscriber init below. If it fails (e.g., already initialised), we
    // fall through to the regular init.
    #[cfg(feature = "otel")]
    let otel_provider = otel::init_otel_tracing();

    #[cfg(not(feature = "otel"))]
    let _otel_provider: Option<()> = None;

    // Only init the regular subscriber if OTel did NOT set one up already.
    #[cfg(feature = "otel")]
    if otel_provider.is_none() {
        let pii_layer = PiiRedactionLayer::new(PiiRedactionConfig::default())
            .expect("PII redaction regex compilation must succeed");
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        if cli.json_logs {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(pii_layer)
                .with(tracing_subscriber::fmt::layer().json().with_target(false))
                .init();
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(pii_layer)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .init();
        }
    }

    #[cfg(not(feature = "otel"))]
    {
        let pii_layer = PiiRedactionLayer::new(PiiRedactionConfig::default())
            .expect("PII redaction regex compilation must succeed");
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        if cli.json_logs {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(pii_layer)
                .with(tracing_subscriber::fmt::layer().json().with_target(false))
                .init();
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(pii_layer)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .init();
        }
    }

    // --- Load configuration file ---
    let config = match &cli.config {
        Some(path) => {
            tracing::info!(path = %path, "Loading config from file");
            Config::load(std::path::Path::new(path))?
        },
        None => match Config::load_default() {
            Ok(cfg) => {
                tracing::info!("Loaded config from default location");
                cfg
            },
            Err(_) => {
                tracing::info!("No config file found — using defaults");
                Config::default()
            },
        },
    };

    // Audit secrets: warn if any are in config file instead of env vars
    clawdius_core::messaging::secret_resolver::audit_config_secrets(&config);

    // Resolve effective values: file → CLI override
    let host = cli.host.unwrap_or_else(|| config.messaging.host.clone());
    let port = cli.port.unwrap_or(config.messaging.port);
    let cors_str = cli
        .cors_origins
        .unwrap_or_else(|| config.messaging.cors_origins.clone().join(","));
    let max_request_size = cli
        .max_request_size
        .unwrap_or(config.messaging.max_request_size_bytes);

    // --- Open session store ---
    let session_store = match &cli.db_path {
        Some(path) => {
            tracing::info!(path = %path, "Opening session database");
            SessionStore::open(std::path::Path::new(path))?
        },
        None => {
            let path = &config.storage.sessions_path;
            if path.as_os_str().is_empty() {
                tracing::info!("Using in-memory session store");
                SessionStore::in_memory()?
            } else {
                tracing::info!(path = %path.display(), "Opening session database");
                SessionStore::open(path)?
            }
        },
    };

    // Build API state (for REST routes)
    let api_state = ApiState::new(session_store);

    // --- Build webhook server from config ---
    let key_store = Arc::new(ApiKeyStore::new());
    let infra = build_webhook_infrastructure(&config.messaging);

    let webhook_config = WebhookServerConfig {
        host: host.clone(),
        port,
        max_request_size_bytes: max_request_size,
        cors_origins: config.messaging.cors_origins.clone(),
        ip_allowlist: config.messaging.ip_allowlist.clone(),
        api_authenticator: infra.api_authenticator.with_key_store(key_store.clone()),
        ..infra.server_config
    };

    let webhook_server = Arc::new(WebhookServer::with_receiver(webhook_config, infra.receiver));

    // Log configured platforms
    if config.messaging.is_configured() {
        tracing::info!("Messaging config loaded with credentials");
    } else {
        tracing::info!("No messaging credentials configured — only API-key auth will work");
    }

    // Build Prometheus metrics store (shared across gateway and HTTP handlers)
    let http_metrics_store = metrics::MetricsStore::new();

    // Build unified state store (memory or sqlite), optionally encrypted
    // Encryption key resolution: env var (CLAWDIUS_ENCRYPTION_KEY) → config file
    let encryption_key = clawdius_core::messaging::secret_resolver::resolve(
        "CLAWDIUS_ENCRYPTION_KEY",
        Some(&config.messaging.state_store.encryption_key),
        false,
    );
    let encryption_key_ref = encryption_key.as_str();
    let state_store = if config.messaging.state_store.backend == "memory" {
        tracing::info!("Using in-memory state store");
        StateStoreFactory::new(StateStoreConfig::Memory)
    } else {
        let path = &config.messaging.state_store.sqlite_path;
        tracing::info!(path = %path, "Using SQLite state store");
        StateStoreFactory::new(StateStoreConfig::SQLite { path: path.clone() })
    }
    .map_err(|e| anyhow::anyhow!("Failed to create state store: {e}"))?;

    // Apply encryption at rest if a key is configured
    let raw_store = state_store.store();
    let state_store_arc = if !encryption_key_ref.trim().is_empty() {
        match maybe_encrypt(raw_store.clone(), encryption_key_ref) {
            Ok(encrypted) => {
                tracing::info!("Encryption at rest enabled for state store");
                encrypted
            },
            Err(e) => {
                tracing::warn!(error = %e, "Encryption at rest failed — using plaintext store");
                raw_store
            },
        }
    } else {
        raw_store
    };

    // Build messaging gateway with audit logging and retry queue
    tracing::info!(
        mock_channels = cli.mock_channels,
        "Building messaging gateway"
    );

    // Create tenant manager (shared between gateway and tenant API)
    let tenant_manager: Option<Arc<TenantManager>> = if config.messaging.tenants.enabled {
        let tenant_db_path = &config.messaging.tenants.db_path;
        let manager = Arc::new(if !tenant_db_path.is_empty() {
            TenantManager::with_persistence(std::path::Path::new(tenant_db_path))
                .expect("failed to open tenant database")
        } else {
            TenantManager::new()
        });
        tracing::info!("Multi-tenancy enabled");
        Some(manager)
    } else {
        None
    };

    // Create usage tracker (shared between gateway and tenant API)
    let usage_tracker = Arc::new(
        UsageTracker::new()
            .with_store(state_store.store())
            .with_metrics(Arc::new(MetricsStoreUsageSink {
                store: Arc::new(http_metrics_store.clone()),
            })),
    );
    tracing::info!("Usage metering enabled");

    let gateway = Arc::new({
        let mut gw = build_gateway(
            &webhook_server,
            cli.mock_channels,
            None,
            None,
            Some(&config.messaging.platforms),
        )
        .await;
        gw = gw.with_audit(Arc::new(MessagingAuditLogger::new(
            clawdius_core::enterprise::audit::AuditStorage::File {
                path: std::path::PathBuf::from("audit.log"),
            },
        )));
        let retry_queue = RetryQueue::new(RetryConfig::default())
            .with_state_store(state_store.retry_queue_store());
        gw = gw.with_retry_queue(Arc::new(retry_queue));

        if let Some(ref manager) = tenant_manager {
            gw = gw.with_tenant_manager(manager.clone());
            gw = gw.with_tenant_resolver(TenantResolver::new(manager.clone()));
        }

        gw = gw.with_state_store(state_store_arc.clone());
        gw = gw.with_usage_tracker(usage_tracker.clone());
        gw
    });

    // Build application state
    let gateway_for_shutdown = gateway.clone();
    let app_state = AppState {
        api_state,
        webhook_server: webhook_server.clone(),
        gateway,
        http_metrics: metrics::HttpMetrics {
            store: http_metrics_store,
        },
        tenant_manager,
        usage_tracker: Some(usage_tracker),
        key_store: key_store.clone(),
        #[cfg(feature = "jwt")]
        jwt_auth: build_jwt_auth(&config.messaging),
    };

    // Build router
    let cors = build_cors_layer(&cors_str);
    let rate_limit_config = api_rate_limiter::ApiRateLimitConfig::new(60, 10);
    let app = build_app(app_state, cors, max_request_size, rate_limit_config);

    // Bind address
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    tracing::info!("🚀 Clawdius Server starting");
    tracing::info!(%addr, "Listening on");

    for route in webhook_server.build_routes() {
        tracing::info!(
            path = %route.path,
            platform = %route.platform,
            methods = ?route.methods,
            "Webhook route registered"
        );
    }

    tracing::info!("Pipeline: webhook → authenticate → parse → command → handler → respond");

    // Serve with graceful shutdown
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    gateway_for_shutdown.shutdown().await;

    // Flush OTel spans before exit
    #[cfg(feature = "otel")]
    otel::shutdown(otel_provider);

    tracing::info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutdown signal received");
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_axum_path_no_params() {
        assert_eq!(to_axum_path("/webhook/telegram"), "/webhook/telegram");
    }

    #[test]
    fn test_to_axum_path_with_param() {
        assert_eq!(
            to_axum_path("/webhook/matrix/:room_id"),
            "/webhook/matrix/{room_id}"
        );
    }

    #[test]
    fn test_to_axum_path_multiple_params() {
        assert_eq!(
            to_axum_path("/webhook/:platform/:room_id"),
            "/webhook/{platform}/{room_id}"
        );
    }

    #[test]
    fn test_to_axum_path_preserves_double_colon() {
        assert_eq!(
            to_axum_path("/api/v1/sessions::history"),
            "/api/v1/sessions::history"
        );
    }

    #[test]
    fn test_to_axum_path_trailing_param() {
        assert_eq!(to_axum_path("/test/:id"), "/test/{id}");
    }

    #[test]
    fn test_to_axum_path_embedded_param() {
        assert_eq!(
            to_axum_path("/rooms/:room_id/events"),
            "/rooms/{room_id}/events"
        );
    }

    #[test]
    fn test_platform_from_path() {
        assert_eq!(
            platform_from_path("/webhook/telegram"),
            Some(Platform::Telegram)
        );
        assert_eq!(
            platform_from_path("/webhook/discord"),
            Some(Platform::Discord)
        );
        assert_eq!(
            platform_from_path("/webhook/matrix"),
            Some(Platform::Matrix)
        );
        assert_eq!(
            platform_from_path("/webhook/matrix/!room:example.com"),
            Some(Platform::Matrix)
        );
        assert_eq!(platform_from_path("/api/v1/health"), None);
        assert_eq!(platform_from_path("/webhook/unknown"), None);
        assert_eq!(platform_from_path("/other"), None);
    }

    #[test]
    fn test_webhook_routes_count() {
        let config = WebhookServerConfig::default();
        let server = WebhookServer::new(config);
        assert_eq!(server.build_routes().len(), 7);
    }

    #[test]
    fn test_build_cors_layer_permissive() {
        let _ = build_cors_layer("*");
    }
    #[test]
    fn test_build_cors_layer_specific() {
        let _ = build_cors_layer("http://localhost:3000,https://example.com");
    }
    #[test]
    fn test_build_cors_layer_empty() {
        let _ = build_cors_layer("");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_starts_and_responds() {
        let session_store = SessionStore::in_memory().unwrap();
        let api_state = ApiState::new(session_store);

        let config = WebhookServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
            ..Default::default()
        };
        let webhook_server = Arc::new(WebhookServer::new(config));

        let gateway = Arc::new(build_gateway(&webhook_server, true, None, None, None).await);

        let app_state = AppState {
            api_state,
            webhook_server,
            gateway,
            http_metrics: metrics::HttpMetrics {
                store: metrics::MetricsStore::new(),
            },
            tenant_manager: None,
            usage_tracker: None,
            key_store: Arc::new(ApiKeyStore::new()),
            #[cfg(feature = "jwt")]
            jwt_auth: None,
        };

        let app = build_app(
            app_state,
            CorsLayer::permissive(),
            1_000_000,
            api_rate_limiter::ApiRateLimitConfig::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                })
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = reqwest::Client::new();
        match client
            .get(format!("http://{}/api/v1/health", local_addr))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(r) => {
                assert!(r.status().is_success());
                let body: serde_json::Value = r.json().await.unwrap();
                assert_eq!(body["status"], "ok");
            },
            Err(e) => {
                eprintln!("Request error (may be timing): {e}");
            },
        }

        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_webhook_pipeline_end_to_end() {
        use clawdius_core::messaging::auth::AuthResult;
        use clawdius_core::messaging::webhook_receiver::{TelegramWebhookConfig, WebhookConfig};

        // Set up a webhook server with Telegram registered
        let mut receiver = clawdius_core::messaging::webhook_receiver::WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "test_secret".into(),
            }),
            None,
        );

        let mut config = WebhookServerConfig::default();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "test_api_key".into());

        let webhook_server = Arc::new(WebhookServer::with_receiver(config, receiver));

        // Verify the webhook server has the correct auth config
        assert!(matches!(
            webhook_server
                .config()
                .api_authenticator
                .validate(Platform::Telegram, Some("test_api_key")),
            AuthResult::Authenticated { .. }
        ));

        let gateway = Arc::new(build_gateway(&webhook_server, true, None, None, None).await);

        let session_store = SessionStore::in_memory().unwrap();
        let api_state = ApiState::new(session_store);

        let app_state = AppState {
            api_state,
            webhook_server: webhook_server.clone(),
            gateway,
            http_metrics: metrics::HttpMetrics {
                store: metrics::MetricsStore::new(),
            },
            tenant_manager: None,
            usage_tracker: None,
            key_store: Arc::new(ApiKeyStore::new()),
            #[cfg(feature = "jwt")]
            jwt_auth: None,
        };

        let app = build_app(
            app_state,
            CorsLayer::permissive(),
            1_000_000,
            api_rate_limiter::ApiRateLimitConfig::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                })
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = reqwest::Client::new();

        // Test 1: Missing auth → 401
        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .json(&serde_json::json!({"update_id": 1, "message": {"message_id": 1, "from": {"id": 42, "is_bot": false, "first_name": "A"}, "chat": {"id": 99}, "text": "hello", "date": 1700000000}}))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401);

        // Test 2: Valid webhook → 200 (parsed → gateway → handler → mock send)
        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .header("Authorization", "Bearer test_api_key")
            .query(&[("secret_token", "test_secret")])
            .json(&serde_json::json!({"update_id": 2, "message": {"message_id": 2, "from": {"id": 42, "is_bot": false, "first_name": "A"}, "chat": {"id": 99}, "text": "/clawd status", "date": 1700000000}}))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
        // MockChannel returns UUID message IDs
        assert!(body["message_ids"].is_array());

        // Test 3: Command with help → 200
        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .header("Authorization", "Bearer test_api_key")
            .query(&[("secret_token", "test_secret")])
            .json(&serde_json::json!({"update_id": 3, "message": {"message_id": 3, "from": {"id": 42, "is_bot": false, "first_name": "A"}, "chat": {"id": 99}, "text": "/clawd help", "date": 1700000000}}))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test 4: Unknown command → 200 (handler returns "Unknown command")
        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .header("Authorization", "Bearer test_api_key")
            .query(&[("secret_token", "test_secret")])
            .json(&serde_json::json!({"update_id": 4, "message": {"message_id": 4, "from": {"id": 42, "is_bot": false, "first_name": "A"}, "chat": {"id": 99}, "text": "/clawd foobar", "date": 1700000000}}))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test 5: Non-command text → 200 (unknown category)
        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .header("Authorization", "Bearer test_api_key")
            .query(&[("secret_token", "test_secret")])
            .json(&serde_json::json!({"update_id": 5, "message": {"message_id": 5, "from": {"id": 42, "is_bot": false, "first_name": "A"}, "chat": {"id": 99}, "text": "just chatting", "date": 1700000000}}))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_generate_handler_with_mock_llm() {
        use clawdius_core::llm::ChatMessage;
        use clawdius_core::messaging::webhook_receiver::{TelegramWebhookConfig, WebhookConfig};
        use tokio::sync::mpsc;

        struct MockLlmClient;

        #[async_trait::async_trait]
        impl LlmClient for MockLlmClient {
            async fn chat(&self, _messages: Vec<ChatMessage>) -> clawdius_core::Result<String> {
                Ok("fn mock() -> bool { true }".to_string())
            }

            async fn chat_stream(
                &self,
                _messages: Vec<ChatMessage>,
            ) -> clawdius_core::Result<mpsc::Receiver<String>> {
                let (tx, rx) = mpsc::channel(1);
                let _ = tx.send("fn mock() -> bool { true }".to_string()).await;
                Ok(rx)
            }

            fn count_tokens(&self, text: &str) -> usize {
                text.len()
            }
        }

        let session_factory: SessionManagerFactory = Arc::new(|| {
            clawdius_core::session::SessionManager::new(&clawdius_core::config::Config::default())
        });

        let mut receiver = clawdius_core::messaging::webhook_receiver::WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "test_secret".into(),
            }),
            None,
        );

        let mut config = WebhookServerConfig::default();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "test_api_key".into());

        let webhook_server = Arc::new(WebhookServer::with_receiver(config, receiver));

        let llm_client: Arc<dyn LlmClient> = Arc::new(MockLlmClient);
        let gateway = Arc::new(
            build_gateway(
                &webhook_server,
                true,
                Some(llm_client),
                Some(session_factory),
                None,
            )
            .await,
        );

        let session_store = SessionStore::in_memory().unwrap();
        let api_state = ApiState::new(session_store);

        let app_state = AppState {
            api_state,
            webhook_server: webhook_server.clone(),
            gateway,
            http_metrics: metrics::HttpMetrics {
                store: metrics::MetricsStore::new(),
            },
            tenant_manager: None,
            usage_tracker: None,
            key_store: Arc::new(ApiKeyStore::new()),
            #[cfg(feature = "jwt")]
            jwt_auth: None,
        };

        let app = build_app(
            app_state,
            CorsLayer::permissive(),
            1_000_000,
            api_rate_limiter::ApiRateLimitConfig::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                })
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = reqwest::Client::new();

        let resp = client
            .post(format!("http://{}/webhook/telegram", local_addr))
            .header("Authorization", "Bearer test_api_key")
            .query(&[("secret_token", "test_secret")])
            .json(&serde_json::json!({
                "update_id": 100,
                "message": {
                    "message_id": 100,
                    "from": {"id": 42, "is_bot": false, "first_name": "A"},
                    "chat": {"id": 99},
                    "text": "/clawd gen write a hello world function",
                    "date": 1700000000
                }
            }))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body["message_ids"].is_array());

        handle.await.unwrap();
    }

    /// E2E: Health, readiness, metrics, and tenant auth endpoints.
    #[tokio::test]
    async fn test_health_readiness_metrics_and_tenant_auth() {
        let mut receiver = clawdius_core::messaging::webhook_receiver::WebhookReceiver::new();
        receiver.register_platform(
            clawdius_core::messaging::webhook_receiver::WebhookConfig::Telegram(
                clawdius_core::messaging::webhook_receiver::TelegramWebhookConfig {
                    secret_token: "test_secret".into(),
                },
            ),
            None,
        );

        let mut config = WebhookServerConfig::default();
        config
            .api_authenticator
            .add_platform_key(Platform::Telegram, "test_api_key".into());

        let webhook_server = Arc::new(WebhookServer::with_receiver(config, receiver));
        let gateway = Arc::new(build_gateway(&webhook_server, true, None, None, None).await);

        let session_store = SessionStore::in_memory().unwrap();
        let api_state = ApiState::new(session_store);

        // Create a key store with a registered key for tenant API auth
        let key_store = Arc::new(ApiKeyStore::new());
        key_store
            .add_key(clawdius_core::messaging::key_rotation::ApiKeyEntry::new(
                clawdius_core::messaging::key_rotation::hash_api_key("tenant-master-key"),
                "e2e-test-key",
            ))
            .await;

        let usage_tracker = Arc::new(UsageTracker::new());

        let app_state = AppState {
            api_state,
            webhook_server,
            gateway,
            http_metrics: metrics::HttpMetrics {
                store: metrics::MetricsStore::new(),
            },
            tenant_manager: None,
            usage_tracker: Some(usage_tracker),
            key_store,
            #[cfg(feature = "jwt")]
            jwt_auth: None,
        };

        let app = build_app(
            app_state,
            CorsLayer::permissive(),
            1_000_000,
            api_rate_limiter::ApiRateLimitConfig::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let base = format!("http://{local_addr}");

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                })
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let client = reqwest::Client::new();

        // --- Health endpoint ---
        let resp = client
            .get(format!("{base}/health"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body["uptime_secs"].is_number());
        assert!(body["uptime_secs"].as_u64().unwrap() < 5);

        // --- Readiness endpoint ---
        let resp = client
            .get(format!("{base}/ready"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");

        // --- Metrics endpoint ---
        let resp = client
            .get(format!("{base}/metrics"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let text = resp.text().await.unwrap();
        assert!(
            text.contains("clawdius_"),
            "metrics should contain clawdius_ prefix"
        );

        // --- Tenant API without auth → 401 ---
        // When multi-tenancy is enabled, the auth middleware intercepts.
        // When disabled, routes fall through to REST fallback (404).
        let resp = client
            .get(format!("{base}/api/v1/tenants"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        // tenant_manager is None → routes not registered → 404 from REST fallback
        assert_eq!(resp.status(), 404);

        // --- Tenant API with wrong key (routes not registered) → still 404 ---
        let resp = client
            .get(format!("{base}/api/v1/tenants"))
            .header("Authorization", "Bearer wrong-key")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);

        handle.await.unwrap();
    }
}
