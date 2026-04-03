//! RPC request handlers

pub mod completion;

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, RwLock};
use tracing::warn;

use super::types::{Error, Request, Response};
use crate::context::{ContextCompactor, ContextItem};
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use crate::session::{Message, Session, SessionId};

pub use completion::CompletionHandler;

#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self, request: Request) -> Response;
}

type InMemorySessions = Arc<RwLock<std::collections::HashMap<String, Session>>>;

pub struct SessionHandler {
    sessions: InMemorySessions,
}

impl SessionHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_store(sessions: InMemorySessions) -> Self {
        Self { sessions }
    }

    /// Clone the shared sessions arc for creating multiple handlers that share state.
    #[must_use]
    pub fn sessions_clone(&self) -> InMemorySessions {
        Arc::clone(&self.sessions)
    }
}

impl Default for SessionHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for SessionHandler {
    async fn handle(&self, request: Request) -> Response {
        match request.method.as_str() {
            "session/list" => self.handle_list(request).await,
            "session/load" => self.handle_load(request).await,
            "session/create" => self.handle_create(request).await,
            "session/save" => self.handle_save(request).await,
            "session/delete" => self.handle_delete(request).await,
            _ => Response::method_not_found(request.id, &request.method),
        }
    }
}

impl SessionHandler {
    async fn handle_list(&self, request: Request) -> Response {
        let sessions = self.sessions.read().await;
        let mut items: Vec<serde_json::Value> = sessions
            .values()
            .map(|s| {
                serde_json::json!({
                    "id": s.id.to_string(),
                    "title": s.title.clone().unwrap_or_default(),
                    "updatedAt": s.updated_at.to_rfc3339(),
                })
            })
            .collect();
        items.sort_by(|a, b| {
            let b_ts = b.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("");
            let a_ts = a.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("");
            b_ts.cmp(a_ts)
        });
        Response::success(request.id, serde_json::json!({ "result": items }))
    }

    async fn handle_load(&self, request: Request) -> Response {
        let id_str = match request
            .params
            .as_ref()
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(s) => s.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'id' parameter"),
        };

        let sessions = self.sessions.read().await;
        let session = match sessions.get(&id_str) {
            Some(s) => s.clone(),
            None => {
                return Response::error(
                    request.id,
                    Error::server_error(-32010, format!("Session '{id_str}' not found")),
                )
            },
        };

        let messages: Vec<serde_json::Value> = session
            .messages
            .into_iter()
            .map(|m| {
                let content = match m.content {
                    crate::session::MessageContent::Text(t) => serde_json::Value::String(t),
                    crate::session::MessageContent::MultiPart(parts) => {
                        serde_json::to_value(parts).unwrap_or(serde_json::Value::Null)
                    },
                };
                serde_json::json!({
                    "id": m.id.to_string(),
                    "role": format!("{:?}", m.role).to_lowercase(),
                    "content": content,
                    "createdAt": m.created_at.to_rfc3339(),
                })
            })
            .collect();

        Response::success(
            request.id,
            serde_json::json!({
                "id": session.id.to_string(),
                "title": session.title.unwrap_or_default(),
                "messages": messages,
            }),
        )
    }

    async fn handle_create(&self, request: Request) -> Response {
        let session = Session::new();
        let id_str = session.id.to_string();

        let mut sessions = self.sessions.write().await;
        sessions.insert(id_str.clone(), session);

        Response::success(request.id, serde_json::json!({ "id": id_str }))
    }

    async fn handle_save(&self, request: Request) -> Response {
        Response::success(request.id, serde_json::json!({"status": "ok"}))
    }

    async fn handle_delete(&self, request: Request) -> Response {
        let id_str = match request
            .params
            .as_ref()
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(s) => s.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'id' parameter"),
        };

        let mut sessions = self.sessions.write().await;
        if sessions.remove(&id_str).is_some() {
            Response::success(request.id, serde_json::json!({"status": "ok"}))
        } else {
            Response::error(
                request.id,
                Error::server_error(-32010, format!("Session '{id_str}' not found")),
            )
        }
    }
}

pub struct ChatHandler {
    llm: Option<Arc<dyn LlmClient>>,
    cancel_token: Arc<Notify>,
}

impl ChatHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            llm: None,
            cancel_token: Arc::new(Notify::new()),
        }
    }

    pub fn with_llm(llm: Arc<dyn LlmClient>) -> Self {
        Self {
            llm: Some(llm),
            cancel_token: Arc::new(Notify::new()),
        }
    }

    #[must_use]
    pub fn with_llm_opt(llm: Option<Arc<dyn LlmClient>>) -> Self {
        Self {
            llm,
            cancel_token: Arc::new(Notify::new()),
        }
    }

    #[must_use]
    pub fn cancel_token(&self) -> Arc<Notify> {
        Arc::clone(&self.cancel_token)
    }
}

impl Default for ChatHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for ChatHandler {
    async fn handle(&self, request: Request) -> Response {
        match request.method.as_str() {
            "chat/send" => self.handle_send(request).await,
            "chat/stream" => self.handle_stream(request).await,
            "chat/cancel" => self.handle_cancel(request).await,
            _ => Response::method_not_found(request.id, &request.method),
        }
    }
}

impl ChatHandler {
    async fn handle_send(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let message = match params.get("message").and_then(|v| v.as_str()) {
            Some(m) => m.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'message' parameter"),
        };

        let _session_id = params.get("sessionId").and_then(|v| v.as_str());

        let llm = match &self.llm {
            Some(llm) => llm,
            None => {
                return Response::error(
                    request.id,
                    Error::server_error(-32000, "No LLM client configured"),
                )
            },
        };

        let system_prompt =
            "You are a helpful coding assistant. Provide clear, concise, and accurate responses. \
            When writing code, follow best practices and include error handling.";

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: message,
            },
        ];

        let result = tokio::time::timeout(Duration::from_secs(30), llm.chat(messages)).await;

        match result {
            Ok(Ok(response)) => {
                Response::success(request.id, serde_json::json!({"content": response}))
            },
            Ok(Err(e)) => {
                warn!("Chat LLM error: {e}");
                Response::internal_error(request.id, format!("LLM error: {e}"))
            },
            Err(_) => {
                warn!("Chat LLM timed out");
                Response::error(
                    request.id,
                    Error::server_error(-32001, "Chat request timed out after 30 seconds"),
                )
            },
        }
    }

    async fn handle_stream(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let message = match params.get("message").and_then(|v| v.as_str()) {
            Some(m) => m.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'message' parameter"),
        };

        let llm = match &self.llm {
            Some(llm) => llm,
            None => {
                return Response::error(
                    request.id,
                    Error::server_error(-32000, "No LLM client configured"),
                )
            },
        };

        let system_prompt =
            "You are a helpful coding assistant. Provide clear, concise, and accurate responses. \
             When writing code, follow best practices and include error handling.";

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: message,
            },
        ];

        let result = tokio::time::timeout(Duration::from_secs(60), llm.chat_stream(messages)).await;

        match result {
            Ok(Ok(mut rx)) => {
                let mut full_content = String::new();
                loop {
                    tokio::select! {
                        chunk = rx.recv() => {
                            match chunk {
                                Some(token) => full_content.push_str(&token),
                                None => break,
                            }
                        }
                        _ = self.cancel_token.notified() => {
                            return Response::success(
                                request.id,
                                serde_json::json!({
                                    "content": full_content,
                                    "streamed": true,
                                    "cancelled": true,
                                }),
                            );
                        }
                    }
                }
                Response::success(
                    request.id,
                    serde_json::json!({
                        "content": full_content,
                        "streamed": true,
                        "cancelled": false,
                    }),
                )
            },
            Ok(Err(e)) => {
                warn!("Chat stream LLM error: {e}");
                Response::internal_error(request.id, format!("LLM error: {e}"))
            },
            Err(_) => {
                warn!("Chat stream timed out");
                Response::error(
                    request.id,
                    Error::server_error(-32001, "Chat stream timed out after 60 seconds"),
                )
            },
        }
    }

    async fn handle_cancel(&self, request: Request) -> Response {
        self.cancel_token.notify_waiters();
        Response::success(request.id, serde_json::json!({"cancelled": true}))
    }
}

pub struct FileHandler;

#[async_trait]
impl Handler for FileHandler {
    async fn handle(&self, request: Request) -> Response {
        match request.method.as_str() {
            "file/read" => self.handle_read(request).await,
            "file/write" => self.handle_write(request).await,
            _ => Response::method_not_found(request.id, &request.method),
        }
    }
}

impl FileHandler {
    async fn handle_read(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing 'path' parameter"),
        };

        match tokio::fs::read_to_string(path).await {
            Ok(content) => Response::success(request.id, serde_json::json!({"content": content})),
            Err(e) => {
                Response::internal_error(request.id, format!("Failed to read file '{path}': {e}"))
            },
        }
    }

    async fn handle_write(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing 'path' parameter"),
        };

        let content = match params.get("content") {
            Some(v) => v.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'content' parameter"),
        };

        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Response::internal_error(
                        request.id,
                        format!("Failed to create directories for '{path}': {e}"),
                    );
                }
            }
        }

        match tokio::fs::write(path, content.as_bytes()).await {
            Ok(()) => Response::success(request.id, serde_json::json!({"status": "ok"})),
            Err(e) => {
                Response::internal_error(request.id, format!("Failed to write file '{path}': {e}"))
            },
        }
    }
}

pub struct ContextHandler {
    items: Arc<RwLock<Vec<ContextItem>>>,
}

impl ContextHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for ContextHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for ContextHandler {
    async fn handle(&self, request: Request) -> Response {
        match request.method.as_str() {
            "context/add" => self.handle_add(request).await,
            "context/remove" => self.handle_remove(request).await,
            "context/list" => self.handle_list(request).await,
            "context/compact" => self.handle_compact(request).await,
            _ => Response::method_not_found(request.id, &request.method),
        }
    }
}

impl ContextHandler {
    async fn handle_add(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let item_type = match params.get("type").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'type' parameter"),
        };

        let source = match params.get("source").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'source' parameter"),
        };

        let item = match item_type.as_str() {
            "file" => {
                let content = tokio::fs::read_to_string(&source).await.unwrap_or_default();
                let language = std::path::Path::new(&source)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(std::string::ToString::to_string);
                ContextItem::File {
                    path: source,
                    content,
                    language,
                }
            },
            "folder" => {
                let files = match tokio::fs::read_dir(&source).await {
                    Ok(mut entries) => {
                        let mut names = Vec::new();
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            if let Some(name) = entry.file_name().to_str() {
                                names.push(name.to_string());
                            }
                        }
                        names.sort();
                        names
                    },
                    Err(_) => Vec::new(),
                };
                ContextItem::Folder {
                    path: source,
                    files,
                }
            },
            "url" => ContextItem::Url {
                url: source,
                content: String::new(),
                title: None,
            },
            _ => {
                return Response::invalid_params(
                    request.id,
                    format!("Unknown context type: '{item_type}'"),
                )
            },
        };

        self.items.write().await.push(item);
        Response::success(request.id, serde_json::json!({"status": "ok"}))
    }

    async fn handle_remove(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let source = match params.get("source").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Response::invalid_params(request.id, "Missing 'source' parameter"),
        };

        let mut items = self.items.write().await;
        let before = items.len();
        items.retain(|item| match item {
            ContextItem::File { path, .. } => path != &source,
            ContextItem::Folder { path, .. } => path != &source,
            ContextItem::Url { url, .. } => url != &source,
            _ => true,
        });

        if items.len() == before {
            Response::error(
                request.id,
                Error::server_error(-32011, format!("Context item '{source}' not found")),
            )
        } else {
            Response::success(request.id, serde_json::json!({"status": "ok"}))
        }
    }

    async fn handle_list(&self, request: Request) -> Response {
        let items = self.items.read().await;
        let serialized: Vec<serde_json::Value> = items
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or_default())
            .collect();
        Response::success(request.id, serde_json::json!({ "items": serialized }))
    }

    async fn handle_compact(&self, request: Request) -> Response {
        let compactor = match ContextCompactor::with_defaults() {
            Ok(c) => c,
            Err(e) => {
                return Response::internal_error(
                    request.id,
                    format!("Failed to create compactor: {e}"),
                )
            },
        };

        let mut items = self.items.write().await;
        let tokens_before: usize = items
            .iter()
            .map(|i| compactor.estimate_item_tokens(i))
            .sum();

        match compactor.compact(std::mem::take(&mut items)) {
            Ok((compacted, result)) => {
                *items = compacted;
                Response::success(
                    request.id,
                    serde_json::json!({
                        "status": "ok",
                        "tokens_before": result.tokens_before,
                        "tokens_after": result.tokens_after,
                        "compacted_count": result.compacted_count,
                    }),
                )
            },
            Err(e) => Response::internal_error(request.id, format!("Compaction failed: {e}")),
        }
    }
}

pub struct StateHandler;

#[async_trait]
impl Handler for StateHandler {
    async fn handle(&self, request: Request) -> Response {
        if request.method == "state/checkpoint" {
            Response::success(request.id, serde_json::json!({"id": "checkpoint-1"}))
        } else {
            Response::success(request.id, serde_json::json!({"status": "ok"}))
        }
    }
}
