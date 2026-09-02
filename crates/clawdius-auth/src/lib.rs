//! Authentication and authorization for the Clawdius platform.
//!
//! Provides OpenID Connect integration, SAML 2.0 Service Provider,
//! Role-Based Access Control (RBAC), JWT session management, Axum
//! middleware, and HTTP handlers for login, callback, logout, refresh,
//! and current-user endpoints.

#![deny(unsafe_code)]

/// Configuration types for OIDC providers and auth sessions.
pub mod config;
/// Axum route handlers for authentication endpoints.
pub mod handlers;
/// Axum extractors and error types for authenticated requests.
pub mod middleware;
/// Role-Based Access Control (RBAC) with 23 permissions and 4 roles.
pub mod rbac;
/// SAML 2.0 Service Provider implementation.
pub mod saml;
/// Core authentication service for OIDC flows and JWT sessions.
pub mod service;
/// User identity and session claim structures.
pub mod user;

pub use config::{AuthConfig, OidcProviderConfig};
pub use handlers::auth_routes;
pub use middleware::{AuthError, AuthUser};
pub use rbac::{
    RbacError, RbacPolicy, RbacService, RequirePermission, RequirePermissionGuard, Role,
};
pub use saml::{saml_routes, SamlAssertion, SamlError, SamlSpConfig};
pub use service::{AuthService, TokenResult};
pub use user::{SessionClaims, UserInfo};
