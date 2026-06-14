//! Tauri command definitions and handlers.
//!
//! Provides the IPC bridge between the Tauri webview frontend and the
//! clawdius-core backend. Commands are wired to real core functionality
//! where possible, with realistic fallbacks for complex integrations.

use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
use std::time::Instant;

#[cfg(feature = "desktop")]
static STARTUP_INSTANT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

#[cfg(feature = "desktop")]
fn startup_instant() -> &'static Instant {
    STARTUP_INSTANT.get_or_init(Instant::now)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub provider_count: usize,
    pub session_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String,
    pub available: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub response: String,
    pub session_id: String,
    pub model: String,
    pub tokens_used: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub created_at: String,
    pub updated_at: String,
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub id: String,
    pub title: String,
    pub messages: Vec<SessionMessage>,
    pub created_at: String,
    pub updated_at: String,
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub session_id: Option<String>,
    pub message: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[cfg(feature = "desktop")]
fn default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            name: "claude-sonnet-4-20250514".into(),
            provider: "anthropic".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into(), "tools".into()],
        },
        ModelInfo {
            name: "gpt-4o".into(),
            provider: "openai".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into(), "tools".into()],
        },
        ModelInfo {
            name: "gemini-2.5-flash".into(),
            provider: "google".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into()],
        },
        ModelInfo {
            name: "deepseek-chat".into(),
            provider: "deepseek".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into(), "tools".into()],
        },
        ModelInfo {
            name: "llama3.2".into(),
            provider: "ollama".into(),
            available: false,
            capabilities: vec!["chat".into(), "streaming".into()],
        },
    ]
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn health_check() -> Result<HealthResponse, String> {
    let uptime = startup_instant().elapsed().as_secs();
    let provider_count = default_models()
        .iter()
        .map(|m| m.provider.clone())
        .collect::<std::collections::HashSet<_>>()
        .len();

    Ok(HealthResponse {
        status: "ok".to_string(),
        version: clawdius_core::VERSION.to_string(),
        uptime_secs: uptime,
        provider_count,
        session_count: 0,
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_models() -> Result<Vec<ModelInfo>, String> {
    let config = clawdius_core::Config::load_or_default();

    let mut models = Vec::new();

    if config.llm.anthropic.is_some() || std::env::var("ANTHROPIC_API_KEY").is_ok() {
        let model = config
            .llm
            .anthropic
            .as_ref()
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        models.push(ModelInfo {
            name: model,
            provider: "anthropic".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into(), "tools".into()],
        });
    }

    if config.llm.openai.is_some() || std::env::var("OPENAI_API_KEY").is_ok() {
        let model = config
            .llm
            .openai
            .as_ref()
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| "gpt-4o".to_string());
        models.push(ModelInfo {
            name: model,
            provider: "openai".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into(), "tools".into()],
        });
    }

    if config.llm.google.is_some() || std::env::var("GOOGLE_API_KEY").is_ok() {
        let model = config
            .llm
            .google
            .as_ref()
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| "gemini-2.5-flash".to_string());
        models.push(ModelInfo {
            name: model,
            provider: "google".into(),
            available: true,
            capabilities: vec!["chat".into(), "streaming".into()],
        });
    }

    if config.llm.ollama.is_some() {
        let model = config
            .llm
            .ollama
            .as_ref()
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| "llama3.2".to_string());
        models.push(ModelInfo {
            name: model,
            provider: "ollama".into(),
            available: false,
            capabilities: vec!["chat".into(), "streaming".into()],
        });
    }

    if models.is_empty() {
        models = default_models();
    }

    Ok(models)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn send_message(request: SendMessageRequest) -> Result<SendMessageResponse, String> {
    let provider_name = request.provider.unwrap_or_else(|| "anthropic".to_string());
    let model = request.model.unwrap_or_else(|| "default".to_string());

    let config = clawdius_core::Config::load_or_default();
    let llm_config =
        clawdius_core::llm::ResolvedLlmConfig::from_config(&config.llm, &provider_name)
            .map_err(|e| e.to_string())?;

    let client = clawdius_core::llm::create_provider_with_retry(&llm_config, None)
        .map_err(|e| e.to_string())?;

    let messages = vec![clawdius_core::llm::ChatMessage {
        role: clawdius_core::llm::ChatRole::User,
        content: request.message,
    }];

    let response = client.chat(messages).await.map_err(|e| e.to_string())?;

    let session_id = request
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    Ok(SendMessageResponse {
        response,
        session_id,
        model,
        tokens_used: None,
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_sessions() -> Result<Vec<SessionSummary>, String> {
    let config = clawdius_core::Config::load_or_default();
    let sessions_path = &config.storage.sessions_path;

    if !sessions_path.exists() {
        return Ok(vec![]);
    }

    let store = clawdius_core::session::SessionStore::open(sessions_path)
        .map_err(|e| format!("failed to open session store: {e}"))?;

    let sessions = store
        .list_sessions()
        .map_err(|e| format!("failed to list sessions: {e}"))?;

    let summaries = sessions
        .into_iter()
        .map(|s| SessionSummary {
            id: s.id.to_string(),
            title: s.title.unwrap_or_else(|| "Untitled".to_string()),
            message_count: s.messages.len(),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
            model: s.meta.model,
            provider: s.meta.provider,
        })
        .collect();

    Ok(summaries)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_session(id: String) -> Result<SessionDetail, String> {
    let config = clawdius_core::Config::load_or_default();
    let sessions_path = &config.storage.sessions_path;

    let store = clawdius_core::session::SessionStore::open(sessions_path)
        .map_err(|e| format!("failed to open session store: {e}"))?;

    let session_id: clawdius_core::session::SessionId = id
        .parse()
        .map_err(|e: uuid::Error| format!("invalid session id: {e}"))?;

    let session = store
        .load_session_full(&session_id)
        .map_err(|e| format!("failed to load session: {e}"))?
        .ok_or_else(|| "session not found".to_string())?;

    let messages = session
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role {
                clawdius_core::session::types::MessageRole::User => "user".to_string(),
                clawdius_core::session::types::MessageRole::Assistant => "assistant".to_string(),
                clawdius_core::session::types::MessageRole::System => "system".to_string(),
                clawdius_core::session::types::MessageRole::Tool => "tool".to_string(),
            };
            let content = match m.content {
                clawdius_core::session::types::MessageContent::Text(t) => t,
                clawdius_core::session::types::MessageContent::MultiPart(parts) => parts
                    .into_iter()
                    .filter_map(|p| match p {
                        clawdius_core::session::types::ContentPart::Text { text } => Some(text),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            SessionMessage {
                role,
                content,
                timestamp: m.created_at.to_rfc3339(),
                tokens: m.tokens,
            }
        })
        .collect();

    Ok(SessionDetail {
        id: session.id.to_string(),
        title: session.title.unwrap_or_else(|| "Untitled".to_string()),
        messages,
        created_at: session.created_at.to_rfc3339(),
        updated_at: session.updated_at.to_rfc3339(),
        model: session.meta.model,
        provider: session.meta.provider,
    })
}

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
