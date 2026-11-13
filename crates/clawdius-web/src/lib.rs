//! Clawdius web frontend: Leptos app and server API types.
//!
//! `clippy::empty_enums` is allowed crate-wide for the same leptos
//! component + `TypedBuilder` derive artifact as clawdius-ui.

#![deny(unsafe_code)]
#![allow(clippy::empty_enums)]

/// Root Leptos application (SPA shell).
pub mod app;
/// Server API request/response types and stub endpoints.
pub mod server;

pub use app::App;
pub use server::{
    get_health_status, list_models, list_sessions, send_message, HealthStatus, ModelInfo,
    SendMessageRequest, SendMessageResponse, SessionInfo,
};
