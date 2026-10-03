use serde::{Deserialize, Serialize};

/// Backend health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Health verdict (currently always `"ok"`).
    pub status: String,
    /// Backend version from the crate metadata.
    pub version: String,
}

/// Builds the current backend health report.
#[must_use]
pub fn get_health_status() -> HealthStatus {
    HealthStatus {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// One model offered in the picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Stable model identifier.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Provider offering the model.
    pub provider: String,
}

/// Lists the models available to the web client.
#[must_use]
pub fn list_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
            provider: "OpenAI".to_string(),
        },
        ModelInfo {
            id: "claude-3-opus".to_string(),
            name: "Claude 3 Opus".to_string(),
            provider: "Anthropic".to_string(),
        },
        ModelInfo {
            id: "claude-3-sonnet".to_string(),
            name: "Claude 3 Sonnet".to_string(),
            provider: "Anthropic".to_string(),
        },
    ]
}

/// Request body for sending a chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    /// Message text to send.
    pub message: String,
    /// Session to continue; a new one is implied when absent.
    pub session_id: Option<String>,
}

/// Reply body returned after a message is processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    /// Assistant reply text.
    pub response: String,
    /// Session the exchange belongs to.
    pub session_id: String,
}

/// Sends a chat message and returns the assistant reply.
pub async fn send_message(req: SendMessageRequest) -> SendMessageResponse {
    SendMessageResponse {
        response: format!("Echo: {}", req.message),
        session_id: req.session_id.unwrap_or_default(),
    }
}

/// One chat session known to the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Unique session identifier.
    pub id: String,
    /// Display title of the session.
    pub title: String,
    /// Creation time formatted for display.
    pub created_at: String,
}

/// Lists the sessions known to the backend (currently empty).
#[must_use]
pub const fn list_sessions() -> Vec<SessionInfo> {
    Vec::new()
}
