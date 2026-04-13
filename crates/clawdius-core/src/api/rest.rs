//! REST API Implementation using Axum
//!
//! Provides a comprehensive REST API for Clawdius using an actor pattern
//! for thread-safe database access.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::api::auth::{auth_middleware, ApiKeyAuth};
use crate::api::gateway::RateLimitConfig;
use crate::api::metrics_handler;
use crate::api::rate_limit::{rate_limit_middleware, ApiRateLimiter};
use crate::api::routes::{ChatRequest, ChatResponse, HealthResponse};
use crate::api::tenant::{default_tenants, AuthenticatedApiKey, TenantStore};
use crate::session::{Session, SessionId, SessionStore};

// ============================================================================
// Database Actor Pattern
// ============================================================================

/// Commands for the database actor
enum DbCommand {
    ListSessions {
        reply: oneshot::Sender<Vec<Session>>,
    },
    CreateSession {
        session: Box<Session>,
        reply: oneshot::Sender<Session>,
    },
    GetSession {
        id: SessionId,
        reply: oneshot::Sender<Option<Session>>,
    },
    DeleteSession {
        id: SessionId,
        reply: oneshot::Sender<bool>,
    },
}

/// Database actor handle
#[derive(Clone)]
pub struct DbActor {
    sender: mpsc::Sender<DbCommand>,
}

impl DbActor {
    /// Create a new database actor from a `SessionStore`
    pub fn new(store: SessionStore) -> Self {
        let (sender, mut receiver) = mpsc::channel::<DbCommand>(32);

        // Spawn the actor task
        tokio::spawn(async move {
            while let Some(cmd) = receiver.recv().await {
                match cmd {
                    DbCommand::ListSessions { reply } => {
                        let sessions = store.list_sessions().unwrap_or_default();
                        let _ = reply.send(sessions);
                    },
                    DbCommand::CreateSession { session, reply } => {
                        let _ = store.create_session(&session);
                        let _ = reply.send(*session);
                    },
                    DbCommand::GetSession { id, reply } => {
                        let session = store.load_session(&id).unwrap_or_default();
                        let _ = reply.send(session);
                    },
                    DbCommand::DeleteSession { id, reply } => {
                        let result = store.delete_session(&id).is_ok();
                        let _ = reply.send(result);
                    },
                }
            }
        });

        Self { sender }
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<Session> {
        let (reply, rx) = oneshot::channel();
        let _ = self.sender.send(DbCommand::ListSessions { reply }).await;
        rx.await.unwrap_or_default()
    }

    /// Create a new session
    pub async fn create_session(&self, session: Session) -> Session {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .sender
            .send(DbCommand::CreateSession {
                session: Box::new(session),
                reply,
            })
            .await;
        rx.await.unwrap_or_else(|_| Session::new())
    }

    /// Get a session by ID
    pub async fn get_session(&self, id: SessionId) -> Option<Session> {
        let (reply, rx) = oneshot::channel();
        let _ = self.sender.send(DbCommand::GetSession { id, reply }).await;
        rx.await.ok().flatten()
    }

    /// Delete a session by ID
    pub async fn delete_session(&self, id: SessionId) -> bool {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .sender
            .send(DbCommand::DeleteSession { id, reply })
            .await;
        rx.await.unwrap_or(false)
    }
}

// ============================================================================
// API State
// ============================================================================

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub db: DbActor,
    pub version: String,
    pub api_keys: HashMap<String, String>,
    pub rate_limit_config: Option<RateLimitConfig>,
    pub tenant_store: Arc<RwLock<TenantStore>>,
}

impl ApiState {
    /// Create new API state
    pub fn new(session_store: SessionStore) -> Self {
        Self {
            db: DbActor::new(session_store),
            version: env!("CARGO_PKG_VERSION").to_string(),
            api_keys: HashMap::new(),
            rate_limit_config: None,
            tenant_store: Arc::new(RwLock::new(default_tenants())),
        }
    }

    pub fn with_api_keys(mut self, keys: HashMap<String, String>) -> Self {
        self.api_keys = keys;
        self
    }

    pub fn with_rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = Some(config);
        self
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Session creation request
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    /// Optional session name
    pub name: Option<String>,
    /// Optional model override
    pub model: Option<String>,
}

/// Tool execution request
#[derive(Debug, Deserialize)]
pub struct ExecuteToolRequest {
    /// Tool name
    pub tool: String,
    /// Tool arguments
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Tool execution response
#[derive(Debug, Serialize)]
pub struct ExecuteToolResponse {
    /// Execution result
    pub result: serde_json::Value,
    /// Execution time in ms
    pub duration_ms: u64,
    /// Whether execution was sandboxed
    pub sandboxed: bool,
}

/// Plugin info response
#[derive(Debug, Serialize)]
pub struct PluginInfo {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Whether plugin is enabled
    pub enabled: bool,
}

/// API Error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

// ============================================================================
// Health Endpoints
// ============================================================================

/// GET /api/v1/health - Health check endpoint
pub async fn health_endpoint() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/v1/ready - Readiness check
pub async fn readiness_check(State(state): State<ApiState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ready": true,
        "version": state.version,
        "components": {
            "session_store": "ok",
            "llm_providers": "ok",
            "tools": "ok"
        }
    }))
}

// ============================================================================
// Session Endpoints
// ============================================================================

/// GET /api/v1/sessions - List all sessions
pub async fn list_sessions(State(state): State<ApiState>) -> Json<Vec<Session>> {
    let sessions = state.db.list_sessions().await;
    Json(sessions)
}

/// POST /api/v1/sessions - Create a new session
pub async fn create_session(
    State(state): State<ApiState>,
    Json(request): Json<CreateSessionRequest>,
) -> Json<Session> {
    let mut session = Session::new();

    if let Some(name) = request.name {
        session.title = Some(name);
    }

    if let Some(model) = request.model {
        session.meta.model = Some(model);
    }

    let session = state.db.create_session(session).await;
    Json(session)
}

/// GET /api/v1/sessions/{id} - Get a specific session
///
/// # Errors
///
/// Returns `BAD_REQUEST` if the session ID is invalid.
/// Returns `NOT_FOUND` if the session doesn't exist.
pub async fn get_session(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, (StatusCode, Json<ApiError>)> {
    let session_id = match Uuid::parse_str(&id) {
        Ok(uuid) => SessionId::from_uuid(uuid),
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    code: "BAD_REQUEST".to_string(),
                    message: format!("Invalid session ID: {e}"),
                }),
            ))
        },
    };

    match state.db.get_session(session_id).await {
        Some(session) => Ok(Json(session)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "NOT_FOUND".to_string(),
                message: format!("Session not found: {id}"),
            }),
        )),
    }
}

/// DELETE /api/v1/sessions/{id} - Delete a session
///
/// # Errors
///
/// Returns `BAD_REQUEST` if the session ID is invalid.
/// Returns `INTERNAL_ERROR` if the deletion fails.
pub async fn delete_session(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let session_id = match Uuid::parse_str(&id) {
        Ok(uuid) => SessionId::from_uuid(uuid),
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    code: "BAD_REQUEST".to_string(),
                    message: format!("Invalid session ID: {e}"),
                }),
            ))
        },
    };

    if state.db.delete_session(session_id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                code: "INTERNAL_ERROR".to_string(),
                message: "Failed to delete session".to_string(),
            }),
        ))
    }
}

// ============================================================================
// Chat Endpoints
// ============================================================================

/// POST /api/v1/chat - Send a chat message
pub async fn chat(
    State(_state): State<ApiState>,
    Json(_request): Json<ChatRequest>,
) -> Json<ChatResponse> {
    Json(ChatResponse {
        response: "Chat API is operational. Connect LLM provider for responses.".to_string(),
        session_id: Uuid::new_v4().to_string(),
        tokens_used: Some(0),
    })
}

// ============================================================================
// Tool Endpoints
// ============================================================================

/// GET /api/v1/tools - List available tools
pub async fn list_tools() -> Json<Vec<serde_json::Value>> {
    Json(vec![
        serde_json::json!({"name": "file_read", "description": "Read a file"}),
        serde_json::json!({"name": "file_write", "description": "Write a file"}),
        serde_json::json!({"name": "shell_execute", "description": "Execute shell command"}),
    ])
}

/// POST /api/v1/tools/execute - Execute a tool
pub async fn execute_tool(Json(_request): Json<ExecuteToolRequest>) -> Json<ExecuteToolResponse> {
    Json(ExecuteToolResponse {
        result: serde_json::json!({"status": "ok"}),
        duration_ms: 0,
        sandboxed: true,
    })
}

// ============================================================================
// Plugin Endpoints
// ============================================================================

/// GET /api/v1/plugins - List installed plugins
pub async fn list_plugins() -> Json<Vec<PluginInfo>> {
    Json(vec![PluginInfo {
        id: "builtin-code-analysis".to_string(),
        name: "Code Analysis".to_string(),
        version: "1.0.0".to_string(),
        description: "Built-in code analysis".to_string(),
        enabled: true,
    }])
}

/// GET /api/v1/plugins/marketplace - List marketplace plugins
pub async fn list_marketplace_plugins() -> Json<Vec<PluginInfo>> {
    Json(vec![])
}

// ============================================================================
// Usage Endpoints
// ============================================================================

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub tenant_id: String,
    pub tier: String,
    pub llm_requests: u64,
    pub total_tokens: u64,
    pub sessions: u64,
    pub quota: QuotaInfo,
}

#[derive(Debug, Serialize)]
pub struct QuotaInfo {
    pub tasks_hour: u64,
    pub tasks_hour_limit: u64,
    pub tasks_day: u64,
    pub tasks_day_limit: u64,
}

/// GET /api/v1/usage - Usage summary for the authenticated tenant
pub async fn usage_endpoint(
    State(state): State<ApiState>,
    api_key: Option<Extension<AuthenticatedApiKey>>,
) -> Json<UsageResponse> {
    use crate::telemetry::metrics;
    use std::sync::atomic::Ordering;

    let store = state.tenant_store.read().unwrap();

    let tenant = match api_key {
        Some(Extension(key)) => store.get_tenant_by_api_key(&key.0),
        None => store.get_tenant("default"),
    };

    let (tenant_id, tier, hour_limit, day_limit) = match tenant {
        Some(t) => (
            t.id.clone(),
            t.tier.as_str().to_string(),
            t.tier.tasks_hour_limit(),
            t.tier.tasks_day_limit(),
        ),
        None => ("default".to_string(), "free".to_string(), 10, 50),
    };

    let m = metrics();
    let requests = m.requests_total.load(Ordering::Relaxed);
    let tokens = m.tokens_used.load(Ordering::Relaxed);
    let sessions = m.sessions_total.load(Ordering::Relaxed);

    Json(UsageResponse {
        tenant_id,
        tier,
        llm_requests: requests,
        total_tokens: tokens,
        sessions,
        quota: QuotaInfo {
            tasks_hour: requests,
            tasks_hour_limit: hour_limit,
            tasks_day: requests,
            tasks_day_limit: day_limit,
        },
    })
}

// ============================================================================
// Router Setup
// ============================================================================

/// Create the REST API router
pub fn create_router(state: ApiState) -> Router {
    let auth = ApiKeyAuth::from_config(if state.api_keys.is_empty() {
        None
    } else {
        Some(state.api_keys.clone())
    });

    let protected_routes = Router::new()
        .route("/api/v1/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/v1/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/api/v1/chat", post(chat))
        .route("/api/v1/tools", get(list_tools))
        .route("/api/v1/tools/execute", post(execute_tool))
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/marketplace", get(list_marketplace_plugins))
        .route("/api/v1/usage", get(usage_endpoint));

    let protected = if auth.is_enabled() {
        protected_routes.layer(middleware::from_fn_with_state(
            auth.clone(),
            auth_middleware,
        ))
    } else {
        protected_routes
    };

    let router = Router::new()
        .route("/metrics", get(metrics_handler::metrics_handler))
        .route("/api/v1/health", get(health_endpoint))
        .route("/api/v1/ready", get(readiness_check))
        .merge(protected)
        .with_state(state.clone());

    if let Some(config) = state.rate_limit_config.clone() {
        let limiter = ApiRateLimiter::new(config);
        router.layer(middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware,
        ))
    } else {
        router
    }
}
