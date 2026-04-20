//! Authentication, authorisation, and session management.
//!
//! Provides JWT-based authentication, Argon2 password hashing, and
//! role-based access control (RBAC) backed by workspace membership.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Claims encoded in the JWT access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject — user UUID.
    pub sub: Uuid,
    /// Which workspace context the token is scoped to.
    pub workspace_id: Uuid,
    /// Membership role within that workspace.
    pub role: String,
    /// Issued-at timestamp (UNIX epoch).
    pub iat: i64,
    /// Expiration timestamp (UNIX epoch).
    pub exp: i64,
}

/// Parameters controlling token generation.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    /// Access token lifetime in seconds. Default: 900 (15 min).
    pub access_token_ttl_secs: i64,
    /// Refresh token lifetime in seconds. Default: 604800 (7 days).
    pub refresh_token_ttl_secs: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into()),
            access_token_ttl_secs: 900,
            refresh_token_ttl_secs: 604_800,
        }
    }
}

/// Generates a signed JWT access token.
///
/// # Errors
///
/// Returns [`jsonwebtoken::errors::Error`] on encoding failure.
pub fn create_access_token(
    user_id: Uuid,
    workspace_id: Uuid,
    role: &str,
    config: &JwtConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = JwtClaims {
        sub: user_id,
        workspace_id,
        role: role.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::seconds(config.access_token_ttl_secs)).timestamp(),
    };

    let header = jsonwebtoken::Header::default();
    jsonwebtoken::encode(&header, &claims, &jsonwebtoken::EncodingKey::from_secret(config.secret.as_bytes()))
}

/// Decodes and validates a JWT, returning the claims if valid.
///
/// # Errors
///
/// Returns [`jsonwebtoken::errors::Error`] if the token is invalid or expired.
pub fn verify_token(
    token: &str,
    config: &JwtConfig,
) -> Result<JwtClaims, jsonwebtoken::errors::Error> {
    let validation = jsonwebtoken::Validation::default();
    let token_data = jsonwebtoken::decode::<JwtClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(config.secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

/// Hashes a plaintext password using Argon2id.
///
/// # Errors
///
/// Returns [`argon2::password_hash::Error`] on hashing failure.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = argon2::Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verifies a plaintext password against an Argon2 hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match argon2::PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Actions that require authorisation checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    // ── Workspaces ──
    WorkspaceRead,
    WorkspaceUpdate,
    WorkspaceDelete,
    // ── Clients ──
    ClientCreate,
    ClientRead,
    ClientUpdate,
    ClientDelete,
    // ── Invoices ──
    InvoiceCreate,
    InvoiceRead,
    InvoiceUpdate,
    InvoiceDelete,
    InvoiceSend,
    // ── Payments ──
    PaymentCreate,
    PaymentRead,
    PaymentRefund,
    // ── Users ──
    UserInvite,
    UserRemove,
    // ── Settings ──
    SettingsRead,
    SettingsUpdate,
    // ── Audit ──
    AuditRead,
    // ── Analytics ──
    AnalyticsRead,
}

/// Checks whether the given role is granted a specific permission.
pub fn role_has_permission(role: &str, permission: Permission) -> bool {
    match role {
        "owner" => true,
        "admin" => !matches!(permission, Permission::WorkspaceDelete),
        "member" => matches!(
            permission,
            Permission::ClientCreate
                | Permission::ClientRead
                | Permission::ClientUpdate
                | Permission::InvoiceCreate
                | Permission::InvoiceRead
                | Permission::InvoiceUpdate
                | Permission::InvoiceSend
                | Permission::PaymentRead
                | Permission::SettingsRead
                | Permission::AnalyticsRead
        ),
        _ => false,
    }
}
