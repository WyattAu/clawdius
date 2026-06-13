//! Tauri command definitions and handlers.
//!
//! Provides the IPC bridge between the Tauri webview frontend and the
//! clawdius-core backend. Each command is a stub that will be wired to
//! clawdius-core functions in a future iteration.

use serde::{Deserialize, Serialize};

/// Health check response returned by the `health_check` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status string (e.g. "ok").
    pub status: String,
    /// Crate version from `CARGO_PKG_VERSION`.
    pub version: String,
}

/// Information about a single LLM model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g. "gpt-4o").
    pub name: String,
    /// Provider name (e.g. "openai").
    pub provider: String,
    /// Whether the model is currently reachable.
    pub available: bool,
}

/// Response returned after sending a message to a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    /// The assistant reply text.
    pub response: String,
    /// The session the message was sent to.
    pub session_id: String,
}

/// Summary of a conversation session for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Unique session identifier.
    pub id: String,
    /// Display title of the session.
    pub title: String,
    /// Number of messages in the session.
    pub message_count: usize,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Full detail of a conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    /// Unique session identifier.
    pub id: String,
    /// Display title of the session.
    pub title: String,
    /// Messages in chronological order.
    pub messages: Vec<SessionMessage>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
}

/// A single message within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Message role (e.g. "user", "assistant").
    pub role: String,
    /// Message content text.
    pub content: String,
}

/// Returns a health check payload indicating the service is alive.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn health_check() -> Result<HealthResponse, String> {
    Ok(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Returns the list of available LLM models.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_models() -> Result<Vec<ModelInfo>, String> {
    Ok(vec![
        ModelInfo {
            name: "claude-sonnet-4-20250514".to_string(),
            provider: "anthropic".to_string(),
            available: true,
        },
        ModelInfo {
            name: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            available: true,
        },
    ])
}

/// Sends a user message to a session and returns the assistant reply.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn send_message(session_id: String, message: String) -> Result<SendMessageResponse, String> {
    let _ = message;
    Ok(SendMessageResponse {
        response: "stub response".to_string(),
        session_id,
    })
}

/// Returns a summary list of all sessions.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_sessions() -> Result<Vec<SessionSummary>, String> {
    Ok(vec![])
}

/// Returns the full detail of a single session by id.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_session(id: String) -> Result<SessionDetail, String> {
    let _ = id;
    Err("session not found".to_string())
}

/// Initializes and runs the Tauri application with all plugins and commands registered.
#[cfg(feature = "desktop")]
pub fn run_tauri_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            health_check,
            list_models,
            send_message,
            list_sessions,
            get_session,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}
