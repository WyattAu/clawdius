//! Admin REST API for multi-tenant management.
//!
//! Provides HTTP endpoints for:
//! - Tenant CRUD (create, list, get, update, delete)
//! - Usage metrics (current cycle, historical)
//! - Quota management (get, set, reset)
//! - Subscription management (create, change plan, cancel)
//! - Health and system info
//!
//! Uses axum for HTTP routing. Auth is handled by a configurable
//! middleware (API key or JWT).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use clawdius_core::billing::{BillingManager, PlanTier, Subscription};
use clawdius_core::usage::{Quota, TenantUsageTracker};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "auth")]
use clawdius_auth::{rbac::RbacService, AuthService};

// ─────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────

/// In-memory store for user-to-role mappings.
#[derive(Clone, Default)]
pub struct RoleStore {
    /// user_id -> role name
    roles: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
}

impl RoleStore {
    /// Get a user's role (defaults to "viewer" if not set).
    pub async fn get_role(&self, user_id: &str) -> String {
        self.roles
            .read()
            .await
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| "viewer".to_string())
    }

    /// Set a user's role.
    pub async fn set_role(&self, user_id: &str, role: &str) {
        self.roles
            .write()
            .await
            .insert(user_id.to_string(), role.to_string());
    }

    /// Remove a user's role (reverts to default "viewer").
    pub async fn remove_role(&self, user_id: &str) -> bool {
        self.roles.write().await.remove(user_id).is_some()
    }

    /// List all user-role mappings.
    pub async fn list_roles(&self) -> HashMap<String, String> {
        self.roles.read().await.clone()
    }
}

/// Admin API application state.
#[allow(missing_docs)]
pub struct AdminState {
    pub billing: Arc<BillingManager>,
    pub usage: Arc<TenantUsageTracker>,
    /// Admin API key for authentication.
    pub api_key: String,
    /// User role assignments.
    pub roles: RoleStore,
    /// Optional auth service for SSO token validation.
    #[cfg(feature = "auth")]
    pub auth: Option<Arc<AuthService>>,
    /// Optional RBAC service for permission checks.
    #[cfg(feature = "auth")]
    pub rbac: Option<Arc<RbacService>>,
}

// ─────────────────────────────────────────────────────────
// Auth Middleware
// ─────────────────────────────────────────────────────────

/// Axum middleware that validates the `X-API-Key` header against the admin key.
async fn require_api_key(
    headers: HeaderMap,
    State(state): State<Arc<AdminState>>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != state.api_key {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "ok": false,
                "error": "Invalid or missing X-API-Key header",
                "timestamp": Utc::now().to_rfc3339(),
            })),
        )
            .into_response();
    }

    next.run(req).into_response()
}

/// Generic API response wrapper.
#[allow(missing_docs, clippy::trait_duplication_in_bounds)]
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Build a JSON error response with status code.
fn error_response(status: StatusCode, msg: impl Into<String>) -> axum::response::Response {
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "error": msg.into(),
            "timestamp": Utc::now().to_rfc3339(),
        })),
    )
        .into_response()
}

/// Build a JSON success response.
fn ok_response<T: Serialize>(data: T) -> axum::response::Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "data": data,
            "timestamp": Utc::now().to_rfc3339(),
        })),
    )
        .into_response()
}

fn created_response<T: Serialize>(data: T) -> axum::response::Response {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "ok": true,
            "data": data,
            "timestamp": Utc::now().to_rfc3339(),
        })),
    )
        .into_response()
}

/// Convert a subscription to a tenant info map (avoids partial move issues).
fn subscription_to_map(sub: &Subscription) -> serde_json::Value {
    serde_json::json!({
        "tenant_id": sub.tenant_id,
        "tier": sub.tier.to_string(),
        "status": format!("{:?}", sub.status).to_lowercase(),
        "tokens_used": sub.tokens_used,
        "tokens_remaining": sub.remaining_tokens(),
        "cost_cents": sub.cost_cents,
        "seats": sub.seats,
        "period_start": sub.current_period_start.to_rfc3339(),
        "period_end": sub.current_period_end.to_rfc3339(),
        "cancel_at_period_end": sub.cancel_at_period_end,
    })
}

/// Request to create a tenant.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub tenant_id: String,
    pub tier: Option<String>,
    pub quota: Option<QuotaOverride>,
}

/// Quota override fields.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct QuotaOverride {
    pub monthly_token_limit: Option<u64>,
    pub per_request_token_limit: Option<u64>,
    pub monthly_cost_limit: Option<f64>,
    pub rate_limit_per_minute: Option<u32>,
}

/// Request to change plan.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct ChangePlanRequest {
    pub new_tier: String,
}

/// Query parameters for listing tenants.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    pub status: Option<String>,
    pub tier: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// System health info.
#[allow(missing_docs)]
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub version: String,
    pub uptime_secs: u64,
    pub total_tenants: usize,
    pub stripe_enabled: bool,
    pub active_subscriptions: usize,
}

// ─────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────

/// Build the admin API router with API key authentication.
pub fn admin_router(state: Arc<AdminState>) -> Router {
    Router::new()
        // Tenant management
        .route("/api/admin/tenants", post(create_tenant))
        .route("/api/admin/tenants", get(list_tenants))
        .route("/api/admin/tenants/{tenant_id}", get(get_tenant))
        .route("/api/admin/tenants/{tenant_id}", delete(delete_tenant))
        .route("/api/admin/tenants/{tenant_id}/usage", get(get_usage))
        .route(
            "/api/admin/tenants/{tenant_id}/usage/reset",
            post(reset_usage),
        )
        .route("/api/admin/tenants/{tenant_id}/quota", get(get_quota))
        .route("/api/admin/tenants/{tenant_id}/quota", put(set_quota))
        .route(
            "/api/admin/tenants/{tenant_id}/subscription",
            get(get_subscription),
        )
        .route(
            "/api/admin/tenants/{tenant_id}/subscription/plan",
            put(change_plan),
        )
        .route(
            "/api/admin/tenants/{tenant_id}/subscription/cancel",
            post(cancel_subscription),
        )
        // Role management
        .route("/api/admin/roles/{user_id}", get(get_user_role))
        .route("/api/admin/roles/{user_id}", put(set_user_role))
        .route("/api/admin/roles/{user_id}", delete(delete_user_role))
        .route("/api/admin/roles", get(list_all_roles))
        // System
        .route("/api/admin/system/info", get(system_info))
        .route("/api/admin/health", get(health_check))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            require_api_key,
        ))
        .with_state(state)
}

// ─────────────────────────────────────────────────────────
// Role Management Handlers
// ─────────────────────────────────────────────────────────

/// Request body for setting a user's role.
#[derive(Debug, Deserialize)]
pub struct SetRoleRequest {
    pub role: String,
}

async fn get_user_role(
    State(state): State<Arc<AdminState>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    let role = state.roles.get_role(&user_id).await;
    ok_response(serde_json::json!({
        "user_id": user_id,
        "role": role,
    }))
}

async fn set_user_role(
    State(state): State<Arc<AdminState>>,
    Path(user_id): Path<String>,
    Json(req): Json<SetRoleRequest>,
) -> impl IntoResponse {
    // Validate role name
    let valid_roles = ["viewer", "user", "operator", "admin"];
    if !valid_roles.contains(&req.role.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            format!(
                "invalid role: '{}'. Must be one of: {}",
                req.role,
                valid_roles.join(", ")
            ),
        );
    }

    state.roles.set_role(&user_id, &req.role).await;
    ok_response(serde_json::json!({
        "user_id": user_id,
        "role": req.role,
        "updated": true,
    }))
}

async fn delete_user_role(
    State(state): State<Arc<AdminState>>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    let removed = state.roles.remove_role(&user_id).await;
    ok_response(serde_json::json!({
        "user_id": user_id,
        "removed": removed,
        "role": "viewer",
    }))
}

async fn list_all_roles(
    State(state): State<Arc<AdminState>>,
) -> impl IntoResponse {
    let roles = state.roles.list_roles().await;
    let entries: Vec<_> = roles
        .into_iter()
        .map(|(user_id, role)| serde_json::json!({"user_id": user_id, "role": role}))
        .collect();
    ok_response(serde_json::json!({
        "roles": entries,
        "total": entries.len(),
    }))
}

// ─────────────────────────────────────────────────────────
// Existing Handlers
// ─────────────────────────────────────────────────────────

async fn create_tenant(
    State(state): State<Arc<AdminState>>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    let tier = parse_tier(req.tier.as_deref()).unwrap_or(PlanTier::Free);
    let sub = state.billing.create_subscription(&req.tenant_id, tier);

    if let Some(q) = req.quota {
        let quota = Quota {
            monthly_token_limit: q.monthly_token_limit.unwrap_or(tier.token_allowance()),
            per_request_token_limit: q.per_request_token_limit.unwrap_or(100_000),
            monthly_cost_limit: q.monthly_cost_limit.unwrap_or(100.0),
            rate_limit_per_minute: q.rate_limit_per_minute.unwrap_or(60),
        };
        state.usage.register_tenant(&req.tenant_id, quota);
    }

    created_response(subscription_to_map(&sub))
}

async fn list_tenants(
    State(state): State<Arc<AdminState>>,
    Query(params): Query<ListTenantsQuery>,
) -> impl IntoResponse {
    let mut subs = state.billing.list_subscriptions();

    if let Some(ref status) = params.status {
        let target = status.to_lowercase();
        subs.retain(|s| format!("{:?}", s.status).to_lowercase() == target);
    }
    if let Some(ref tier) = params.tier {
        let target = tier.to_lowercase();
        subs.retain(|s| s.tier.to_string().to_lowercase() == target);
    }

    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50);
    let total = subs.len();

    let tenants: Vec<_> = subs
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|s| subscription_to_map(&s))
        .collect();

    ok_response(serde_json::json!({
        "tenants": tenants,
        "total": total,
        "offset": offset,
        "limit": limit,
    }))
}

async fn get_tenant(
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    match state.billing.get_subscription(&tenant_id) {
        Some(sub) => ok_response(subscription_to_map(&sub)),
        #[allow(clippy::single_match_else)]
        None => error_response(StatusCode::NOT_FOUND, "tenant not found"),
    }
}

async fn delete_tenant(
    headers: HeaderMap,
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    #[cfg(feature = "auth")]
    {
        if let Err(resp) = check_rbac_permission(
            &headers,
            &state,
            clawdius_auth::rbac::permissions::admin_manage_users(),
        )
        .await
        {
            return resp;
        }
    }

    match state.billing.cancel_subscription(&tenant_id) {
        Ok(()) => ok_response(serde_json::json!({ "deleted": true })),
        Err(e) => error_response(StatusCode::NOT_FOUND, e.to_string()),
    }
}

#[allow(clippy::cast_precision_loss)]
async fn get_usage(
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    match state.usage.get_usage(&tenant_id) {
        Some((tokens, cost_cents, calls)) => {
            let allowance = state
                .billing
                .get_subscription(&tenant_id)
                .map_or(0, |s| s.tier.token_allowance());
            let utilization = if allowance > 0 {
                (tokens as f64 / allowance as f64) * 100.0
            } else {
                0.0
            };
            ok_response(serde_json::json!({
                "tenant_id": tenant_id,
                "tokens": tokens,
                "cost_cents": cost_cents,
                "api_calls": calls,
                "utilization_pct": utilization,
            }))
        },
        None => error_response(StatusCode::NOT_FOUND, "tenant not found"),
    }
}

async fn reset_usage(
    headers: HeaderMap,
    State(state): State<Arc<AdminState>>,
    Path(_tenant_id): Path<String>,
) -> impl IntoResponse {
    #[cfg(feature = "auth")]
    {
        if let Err(resp) = check_rbac_permission(
            &headers,
            &state,
            clawdius_auth::rbac::permissions::admin_manage_config(),
        )
        .await
        {
            return resp;
        }
    }

    state.usage.reset_all();
    state.billing.reset_all_periods();
    ok_response(serde_json::json!({ "reset": true }))
}

#[allow(clippy::cast_precision_loss)]
async fn get_quota(
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    match state.billing.get_subscription(&tenant_id) {
        Some(sub) => ok_response(serde_json::json!({
            "monthly_token_limit": sub.tier.token_allowance(),
            "per_request_token_limit": 100_000,
            "monthly_cost_limit_usd": sub.tier.price_cents() as f64 / 100.0,
            "rate_limit_per_minute": 60,
        })),
        None => error_response(StatusCode::NOT_FOUND, "tenant not found"),
    }
}

async fn set_quota(
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
    Json(quota): Json<Quota>,
) -> impl IntoResponse {
    state.usage.register_tenant(&tenant_id, quota);
    ok_response(serde_json::json!({ "quota_set": true }))
}

async fn get_subscription(
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    state.billing.get_subscription(&tenant_id).map_or_else(
        || error_response(StatusCode::NOT_FOUND, "tenant not found"),
        |sub| ok_response(subscription_to_map(&sub)),
    )
}

async fn change_plan(
    headers: HeaderMap,
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
    Json(req): Json<ChangePlanRequest>,
) -> impl IntoResponse {
    #[cfg(feature = "auth")]
    {
        if let Err(resp) = check_rbac_permission(
            &headers,
            &state,
            clawdius_auth::rbac::permissions::admin_manage_config(),
        )
        .await
        {
            return resp;
        }
    }

    let tier_str = req.new_tier.clone();
    let new_tier = match parse_tier(Some(tier_str.as_str())) {
        Some(t) => t,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                format!("invalid tier: {}", req.new_tier),
            );
        },
    };
    match state.billing.change_plan(&tenant_id, new_tier) {
        Ok(sub) => ok_response(subscription_to_map(&sub)),
        Err(e) => error_response(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

async fn cancel_subscription(
    headers: HeaderMap,
    State(state): State<Arc<AdminState>>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    #[cfg(feature = "auth")]
    {
        if let Err(resp) = check_rbac_permission(
            &headers,
            &state,
            clawdius_auth::rbac::permissions::admin_manage_config(),
        )
        .await
        {
            return resp;
        }
    }

    match state.billing.cancel_subscription(&tenant_id) {
        Ok(()) => ok_response(serde_json::json!({ "canceled": true })),
        Err(e) => error_response(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

async fn system_info(State(state): State<Arc<AdminState>>) -> impl IntoResponse {
    let subs = state.billing.list_subscriptions();
    let active = subs.iter().filter(|s| s.is_active()).count();
    ok_response(SystemInfo {
        version: clawdius_core::VERSION.to_string(),
        uptime_secs: 0,
        total_tenants: subs.len(),
        stripe_enabled: state.billing.is_stripe_enabled(),
        active_subscriptions: active,
    })
}

async fn health_check() -> impl IntoResponse {
    ok_response(serde_json::json!({ "status": "healthy" }))
}

// ─────────────────────────────────────────────────────────
// RBAC Helpers
// ─────────────────────────────────────────────────────────

/// Check if an SSO token (from Authorization header) has the required RBAC permission.
/// Returns Ok(()) if auth is not configured (API key only) or if the token has the permission.
/// Returns Err response if the token is missing or lacks permission.
#[cfg(feature = "auth")]
async fn check_rbac_permission(
    headers: &HeaderMap,
    state: &AdminState,
    permission: clawdius_auth::rbac::Permission,
) -> Result<(), impl IntoResponse> {
    let (Some(auth), Some(rbac)) = (&state.auth, &state.rbac) else {
        return Ok(());
    };

    // Extract Bearer token from Authorization header
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if token.is_empty() {
        return Ok(());
    }

    // Validate the token and check permission
    match auth.validate_session(token) {
        Ok(claims) => match rbac.check(&claims, &permission) {
            Ok(()) => Ok(()),
            Err(e) => Err(error_response(StatusCode::FORBIDDEN, e.to_string())),
        },
        Err(e) => Err(error_response(
            StatusCode::UNAUTHORIZED,
            format!("Invalid token: {e}"),
        )),
    }
}

#[cfg(not(feature = "auth"))]
async fn check_rbac_permission(
    _headers: &HeaderMap,
    _state: &AdminState,
    _perm: &str,
) -> Result<(), axum::response::Response> {
    Ok(())
}

// ─────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────

fn parse_tier(tier: Option<&str>) -> Option<PlanTier> {
    match tier {
        Some("free") => Some(PlanTier::Free),
        Some("pro") => Some(PlanTier::Pro),
        Some("team") => Some(PlanTier::Team),
        Some("enterprise") => Some(PlanTier::Enterprise),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::billing::{BillingError, SubscriptionStatus};

    fn test_state() -> Arc<AdminState> {
        Arc::new(AdminState {
            billing: Arc::new(BillingManager::new()),
            usage: Arc::new(TenantUsageTracker::new()),
            api_key: "test-key".to_string(),
            roles: Default::default(),
            #[cfg(feature = "auth")]
            auth: None,
            #[cfg(feature = "auth")]
            rbac: None,
        })
    }

    #[tokio::test]
    async fn test_create_and_get_tenant() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        let sub = state.billing.get_subscription("org1").unwrap();
        assert_eq!(sub.tier, PlanTier::Pro);
    }

    #[test]
    fn test_parse_tier() {
        assert_eq!(parse_tier(Some("free")), Some(PlanTier::Free));
        assert_eq!(parse_tier(Some("pro")), Some(PlanTier::Pro));
        assert_eq!(parse_tier(Some("team")), Some(PlanTier::Team));
        assert_eq!(parse_tier(Some("enterprise")), Some(PlanTier::Enterprise));
        assert_eq!(parse_tier(Some("invalid")), None);
        assert_eq!(parse_tier(None), None);
    }

    #[test]
    fn test_subscription_to_map() {
        let sub = Subscription::new("org1", PlanTier::Pro);
        let map = subscription_to_map(&sub);
        assert_eq!(map["tenant_id"], "org1");
        assert_eq!(map["tier"], "Pro");
        assert_eq!(map["status"], "active");
    }

    #[test]
    fn test_usage_tracking_with_billing() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        state.usage.register_tenant("org1", Quota::default());
        state.usage.record_usage("org1", 500_000, 50).unwrap();

        let (tokens, _cost_cents, calls) = state.usage.get_usage("org1").unwrap();
        assert_eq!(tokens, 500_000);
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_plan_change_via_billing_manager() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Free);
        state.billing.change_plan("org1", PlanTier::Team).unwrap();
        let sub = state.billing.get_subscription("org1").unwrap();
        assert_eq!(sub.tier, PlanTier::Team);
    }

    #[test]
    fn test_list_tenants() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Free);
        state.billing.create_subscription("org2", PlanTier::Pro);
        state.billing.create_subscription("org3", PlanTier::Free);
        assert_eq!(state.billing.list_subscriptions().len(), 3);
    }

    #[test]
    fn test_cancel_subscription() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        state.billing.cancel_subscription("org1").unwrap();
        let sub = state.billing.get_subscription("org1").unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Canceled);
    }

    #[test]
    fn test_custom_quota() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        let custom_quota = Quota {
            monthly_token_limit: 1_000_000,
            per_request_token_limit: 50_000,
            monthly_cost_limit: 50.0,
            rate_limit_per_minute: 30,
        };
        state.usage.register_tenant("org1", custom_quota);
        assert!(state.usage.record_usage("org1", 999_999, 0).is_ok());
        assert!(state.usage.record_usage("org1", 2, 0).is_err());
    }

    #[test]
    fn test_nonexistent_tenant() {
        let state = test_state();
        assert!(state.billing.get_subscription("ghost").is_none());
        assert!(state.usage.get_usage("ghost").is_none());
    }

    #[test]
    fn test_create_tenant_request_deserialize() {
        let json = r#"{"tenant_id":"org1","tier":"pro","quota":{"monthly_token_limit":5000000}}"#;
        let req: CreateTenantRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tenant_id, "org1");
        assert_eq!(req.tier, Some("pro".to_string()));
        assert_eq!(
            req.quota.as_ref().unwrap().monthly_token_limit,
            Some(5_000_000)
        );
    }

    #[test]
    fn test_system_info() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        let subs = state.billing.list_subscriptions();
        let active = subs.iter().filter(|s| s.is_active()).count();
        assert_eq!(active, 1);
        assert!(!state.billing.is_stripe_enabled());
    }

    #[test]
    fn test_billing_error_not_found() {
        let state = test_state();
        let result = state.billing.change_plan("ghost", PlanTier::Pro);
        assert!(matches!(result, Err(BillingError::NotFound { .. })));
    }

    #[test]
    fn test_billing_error_quota_exceeded() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Free);
        // Register with a small quota to trigger the exceeded case
        let small_quota = Quota {
            monthly_token_limit: 100_000,
            per_request_token_limit: 100_000,
            monthly_cost_limit: 100.0,
            rate_limit_per_minute: 60,
        };
        state.usage.register_tenant("org1", small_quota);
        state.usage.record_usage("org1", 100_000, 0).unwrap();
        let result = state.usage.record_usage("org1", 1, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_tenant_request_minimal() {
        let json = r#"{"tenant_id":"org1"}"#;
        let req: CreateTenantRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tenant_id, "org1");
        assert!(req.tier.is_none());
        assert!(req.quota.is_none());
    }

    #[test]
    fn test_change_plan_request_deserialize() {
        let json = r#"{"new_tier":"team"}"#;
        let req: ChangePlanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.new_tier, "team");
    }

    #[test]
    fn test_list_tenants_query_default() {
        let query = ListTenantsQuery {
            status: None,
            tier: None,
            limit: None,
            offset: None,
        };
        assert!(query.status.is_none());
        assert!(query.tier.is_none());
        assert!(query.limit.is_none());
        assert!(query.offset.is_none());
    }

    #[test]
    fn test_subscription_to_map_canceled() {
        let mut sub = Subscription::new("org1", PlanTier::Pro);
        sub.status = SubscriptionStatus::Canceled;
        sub.cancel_at_period_end = true;
        let map = subscription_to_map(&sub);
        assert_eq!(map["status"], "canceled");
        assert_eq!(map["cancel_at_period_end"], true);
    }

    #[test]
    fn test_usage_tracking_multiple_tenants() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        state.billing.create_subscription("org2", PlanTier::Free);
        state.usage.register_tenant("org1", Quota::default());
        state.usage.register_tenant("org2", Quota::default());

        state.usage.record_usage("org1", 100_000, 10).unwrap();
        state.usage.record_usage("org2", 50_000, 5).unwrap();

        let (t1, _, _) = state.usage.get_usage("org1").unwrap();
        let (t2, _, _) = state.usage.get_usage("org2").unwrap();
        assert_eq!(t1, 100_000);
        assert_eq!(t2, 50_000);
    }

    #[test]
    fn test_tenant_creation_with_all_tiers() {
        let state = test_state();
        for tier in [PlanTier::Free, PlanTier::Pro, PlanTier::Team] {
            let sub = state
                .billing
                .create_subscription(format!("org_{tier:?}"), tier);
            assert_eq!(sub.tier, tier);
        }
        assert_eq!(state.billing.list_subscriptions().len(), 3);
    }

    #[test]
    fn test_system_info_no_tenants() {
        let state = test_state();
        let subs = state.billing.list_subscriptions();
        assert_eq!(subs.len(), 0);
        assert!(!state.billing.is_stripe_enabled());
    }

    #[test]
    fn test_quota_override_full() {
        let json = r#"{"monthly_token_limit":5000000,"per_request_token_limit":50000,"monthly_cost_limit":25.0,"rate_limit_per_minute":30}"#;
        let quota: QuotaOverride = serde_json::from_str(json).unwrap();
        assert_eq!(quota.monthly_token_limit, Some(5_000_000));
        assert_eq!(quota.per_request_token_limit, Some(50_000));
        assert!((quota.monthly_cost_limit.unwrap() - 25.0).abs() < 0.01);
        assert_eq!(quota.rate_limit_per_minute, Some(30));
    }

    #[test]
    fn test_usage_reset_restores_zero() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Pro);
        state.usage.register_tenant("org1", Quota::default());
        state.usage.record_usage("org1", 500_000, 50).unwrap();

        state.usage.reset_all();
        state.billing.reset_all_periods();

        let (tokens, cost, calls) = state.usage.get_usage("org1").unwrap();
        assert_eq!(tokens, 0);
        assert_eq!(cost, 0);
        assert_eq!(calls, 0);

        let sub = state.billing.get_subscription("org1").unwrap();
        assert_eq!(sub.tokens_used, 0);
    }

    #[test]
    fn test_duplicate_tenant_creation() {
        let state = test_state();
        state.billing.create_subscription("org1", PlanTier::Free);
        let sub2 = state.billing.create_subscription("org1", PlanTier::Pro);
        assert_eq!(sub2.tier, PlanTier::Pro);
        let current = state.billing.get_subscription("org1").unwrap();
        assert_eq!(current.tier, PlanTier::Pro);
    }

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    // ─────────────────────────────────────────────────────────
    // API Key Auth Tests
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_admin_requires_api_key() {
        let state = test_state();
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/health")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_admin_wrong_api_key() {
        let state = test_state();
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/health")
            .header("x-api-key", "wrong-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_admin_valid_api_key() {
        let state = test_state();
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/health")
            .header("x-api-key", "test-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // ─────────────────────────────────────────────────────────
    // Role Management Tests
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_role_store_default() {
        let store = RoleStore::default();
        assert_eq!(store.get_role("user1").await, "viewer");
    }

    #[tokio::test]
    async fn test_role_store_set_and_get() {
        let store = RoleStore::default();
        store.set_role("user1", "admin").await;
        assert_eq!(store.get_role("user1").await, "admin");
    }

    #[tokio::test]
    async fn test_role_store_remove() {
        let store = RoleStore::default();
        store.set_role("user1", "admin").await;
        assert!(store.remove_role("user1").await);
        assert_eq!(store.get_role("user1").await, "viewer");
    }

    #[tokio::test]
    async fn test_role_store_list() {
        let store = RoleStore::default();
        store.set_role("user1", "admin").await;
        store.set_role("user2", "viewer").await;
        let roles = store.list_roles().await;
        assert_eq!(roles.len(), 2);
        assert_eq!(roles.get("user1").unwrap(), "admin");
    }

    #[tokio::test]
    async fn test_get_user_role_endpoint() {
        let state = test_state();
        state.roles.set_role("user1", "operator").await;
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/roles/user1")
            .header("x-api-key", "test-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["role"], "operator");
    }

    #[tokio::test]
    async fn test_set_user_role_endpoint() {
        let state = test_state();
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/roles/user1")
            .method("PUT")
            .header("x-api-key", "test-key")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"role":"admin"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["role"], "admin");
    }

    #[tokio::test]
    async fn test_set_invalid_role() {
        let state = test_state();
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/roles/user1")
            .method("PUT")
            .header("x-api-key", "test-key")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"role":"superuser"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_user_role() {
        let state = test_state();
        state.roles.set_role("user1", "admin").await;
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/roles/user1")
            .method("DELETE")
            .header("x-api-key", "test-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["role"], "viewer");
    }

    #[tokio::test]
    async fn test_list_all_roles() {
        let state = test_state();
        state.roles.set_role("user1", "admin").await;
        state.roles.set_role("user2", "viewer").await;
        let app = admin_router(state);

        let req = Request::builder()
            .uri("/api/admin/roles")
            .header("x-api-key", "test-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"]["total"], 2);
    }
}
