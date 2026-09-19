// Unwrap purge batch 2: api module — production code must not
// unwrap/expect; propagate, use poison-recovery for lock poisoning, or
// restructure. Lints inherit into all child modules (handlers, RAG, tests).
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod auth;
pub mod auth_handler;
pub mod gateway;
pub mod metrics_handler;
pub mod rate_limit;
pub mod rest;
pub mod routes;
pub mod sprint_handler;
pub mod tenant;

pub use auth::ApiKeyAuth;
pub use auth_handler::{
    create_api_key, delete_tenant, get_tenant, list_api_keys, list_tenants, login,
    record_tenant_task, revoke_api_key, signup, update_tenant, ApiKeyCreatedResponse, ApiKeyInfo,
    LoginRequest, LoginResponse, SignupRequest, SignupResponse, TenantResponse,
    UpdateTenantRequest,
};
pub use gateway::*;
pub use rate_limit::ApiRateLimiter;
pub use rest::*;
pub use routes::*;
pub use sprint_handler::{
    execute_skill, generate_commit_message, list_skills, list_sprint_sessions, run_pre_ship_checks,
    run_sprint, submit_sprint_session,
};
pub use tenant::{AuthenticatedApiKey, Tenant, TenantStore, TenantTier};
