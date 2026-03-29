//! Tenant Management REST API
//!
//! Customer-facing endpoints for multi-tenant SaaS administration.
//! All endpoints require Bearer token authentication.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET    | `/api/v1/tenants` | List all tenants |
//! | POST   | `/api/v1/tenants` | Create a tenant |
//! | GET    | `/api/v1/tenants/{id}` | Get tenant details |
//! | PUT    | `/api/v1/tenants/{id}` | Update a tenant |
//! | DELETE | `/api/v1/tenants/{id}` | Delete a tenant |
//! | GET    | `/api/v1/tenants/{id}/usage` | Usage summary |
//! | GET    | `/api/v1/usage` | Global usage overview |

#![deny(unsafe_code)]

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
#[allow(unused_imports)]
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::messaging::tenant::{TenantConfig, TenantId, TenantManager};
use crate::messaging::types::{CommandCategory, Platform};
use crate::messaging::usage_tracker::UsageTracker;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    #[serde(default)]
    pub allowed_platforms: Vec<String>,
    #[serde(default)]
    pub command_whitelist: Vec<String>,
    #[serde(default = "default_max_sessions")]
    pub max_sessions_per_user: u32,
    #[serde(default)]
    pub rate_limit_per_minute: Option<u32>,
    #[serde(default)]
    pub can_generate: bool,
    #[serde(default = "default_true")]
    pub can_analyze: bool,
    #[serde(default)]
    pub can_modify_files: bool,
    #[serde(default)]
    pub can_admin: bool,
}

fn default_max_sessions() -> u32 {
    100
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub enabled: bool,
    pub allowed_platforms: Vec<String>,
    pub command_whitelist: Vec<String>,
    pub max_sessions_per_user: u32,
    pub rate_limit_per_minute: Option<u32>,
    pub can_generate: bool,
    pub can_analyze: bool,
    pub can_modify_files: bool,
    pub can_admin: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub allowed_platforms: Option<Vec<String>>,
    #[serde(default)]
    pub command_whitelist: Option<Vec<String>>,
    pub max_sessions_per_user: Option<u32>,
    pub rate_limit_per_minute: Option<Option<u32>>,
    pub can_generate: Option<bool>,
    pub can_analyze: Option<bool>,
    pub can_modify_files: Option<bool>,
    pub can_admin: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UsageSummaryResponse {
    pub tenant_id: String,
    pub total_messages: u64,
    pub successful: u64,
    pub errors: u64,
    pub avg_latency_ms: u64,
    pub unique_users: usize,
}

#[derive(Debug, Serialize)]
pub struct GlobalUsageResponse {
    pub total_tenants: usize,
    pub tenant_summaries: Vec<UsageSummaryResponse>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TenantApiState {
    pub tenant_manager: Arc<TenantManager>,
    pub usage_tracker: Option<Arc<UsageTracker>>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET `/api/v1/tenants` — List all tenants.
pub async fn list_tenants(State(state): State<TenantApiState>) -> impl IntoResponse {
    let tenants = state.tenant_manager.list_tenants();
    let response: Vec<TenantResponse> = tenants
        .into_iter()
        .map(|t| TenantResponse {
            id: t.tenant_id.to_string(),
            enabled: t.enabled,
            allowed_platforms: t
                .allowed_platforms
                .iter()
                .map(|p| format!("{p:?}"))
                .collect(),
            command_whitelist: t
                .command_whitelist
                .unwrap_or_default()
                .iter()
                .map(|c| format!("{c:?}"))
                .collect(),
            max_sessions_per_user: t.max_sessions_per_user,
            rate_limit_per_minute: extract_default_rate_limit(&t.rate_limit_overrides),
            can_generate: t.default_permissions.can_generate,
            can_analyze: t.default_permissions.can_analyze,
            can_modify_files: t.default_permissions.can_modify_files,
            can_admin: t.default_permissions.can_admin,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();
    (StatusCode::OK, Json(response)).into_response()
}

/// POST `/api/v1/tenants` — Create a new tenant.
pub async fn create_tenant(
    State(state): State<TenantApiState>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    let id = TenantId::new(uuid::Uuid::new_v4());
    let mut config = TenantConfig::new(id.clone());
    config.enabled = true;

    config.allowed_platforms = req
        .allowed_platforms
        .iter()
        .filter_map(|s| parse_platform(s))
        .collect();

    config.command_whitelist = if req.command_whitelist.is_empty() {
        None
    } else {
        Some(
            req.command_whitelist
                .iter()
                .filter_map(|s| parse_command_category(s))
                .collect(),
        )
    };

    config.max_sessions_per_user = req.max_sessions_per_user;
    config.default_permissions.can_generate = req.can_generate;
    config.default_permissions.can_analyze = req.can_analyze;
    config.default_permissions.can_modify_files = req.can_modify_files;
    config.default_permissions.can_admin = req.can_admin;

    if let Some(rl) = req.rate_limit_per_minute {
        for platform in &config.allowed_platforms {
            config.rate_limit_overrides.insert(
                *platform,
                crate::messaging::types::RateLimitConfig {
                    requests_per_minute: rl,
                    burst_capacity: rl / 2,
                    tokens_per_refill: 1,
                    refill_interval_ms: 60_000u64 / rl.max(1) as u64,
                },
            );
        }
    }

    match state.tenant_manager.create_tenant(id, config) {
        Ok(created) => {
            let response = TenantResponse {
                id: created.tenant_id.to_string(),
                enabled: created.enabled,
                allowed_platforms: created
                    .allowed_platforms
                    .iter()
                    .map(|p| format!("{p:?}"))
                    .collect(),
                command_whitelist: created
                    .command_whitelist
                    .unwrap_or_default()
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect(),
                max_sessions_per_user: created.max_sessions_per_user,
                rate_limit_per_minute: extract_default_rate_limit(&created.rate_limit_overrides),
                can_generate: created.default_permissions.can_generate,
                can_analyze: created.default_permissions.can_analyze,
                can_modify_files: created.default_permissions.can_modify_files,
                can_admin: created.default_permissions.can_admin,
                created_at: created.created_at,
                updated_at: created.updated_at,
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TENANT_CREATE_FAILED",
            &e.to_string(),
        ),
    }
}

/// GET `/api/v1/tenants/{id}` — Get tenant details.
pub async fn get_tenant(
    State(state): State<TenantApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tenant_id = TenantId::from(id.as_str());

    match state.tenant_manager.get_tenant(&tenant_id) {
        Ok(Some(t)) => {
            let response = TenantResponse {
                id: t.tenant_id.to_string(),
                enabled: t.enabled,
                allowed_platforms: t
                    .allowed_platforms
                    .iter()
                    .map(|p| format!("{p:?}"))
                    .collect(),
                command_whitelist: t
                    .command_whitelist
                    .unwrap_or_default()
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect(),
                max_sessions_per_user: t.max_sessions_per_user,
                rate_limit_per_minute: extract_default_rate_limit(&t.rate_limit_overrides),
                can_generate: t.default_permissions.can_generate,
                can_analyze: t.default_permissions.can_analyze,
                can_modify_files: t.default_permissions.can_modify_files,
                can_admin: t.default_permissions.can_admin,
                created_at: t.created_at,
                updated_at: t.updated_at,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "TENANT_NOT_FOUND",
            "Tenant not found",
        ),
        Err(e) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &e.to_string(),
        ),
    }
}

/// PUT `/api/v1/tenants/{id}` — Update a tenant.
pub async fn update_tenant(
    State(state): State<TenantApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTenantRequest>,
) -> impl IntoResponse {
    let tenant_id = TenantId::from(id.as_str());

    let mut config = match state.tenant_manager.get_tenant(&tenant_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return json_error(
                StatusCode::NOT_FOUND,
                "TENANT_NOT_FOUND",
                "Tenant not found",
            );
        }
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                &e.to_string(),
            );
        }
    };

    if let Some(enabled) = req.enabled {
        config.enabled = enabled;
    }
    if let Some(platforms) = &req.allowed_platforms {
        config.allowed_platforms = platforms.iter().filter_map(|s| parse_platform(s)).collect();
    }
    if let Some(whitelist) = &req.command_whitelist {
        config.command_whitelist = if whitelist.is_empty() {
            None
        } else {
            Some(
                whitelist
                    .iter()
                    .filter_map(|s| parse_command_category(s))
                    .collect(),
            )
        };
    }
    if let Some(max) = req.max_sessions_per_user {
        config.max_sessions_per_user = max;
    }
    if let Some(rl) = req.rate_limit_per_minute {
        if let Some(rate) = rl {
            for platform in &config.allowed_platforms {
                config.rate_limit_overrides.insert(
                    *platform,
                    crate::messaging::types::RateLimitConfig {
                        requests_per_minute: rate,
                        burst_capacity: rate / 2,
                        tokens_per_refill: 1,
                        refill_interval_ms: 60_000u64 / rate.max(1) as u64,
                    },
                );
            }
        } else {
            config.rate_limit_overrides.clear();
        }
    }
    if let Some(v) = req.can_generate {
        config.default_permissions.can_generate = v;
    }
    if let Some(v) = req.can_analyze {
        config.default_permissions.can_analyze = v;
    }
    if let Some(v) = req.can_modify_files {
        config.default_permissions.can_modify_files = v;
    }
    if let Some(v) = req.can_admin {
        config.default_permissions.can_admin = v;
    }

    match state.tenant_manager.update_tenant(&tenant_id, config) {
        Ok(t) => {
            let response = TenantResponse {
                id: t.tenant_id.to_string(),
                enabled: t.enabled,
                allowed_platforms: t
                    .allowed_platforms
                    .iter()
                    .map(|p| format!("{p:?}"))
                    .collect(),
                command_whitelist: t
                    .command_whitelist
                    .unwrap_or_default()
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect(),
                max_sessions_per_user: t.max_sessions_per_user,
                rate_limit_per_minute: extract_default_rate_limit(&t.rate_limit_overrides),
                can_generate: t.default_permissions.can_generate,
                can_analyze: t.default_permissions.can_analyze,
                can_modify_files: t.default_permissions.can_modify_files,
                can_admin: t.default_permissions.can_admin,
                created_at: t.created_at,
                updated_at: t.updated_at,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TENANT_UPDATE_FAILED",
            &e.to_string(),
        ),
    }
}

/// DELETE `/api/v1/tenants/{id}` — Delete a tenant.
pub async fn delete_tenant(
    State(state): State<TenantApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tenant_id = TenantId::from(id.as_str());

    match state.tenant_manager.delete_tenant(&tenant_id) {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TENANT_DELETE_FAILED",
            &e.to_string(),
        ),
    }
}

/// GET `/api/v1/tenants/{id}/usage` — Usage summary for a tenant.
pub async fn tenant_usage(
    State(state): State<TenantApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tracker = match &state.usage_tracker {
        Some(t) => t,
        None => {
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "USAGE_TRACKING_DISABLED",
                "Usage tracking is not enabled on this instance",
            );
        }
    };

    let summary = tracker.tenant_summary(&id).await;
    let response = UsageSummaryResponse {
        tenant_id: summary.tenant_id,
        total_messages: summary.total_messages,
        successful: summary.successful,
        errors: summary.errors,
        avg_latency_ms: summary.avg_latency_ms,
        unique_users: summary.unique_users,
    };
    (StatusCode::OK, Json(response)).into_response()
}

/// GET `/api/v1/usage` — Global usage overview.
pub async fn global_usage(State(state): State<TenantApiState>) -> impl IntoResponse {
    let tracker = match &state.usage_tracker {
        Some(t) => t,
        None => {
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "USAGE_TRACKING_DISABLED",
                "Usage tracking is not enabled on this instance",
            );
        }
    };

    let tenant_ids: Vec<String> = state
        .tenant_manager
        .list_tenants()
        .iter()
        .map(|t| t.tenant_id.to_string())
        .collect();

    let mut summaries: Vec<UsageSummaryResponse> = Vec::new();
    for tid in &tenant_ids {
        let s = tracker.tenant_summary(tid).await;
        summaries.push(UsageSummaryResponse {
            tenant_id: s.tenant_id,
            total_messages: s.total_messages,
            successful: s.successful,
            errors: s.errors,
            avg_latency_ms: s.avg_latency_ms,
            unique_users: s.unique_users,
        });
    }

    let response = GlobalUsageResponse {
        total_tenants: summaries.len(),
        tenant_summaries: summaries,
    };
    (StatusCode::OK, Json(response)).into_response()
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Create the tenant management router.
///
/// This returns a `Router<TenantApiState>` that must be merged with the
/// main application router. Since axum 0.8 does not allow merging
/// `Router<S>` with different `S`, use `fallback_service()` or nest
/// under a shared state type.
pub fn create_tenant_router(state: TenantApiState) -> Router {
    Router::new()
        .route("/api/v1/tenants", get(list_tenants).post(create_tenant))
        .route(
            "/api/v1/tenants/{id}",
            get(get_tenant).put(update_tenant).delete(delete_tenant),
        )
        .route("/api/v1/tenants/{id}/usage", get(tenant_usage))
        .route("/api/v1/usage", get(global_usage))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_platform(s: &str) -> Option<Platform> {
    match s.to_lowercase().as_str() {
        "telegram" => Some(Platform::Telegram),
        "discord" => Some(Platform::Discord),
        "matrix" => Some(Platform::Matrix),
        "slack" => Some(Platform::Slack),
        "signal" => Some(Platform::Signal),
        "whatsapp" => Some(Platform::WhatsApp),
        "rocketchat" => Some(Platform::RocketChat),
        _ => None,
    }
}

fn parse_command_category(s: &str) -> Option<CommandCategory> {
    match s.to_lowercase().as_str() {
        "generate" | "gen" => Some(CommandCategory::Generate),
        "analyze" | "analyse" => Some(CommandCategory::Analyze),
        "config" | "configure" => Some(CommandCategory::Config),
        "admin" => Some(CommandCategory::Admin),
        "session" => Some(CommandCategory::Session),
        "status" => Some(CommandCategory::Status),
        "help" => Some(CommandCategory::Help),
        _ => None,
    }
}

/// Extract the default rate limit from the overrides map.
/// Returns the rate limit from the first platform found, or None.
fn extract_default_rate_limit(
    overrides: &std::collections::HashMap<Platform, crate::messaging::types::RateLimitConfig>,
) -> Option<u32> {
    overrides.values().next().map(|c| c.requests_per_minute)
}

fn json_error(status: StatusCode, code: &str, message: &str) -> axum::response::Response {
    (
        status,
        Json(serde_json::json!({
            "status": "error",
            "error": { "code": code, "message": message }
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_platform_valid() {
        assert_eq!(parse_platform("telegram"), Some(Platform::Telegram));
        assert_eq!(parse_platform("Discord"), Some(Platform::Discord));
        assert_eq!(parse_platform("MATRIX"), Some(Platform::Matrix));
        assert_eq!(parse_platform("slack"), Some(Platform::Slack));
        assert_eq!(parse_platform("signal"), Some(Platform::Signal));
        assert_eq!(parse_platform("whatsapp"), Some(Platform::WhatsApp));
        assert_eq!(parse_platform("rocketchat"), Some(Platform::RocketChat));
    }

    #[test]
    fn test_parse_platform_invalid() {
        assert_eq!(parse_platform("invalid"), None);
        assert_eq!(parse_platform(""), None);
    }

    #[test]
    fn test_parse_command_category_valid() {
        assert_eq!(
            parse_command_category("generate"),
            Some(CommandCategory::Generate)
        );
        assert_eq!(
            parse_command_category("gen"),
            Some(CommandCategory::Generate)
        );
        assert_eq!(
            parse_command_category("analyze"),
            Some(CommandCategory::Analyze)
        );
        assert_eq!(
            parse_command_category("config"),
            Some(CommandCategory::Config)
        );
        assert_eq!(
            parse_command_category("admin"),
            Some(CommandCategory::Admin)
        );
        assert_eq!(
            parse_command_category("status"),
            Some(CommandCategory::Status)
        );
        assert_eq!(parse_command_category("help"), Some(CommandCategory::Help));
        assert_eq!(
            parse_command_category("session"),
            Some(CommandCategory::Session)
        );
    }

    #[test]
    fn test_parse_command_category_invalid() {
        assert_eq!(parse_command_category("foobar"), None);
        assert_eq!(parse_command_category(""), None);
    }

    #[test]
    fn test_tenant_response_serialization() {
        let r = TenantResponse {
            id: "test-id".to_string(),
            enabled: true,
            allowed_platforms: vec!["Telegram".to_string()],
            command_whitelist: vec![],
            max_sessions_per_user: 100,
            rate_limit_per_minute: Some(60),
            can_generate: true,
            can_analyze: true,
            can_modify_files: true,
            can_admin: false,
            created_at: 0,
            updated_at: 0,
        };
        let json = serde_json::to_string(&r).expect("serialize ok");
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"can_generate\":true"));
    }

    #[test]
    fn test_usage_summary_serialization() {
        let s = UsageSummaryResponse {
            tenant_id: "t1".to_string(),
            total_messages: 100,
            successful: 95,
            errors: 5,
            avg_latency_ms: 42,
            unique_users: 10,
        };
        let json = serde_json::to_string(&s).expect("serialize ok");
        assert!(json.contains("\"total_messages\":100"));
    }

    #[test]
    fn test_global_usage_serialization() {
        let g = GlobalUsageResponse {
            total_tenants: 2,
            tenant_summaries: vec![],
        };
        let json = serde_json::to_string(&g).expect("serialize ok");
        assert!(json.contains("\"total_tenants\":2"));
    }

    #[test]
    fn test_create_tenant_request_deserialization() {
        let json = r#"{"allowed_platforms":["telegram","discord"],"max_sessions_per_user":50,"rate_limit_per_minute":120,"can_generate":false}"#;
        let req: CreateTenantRequest = serde_json::from_str(json).expect("deserialize ok");
        assert_eq!(req.allowed_platforms.len(), 2);
        assert_eq!(req.max_sessions_per_user, 50);
        assert_eq!(req.rate_limit_per_minute, Some(120));
        assert!(!req.can_generate);
    }

    #[test]
    fn test_update_tenant_request_partial() {
        let json = r#"{"enabled":false}"#;
        let req: UpdateTenantRequest = serde_json::from_str(json).expect("deserialize ok");
        assert_eq!(req.enabled, Some(false));
        assert!(req.allowed_platforms.is_none());
        assert!(req.max_sessions_per_user.is_none());
    }

    #[test]
    fn test_extract_default_rate_limit() {
        use crate::messaging::types::RateLimitConfig;
        use std::collections::HashMap;

        let mut overrides = HashMap::new();
        overrides.insert(
            Platform::Telegram,
            RateLimitConfig {
                requests_per_minute: 60,
                burst_capacity: 30,
                tokens_per_refill: 1,
                refill_interval_ms: 1000,
            },
        );
        overrides.insert(
            Platform::Discord,
            RateLimitConfig {
                requests_per_minute: 120,
                burst_capacity: 60,
                tokens_per_refill: 2,
                refill_interval_ms: 500,
            },
        );

        // Returns first value found
        let result = extract_default_rate_limit(&overrides);
        assert!(result.is_some());
        let val = result.unwrap();
        assert!(val == 60 || val == 120);
    }
}
