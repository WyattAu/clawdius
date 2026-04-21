//! API key authentication middleware for Axum
//!
//! Supports two sources of API keys:
//! 1. Config-level keys (from `[api.keys]` in config)
//! 2. Tenant-level keys (from TenantStore, created via /api/v1/auth/signup)
//!
//! When auth is enabled, both sources are checked. If either validates the
//! Bearer token, the request is allowed through.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::api::tenant::{AuthenticatedApiKey, TenantStore};

// ── Config-level auth state ──────────────────────────────────────────

#[derive(Clone)]
pub struct ApiKeyAuth {
    valid_keys: HashMap<String, String>,
    enabled: bool,
}

impl ApiKeyAuth {
    pub fn new(keys: HashMap<String, String>) -> Self {
        let enabled = !keys.is_empty();
        Self {
            valid_keys: keys,
            enabled,
        }
    }

    pub fn from_config(api_keys: Option<HashMap<String, String>>) -> Self {
        match api_keys {
            Some(keys) => Self::new(keys),
            None => Self {
                valid_keys: HashMap::new(),
                enabled: false,
            },
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ── Combined auth state (config keys + tenant store) ──────────────────

/// Combined authentication state that checks both config-level API keys
/// and tenant-level API keys from the TenantStore.
#[derive(Clone)]
pub struct AuthState {
    config_auth: ApiKeyAuth,
    tenant_store: Arc<RwLock<TenantStore>>,
}

impl AuthState {
    pub fn new(config_auth: ApiKeyAuth, tenant_store: Arc<RwLock<TenantStore>>) -> Self {
        Self {
            config_auth,
            tenant_store,
        }
    }

    /// Whether authentication is enabled at all.
    /// Auth is enabled if config has keys OR there are any tenants with keys.
    pub fn is_enabled(&self) -> bool {
        self.config_auth.enabled
    }
}

// ── Skip paths (no auth required) ────────────────────────────────────

const SKIP_PATHS: &[&str] = &[
    "/api/v1/health",
    "/api/v1/ready",
    "/api/v1/auth/signup",
    "/api/v1/auth/login",
];

fn should_skip(path: &str) -> bool {
    SKIP_PATHS.iter().any(|skip| path == *skip)
}

// ── Token extraction ─────────────────────────────────────────────────

fn extract_bearer_token(headers: &header::HeaderMap) -> Option<String> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let prefix = "Bearer ";
    if auth.starts_with(prefix) {
        Some(auth[prefix.len()..].to_string())
    } else {
        None
    }
}

// ── Auth middleware (config-only, for backward compat) ────────────────

pub async fn auth_middleware(
    axum::extract::State(auth): axum::extract::State<ApiKeyAuth>,
    mut request: Request,
    next: Next,
) -> Response {
    if !auth.enabled || should_skip(request.uri().path()) {
        return next.run(request).await;
    }

    match extract_bearer_token(request.headers()) {
        None => unauthorized_response("Missing authorization header"),
        Some(token) => {
            if auth.valid_keys.values().any(|v| v == &token) {
                request.extensions_mut().insert(AuthenticatedApiKey(token));
                next.run(request).await
            } else {
                forbidden_response("Invalid API key")
            }
        },
    }
}

// ── Auth middleware (config + tenant store) ───────────────────────────

/// Authentication middleware that checks both config-level API keys and
/// tenant-level API keys from the TenantStore.
///
/// Priority:
/// 1. Config-level keys (fast HashMap lookup)
/// 2. Tenant store keys (linear scan, acquires read lock)
pub async fn tenant_aware_auth_middleware(
    axum::extract::State(auth_state): axum::extract::State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    if !auth_state.is_enabled() || should_skip(request.uri().path()) {
        return next.run(request).await;
    }

    let token = match extract_bearer_token(request.headers()) {
        None => return unauthorized_response("Missing authorization header"),
        Some(token) => token,
    };

    // Check config-level keys first (fast path)
    let config_valid = auth_state
        .config_auth
        .valid_keys
        .values()
        .any(|v| v == &token);

    if config_valid {
        request.extensions_mut().insert(AuthenticatedApiKey(token));
        return next.run(request).await;
    }

    // Check tenant store keys (fallback) — lock is scoped to this block
    let tenant_valid = {
        let store = auth_state.tenant_store.read().unwrap();
        store.get_tenant_by_api_key(&token).is_some()
    }; // RwLockReadGuard dropped here

    if tenant_valid {
        request.extensions_mut().insert(AuthenticatedApiKey(token));
        next.run(request).await
    } else {
        forbidden_response("Invalid API key")
    }
}

// ── Error responses ──────────────────────────────────────────────────

fn unauthorized_response(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        axum::Json(serde_json::json!({
            "error": "unauthorized",
            "message": message,
        })),
    )
        .into_response()
}

fn forbidden_response(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "forbidden",
            "message": message,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request as HttpRequest, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn make_keys() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("test-key".to_string(), "secret123".to_string());
        m
    }

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn auth_router(keys: Option<HashMap<String, String>>) -> Router {
        let auth = ApiKeyAuth::from_config(keys);
        let app = Router::new()
            .route("/api/v1/health", get(|| async { "healthy" }))
            .route("/api/v1/ready", get(|| async { "ready" }))
            .route("/api/v1/test", get(ok_handler))
            .with_state(auth.clone());
        if auth.is_enabled() {
            app.layer(middleware::from_fn_with_state(auth, auth_middleware))
        } else {
            app
        }
    }

    fn make_request(path: &str, bearer: Option<&str>) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder().method("GET").uri(path);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn valid_key_accepted() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", Some("secret123"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_key_rejected() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", Some("wrong-key"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_key_rejected() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_accessible_without_auth() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/health", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tenant_store_key_accepted() {
        use crate::api::tenant::{ApiKeyEntry, Tenant, TenantTier, TenantUsage};
        use chrono::Utc;

        let now = Utc::now();
        let tenant = Tenant {
            id: "org_test".to_string(),
            name: "Test Org".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![ApiKeyEntry {
                key: "ck_test_tenant_key_12345".to_string(),
                label: "test-label".to_string(),
                created_at: now,
                last_used_at: now,
                active: true,
            }],
            email: Some("test@test.com".to_string()),
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: now,
            last_active_at: now,
        };

        let mut store = TenantStore::new();
        store.add_tenant(tenant);

        let config_auth = ApiKeyAuth::new(make_keys());
        let auth_state = AuthState::new(
            config_auth,
            Arc::new(RwLock::new(store)),
        );

        let app = Router::new()
            .route("/api/v1/test", get(ok_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                tenant_aware_auth_middleware,
            ));

        // The tenant store key should be accepted
        let req = make_request("/api/v1/test", Some("ck_test_tenant_key_12345"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tenant_store_invalid_key_rejected() {
        use crate::api::tenant::{ApiKeyEntry, Tenant, TenantTier, TenantUsage};
        use chrono::Utc;

        let now = Utc::now();
        let tenant = Tenant {
            id: "org_test".to_string(),
            name: "Test Org".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![ApiKeyEntry {
                key: "ck_different_key".to_string(),
                label: "test-label".to_string(),
                created_at: now,
                last_used_at: now,
                active: true,
            }],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: now,
            last_active_at: now,
        };

        let mut store = TenantStore::new();
        store.add_tenant(tenant);

        let config_auth = ApiKeyAuth::new(make_keys());
        let auth_state = AuthState::new(
            config_auth,
            Arc::new(RwLock::new(store)),
        );

        let app = Router::new()
            .route("/api/v1/test", get(ok_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                tenant_aware_auth_middleware,
            ));

        // A key not in config or tenant store should be rejected
        let req = make_request("/api/v1/test", Some("ck_nonexistent_key"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn config_key_still_works_with_tenant_middleware() {
        let store = TenantStore::new();
        let config_auth = ApiKeyAuth::new(make_keys());
        let auth_state = AuthState::new(
            config_auth,
            Arc::new(RwLock::new(store)),
        );

        let app = Router::new()
            .route("/api/v1/test", get(ok_handler))
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                tenant_aware_auth_middleware,
            ));

        // Config key should still work
        let req = make_request("/api/v1/test", Some("secret123"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
