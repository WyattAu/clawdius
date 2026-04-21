//! Authentication and tenant management API handlers.
//!
//! Endpoints:
//! - POST /api/v1/auth/signup — Create a new tenant account
//! - POST /api/v1/auth/login — Authenticate with API key, get tenant info
//! - GET  /api/v1/tenants — List tenants (admin only)
//! - GET  /api/v1/tenants/{id} — Get tenant details
//! - PATCH /api/v1/tenants/{id} — Update tenant
//! - DELETE /api/v1/tenants/{id} — Delete tenant
//! - POST /api/v1/tenants/{id}/keys — Create API key
//! - GET  /api/v1/tenants/{id}/keys — List API keys
//! - DELETE /api/v1/tenants/{id}/keys/{key} — Revoke API key

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::rest::{ApiError, ApiState};
use crate::api::tenant::{ApiKeyEntry, AuthenticatedApiKey, Tenant, TenantTier, TenantUsage};

// ── Request/Response Types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    /// Tenant name
    pub name: String,
    /// Contact email (optional)
    #[serde(default)]
    pub email: Option<String>,
    /// Desired tier (default: "free")
    #[serde(default)]
    pub tier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub tenant_id: String,
    pub name: String,
    pub tier: String,
    pub api_key: String,
    pub api_key_label: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// API key to authenticate with
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub tenant_id: String,
    pub name: String,
    pub tier: String,
    pub email: Option<String>,
    pub workspace_root: Option<String>,
    pub usage: TenantUsage,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub email: Option<String>,
    pub workspace_root: Option<String>,
    pub api_keys: Vec<ApiKeyInfo>,
    pub usage: TenantUsage,
    pub created_at: String,
    pub last_active_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyInfo {
    pub key_masked: String,
    pub label: String,
    pub created_at: String,
    pub last_used_at: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub workspace_root: Option<String>,
}

fn default_label() -> String {
    "default".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Label for the API key
    #[serde(default = "default_label")]
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyCreatedResponse {
    pub key: String,
    pub key_masked: String,
    pub label: String,
    pub message: String,
}

// ── Auth Endpoints ──────────────────────────────────────────────────────────────

/// POST /api/v1/auth/signup — Create a new tenant account.
pub async fn signup(
    State(state): State<ApiState>,
    Json(request): Json<SignupRequest>,
) -> Result<(StatusCode, Json<SignupResponse>), (StatusCode, Json<ApiError>)> {
    let uuid_str = uuid::Uuid::new_v4().to_string().replace('-', "");
    let tenant_id = format!("org_{}", &uuid_str[..16]);

    let tier = request
        .tier
        .as_deref()
        .and_then(TenantTier::from_str_opt)
        .unwrap_or(TenantTier::Free);

    // Create an API key
    let api_key_entry = ApiKeyEntry::new(&request.name);
    let api_key = api_key_entry.key.clone();
    let api_key_label = api_key_entry.label.clone();

    let tenant = Tenant {
        id: tenant_id.clone(),
        name: request.name.clone(),
        tier: tier.clone(),
        api_keys: vec![api_key_entry],
        email: request.email.clone(),
        workspace_root: None,
        usage: TenantUsage::default(),
        created_at: chrono::Utc::now(),
        last_active_at: chrono::Utc::now(),
    };

    {
        let mut store = state.tenant_store.write().unwrap();
        store.add_tenant(tenant);
    }

    Ok((
        StatusCode::CREATED,
        Json(SignupResponse {
            tenant_id,
            name: request.name,
            tier: tier.to_string(),
            api_key,
            api_key_label,
            message: "Tenant created. Save your API key — you won't be able to see it again.".to_string(),
        }),
    ))
}

/// POST /api/v1/auth/login — Authenticate with an API key.
pub async fn login(
    State(state): State<ApiState>,
    Json(request): Json<LoginRequest>,
) -> Result<(StatusCode, Json<LoginResponse>), (StatusCode, Json<ApiError>)> {
    // First pass: validate the API key and get tenant_id
    let tenant_id = {
        let store = state.tenant_store.read().unwrap();
        let tenant = store
            .get_tenant_by_api_key(&request.api_key)
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(ApiError {
                code: "UNAUTHORIZED".to_string(),
                message: "Invalid API key".to_string(),
            })))?;
        tenant.id.clone()
    };

    // Second pass: update last used timestamp
    {
        let mut store = state.tenant_store.write().unwrap();
        if let Some(tenant) = store.get_tenant_mut(&tenant_id) {
            if let Some(entry) = tenant
                .api_keys
                .iter_mut()
                .find(|k| k.key == request.api_key)
            {
                entry.last_used_at = chrono::Utc::now();
            }
        }
    }

    // Third pass: read the tenant for the response
    let store = state.tenant_store.read().unwrap();
    let tenant = store.get_tenant(&tenant_id).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                code: "INTERNAL_ERROR".to_string(),
                message: "Tenant disappeared after authentication".to_string(),
            }),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            tenant_id: tenant.id.clone(),
            name: tenant.name.clone(),
            tier: tenant.tier.to_string(),
            email: tenant.email.clone(),
            workspace_root: tenant.workspace_root.clone(),
            usage: tenant.usage.clone(),
            message: "Authenticated successfully".to_string(),
        }),
    ))
}

// ── Tenant Endpoints ──────────────────────────────────────────────────────────

/// GET /api/v1/tenants — List all tenants (requires auth).
pub async fn list_tenants(
    State(state): State<ApiState>,
) -> Json<Vec<TenantResponse>> {
    let store = state.tenant_store.read().unwrap();
    let tenants = store
        .list_tenants()
        .into_iter()
        .map(|t| tenant_to_response(t))
        .collect();
    Json(tenants)
}

/// GET /api/v1/tenants/{id} — Get tenant details.
pub async fn get_tenant(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<TenantResponse>, (StatusCode, Json<ApiError>)> {
    let store = state.tenant_store.read().unwrap();
    let tenant = store.get_tenant(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "NOT_FOUND".to_string(),
                message: format!("Tenant '{id}' not found"),
            }),
        )
    })?;

    Ok(Json(tenant_to_response(tenant)))
}

/// PATCH /api/v1/tenants/{id} — Update a tenant.
pub async fn update_tenant(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(request): Json<UpdateTenantRequest>,
) -> Result<Json<TenantResponse>, (StatusCode, Json<ApiError>)> {
    let tier = request
        .tier
        .as_deref()
        .and_then(TenantTier::from_str_opt);

    {
        let mut store = state.tenant_store.write().unwrap();
        store.update_tenant(
            &id,
            request.name.as_deref(),
            tier,
            request.email.as_deref(),
            request.workspace_root.as_deref(),
        )
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    code: "NOT_FOUND".to_string(),
                    message: format!("Tenant '{id}' not found"),
                }),
            )
        })?;
    }

    let store = state.tenant_store.read().unwrap();
    let tenant = store.get_tenant(&id).unwrap();
    Ok(Json(tenant_to_response(tenant)))
}

/// DELETE /api/v1/tenants/{id} — Delete a tenant and all its data.
pub async fn delete_tenant(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ApiError>)> {
    {
        let mut store = state.tenant_store.write().unwrap();
        store.delete_tenant(&id)
            .then_some(())
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ApiError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("Tenant '{id}' not found"),
                    }),
                )
            })?;
    }

    Ok((
        StatusCode::NO_CONTENT,
        Json(serde_json::json!({
            "message": format!("Tenant '{id}' deleted successfully"),
        })),
    ))
}

// ── API Key Management ──────────────────────────────────────────────────────────

/// POST /api/v1/tenants/{id}/keys — Create a new API key for a tenant.
pub async fn create_api_key(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyCreatedResponse>), (StatusCode, Json<ApiError>)> {
    let label = if request.label.is_empty() {
        "default"
    } else {
        &request.label
    };

    let key_entry = {
        let mut store = state.tenant_store.write().unwrap();
        store.add_api_key(&id, label).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    code: "NOT_FOUND".to_string(),
                    message: format!("Tenant '{id}' not found"),
                }),
            )
        })?
    };

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyCreatedResponse {
            key: key_entry.key.clone(),
            key_masked: key_entry.masked(),
            label: key_entry.label,
            message: "Save this API key — you won't be able to see it again.".to_string(),
        }),
    ))
}

/// GET /api/v1/tenants/{id}/keys — List API keys for a tenant.
pub async fn list_api_keys(
    State(state): State<ApiState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<ApiKeyInfo>>, (StatusCode, Json<ApiError>)> {
    let store = state.tenant_store.read().unwrap();
    let tenant = store.get_tenant(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "NOT_FOUND".to_string(),
                message: format!("Tenant '{id}' not found"),
            }),
        )
    })?;

    let keys: Vec<ApiKeyInfo> = tenant
        .api_keys
        .iter()
        .map(|k| ApiKeyInfo {
            key_masked: k.masked(),
            label: k.label.clone(),
            created_at: k.created_at.to_rfc3339(),
            last_used_at: k.last_used_at.to_rfc3339(),
            active: k.active,
        })
        .collect();

    Ok(Json(keys))
}

/// DELETE /api/v1/tenants/{id}/keys/{key} — Revoke an API key.
pub async fn revoke_api_key(
    State(state): State<ApiState>,
    axum::extract::Path(params): axum::extract::Path<std::collections::HashMap<String, String>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ApiError>)> {
    let tenant_id = params.get("id").cloned().unwrap_or_default();
    let key = params.get("key").cloned().unwrap_or_default();

    let success = {
        let mut store = state.tenant_store.write().unwrap();
        store.revoke_api_key(&tenant_id, &key)
    };

    if !success {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "NOT_FOUND".to_string(),
                message: "API key not found or tenant not found".to_string(),
            }),
        ));
    }

    Ok((
        StatusCode::NO_CONTENT,
        Json(serde_json::json!({
            "message": "API key revoked successfully",
        })),
    ))
}

// ── Helpers ──────────────────────────────────────────────────────────────────────

fn tenant_to_response(tenant: &Tenant) -> TenantResponse {
    TenantResponse {
        id: tenant.id.clone(),
        name: tenant.name.clone(),
        tier: tenant.tier.to_string(),
        email: tenant.email.clone(),
        workspace_root: tenant.workspace_root.clone(),
        api_keys: tenant
            .api_keys
            .iter()
            .map(|k| ApiKeyInfo {
                key_masked: k.masked(),
                label: k.label.clone(),
                created_at: k.created_at.to_rfc3339(),
                last_used_at: k.last_used_at.to_rfc3339(),
                active: k.active,
            })
            .collect(),
        usage: tenant.usage.clone(),
        created_at: tenant.created_at.to_rfc3339(),
        last_active_at: tenant.last_active_at.to_rfc3339(),
    }
}

/// Record a task (LLM request) against a tenant.
/// Returns false if rate limit would be exceeded.
pub fn record_tenant_task(
    state: &ApiState,
    tenant_id: &str,
    tokens: usize,
) -> bool {
    let mut store = state.tenant_store.write().unwrap();
    store.record_task(tenant_id, tokens)
}
