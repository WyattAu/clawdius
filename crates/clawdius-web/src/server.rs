use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
}

pub fn get_health_status() -> HealthStatus {
    HealthStatus {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub response: String,
    pub session_id: String,
}

pub async fn send_message(req: SendMessageRequest) -> SendMessageResponse {
    SendMessageResponse {
        response: format!("Echo: {}", req.message),
        session_id: req.session_id.unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

pub fn list_sessions() -> Vec<SessionInfo> {
    vec![]
}
