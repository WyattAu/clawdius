//! Clawdius web frontend: Leptos app and server API types.

#![deny(unsafe_code)]

/// Root Leptos application (SPA shell).
pub mod app;
/// Server API request/response types and stub endpoints.
pub mod server;

pub use app::App;
pub use server::{
    get_health_status, list_models, list_sessions, send_message, HealthStatus, ModelInfo,
    SendMessageRequest, SendMessageResponse, SessionInfo,
};
