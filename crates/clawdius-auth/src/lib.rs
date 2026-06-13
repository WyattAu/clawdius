pub mod config;
pub mod handlers;
pub mod middleware;
pub mod service;
pub mod user;

pub use config::{AuthConfig, OidcProviderConfig};
pub use handlers::auth_routes;
pub use middleware::{AuthError, AuthUser};
pub use service::{AuthService, TokenResult};
pub use user::{SessionClaims, UserInfo};
