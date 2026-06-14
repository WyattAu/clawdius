//! Authentication and authorization for the Clawdius platform.
//!
//! Provides OpenID Connect integration, JWT session management, Axum
//! middleware, and HTTP handlers for login, callback, logout, refresh,
//! and current-user endpoints.

/// Configuration types for OIDC providers and auth sessions.
pub mod config;
/// Axum route handlers for authentication endpoints.
pub mod handlers;
/// Axum extractors and error types for authenticated requests.
pub mod middleware;
/// Core authentication service for OIDC flows and JWT sessions.
pub mod service;
/// User identity and session claim structures.
pub mod user;

pub use config::{AuthConfig, OidcProviderConfig};
pub use handlers::auth_routes;
pub use middleware::{AuthError, AuthUser};
pub use service::{AuthService, TokenResult};
pub use user::{SessionClaims, UserInfo};
