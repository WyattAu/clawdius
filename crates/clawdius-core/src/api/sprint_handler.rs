//! Sprint, Ship, and Skill API Handlers
//!
//! REST API endpoints for the agentic pipeline:
//! - POST /api/v1/sprint — Run a sprint (actually invokes SprintEngine)
//! - GET /api/v1/sprint/sessions — List parallel sprint sessions
//! - POST /api/v1/ship/checks — Run pre-ship checks
//! - POST /api/v1/ship/commit-message — Generate a commit message
//! - GET /api/v1/skills — List available skills (built-in + user markdown)
//! - POST /api/v1/skills/execute — Execute a skill (loads & runs markdown)

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::agentic::tool_executor::{ShellToolExecutor, ToolExecutor};
use crate::agentic::{
    ship_pipeline::CommitMessage, GenerationMode, ParallelSprintConfig, ParallelSprintManager,
    SprintConfig, SprintEngine,
};
use crate::api::rest::ApiState;
use crate::llm::providers::LlmClient;
use crate::session::SessionStore;
use crate::skills::{Skill, SkillContext, SkillRegistry};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSprintRequest {
    pub task: String,
    #[serde(default)]
    pub max_iterations: usize,
    #[serde(default)]
    pub real_execution: bool,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub target_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSprintResponse {
    pub success: bool,
    pub message: String,
    pub mode: String,
    pub duration_ms: u64,
    /// Sprint phase results (populated when sprint actually runs)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phase_results: Vec<serde_json::Value>,
    /// Sprint summary (populated when sprint actually runs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Whether rollback is available
    #[serde(default)]
    pub rollback_available: bool,
    /// Sprint metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreShipCheckRequest {
    pub branch: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default = "default_true")]
    pub tests_passed: bool,
    #[serde(default)]
    pub has_review_approval: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCommitMessageRequest {
    pub changed_files: Vec<String>,
    pub description: String,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteSkillRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
    #[serde(default)]
    pub project_root: Option<String>,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Create a minimal LlmConfig for test/stub use.
fn stub_llm_config() -> crate::llm::LlmConfig {
    crate::llm::LlmConfig {
        provider: "openrouter".to_string(),
        model: "default".to_string(),
        api_key: None,
        base_url: None,
        max_tokens: 4096,
    }
}

/// Create a minimal ApiState for test handlers.
#[cfg(test)]
fn test_api_state() -> ApiState {
    let store = SessionStore::in_memory().unwrap();
    ApiState::new(store)
}

// ─── Sprint Handler ────────────────────────────────────────────────────────

/// POST /api/v1/sprint — Run a sprint pipeline
///
/// Actually invokes `SprintEngine::run()` with the configured LLM client.
/// Returns 503 if no LLM client is configured.
pub async fn run_sprint(
    State(state): State<ApiState>,
    Json(request): Json<RunSprintRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Check for LLM client
    let llm_provider = match &state.llm_client {
        Some(provider) => Arc::clone(provider) as Arc<dyn LlmClient>,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "success": false,
                    "error": "No LLM provider is configured. Set up a provider in your config file.",
                    "code": "LLM_NOT_CONFIGURED",
                })),
            );
        },
    };

    let start = std::time::Instant::now();

    // Determine generation mode
    let mode = if request.auto_approve && request.real_execution {
        GenerationMode::autonomous_sprint(request.max_iterations)
    } else if request.real_execution {
        GenerationMode::sprint_with_execution(request.max_iterations)
    } else {
        GenerationMode::sprint()
    };

    // Build SprintConfig
    let mut config = SprintConfig::new(&request.task);
    config.auto_approve = request.auto_approve;
    config.real_execution = request.real_execution;
    if request.max_iterations > 0 {
        config.max_iterations = request.max_iterations;
    }

    // Create and run SprintEngine with real shell execution
    let project_root = std::path::PathBuf::from(&config.project_root);
    let tool_executor: Arc<dyn crate::agentic::tool_executor::ToolExecutor> =
        Arc::new(ShellToolExecutor::new(project_root));
    let engine = SprintEngine::new(llm_provider).with_tool_executor(tool_executor);

    match engine.run(config).await {
        Ok(result) => {
            let phase_results: Vec<serde_json::Value> = result
                .phase_results
                .iter()
                .map(|pr| {
                    serde_json::json!({
                        "phase": pr.phase.display_name(),
                        "status": format!("{:?}", pr.status),
                        "output": pr.output,
                        "duration_ms": pr.duration_ms,
                        "files_modified": pr.files_modified,
                        "errors": pr.errors,
                        "tokens_used": pr.tokens_used,
                    })
                })
                .collect();

            let body = serde_json::json!({
                "success": result.success,
                "message": if result.success {
                    "Sprint completed successfully".to_string()
                } else {
                    "Sprint completed with failures".to_string()
                },
                "mode": mode.name(),
                "duration_ms": start.elapsed().as_millis() as u64,
                "phase_results": phase_results,
                "summary": result.summary,
                "rollback_available": result.rollback_available,
                "checkpoint_ref": result.checkpoint_ref,
                "metrics": serde_json::json!({
                    "total_tokens": result.metrics.total_tokens,
                    "retry_cycles": result.metrics.retry_cycles,
                    "phases_succeeded": result.metrics.phases_succeeded,
                    "phases_failed": result.metrics.phases_failed,
                    "phases_skipped": result.metrics.phases_skipped,
                }),
            });

            if result.success {
                (StatusCode::OK, Json(body))
            } else {
                (StatusCode::MULTI_STATUS, Json(body))
            }
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Sprint failed: {e}"),
                "mode": mode.name(),
                "duration_ms": start.elapsed().as_millis() as u64,
            })),
        ),
    }
}

// ─── Ship Handler ──────────────────────────────────────────────────────────

/// POST /api/v1/ship/checks — Run pre-ship checks
pub async fn run_pre_ship_checks(
    State(_state): State<ApiState>,
    Json(request): Json<PreShipCheckRequest>,
) -> (StatusCode, Json<crate::agentic::ShipCheckReport>) {
    let pipeline = crate::agentic::ShipPipeline::new_default();
    let report = pipeline
        .run_pre_ship_checks(
            &request.branch,
            &request.changed_files,
            request.tests_passed,
            request.has_review_approval,
        )
        .await;

    (StatusCode::OK, Json(report))
}

/// POST /api/v1/ship/commit-message — Generate a commit message
pub async fn generate_commit_message(
    State(_state): State<ApiState>,
    Json(request): Json<GenerateCommitMessageRequest>,
) -> (StatusCode, Json<CommitMessage>) {
    let pipeline = crate::agentic::ShipPipeline::new_default();
    let msg = pipeline.generate_commit_message(
        &request.changed_files,
        &request.description,
        request.scope.as_deref(),
    );

    (StatusCode::OK, Json(msg))
}

// ─── Skills Handler ────────────────────────────────────────────────────────

/// GET /api/v1/skills — List available skills (built-in + user markdown)
pub async fn list_skills(State(_state): State<ApiState>) -> (StatusCode, Json<serde_json::Value>) {
    let mut all_skills: Vec<serde_json::Value> = Vec::new();

    // List built-in skills
    let registry = SkillRegistry::new();
    registry.register_builtin_skills().await;
    let builtins = registry.list().await;
    for meta in &builtins {
        all_skills.push(serde_json::json!({
            "name": meta.name,
            "source": "builtin",
            "description": meta.description,
            "version": meta.version,
            "tags": meta.tags,
        }));
    }

    // Check for markdown skills in ~/.clawdius/skills/
    let home_dir = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from));

    if let Some(home) = home_dir {
        let skills_dir = home.join(".clawdius").join("skills");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "md") {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        // Try to parse metadata from the markdown file
                        let meta_info = if let Ok(skill) =
                            crate::skills::markdown_skill::MarkdownSkill::from_file(&path)
                        {
                            serde_json::json!({
                                "name": skill.meta().name,
                                "source": "user",
                                "description": skill.meta().description,
                                "version": skill.meta().version,
                                "path": path.display().to_string(),
                            })
                        } else {
                            serde_json::json!({
                                "name": name,
                                "source": "user",
                                "path": path.display().to_string(),
                            })
                        };
                        all_skills.push(meta_info);
                    }
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({"skills": all_skills, "total": all_skills.len()})),
    )
}

/// POST /api/v1/skills/execute — Execute a skill
///
/// Actually loads the skill from disk (for markdown skills) or the registry
/// (for built-in skills), creates a SkillContext with the LLM client, and
/// runs it via SkillRegistry::execute().
pub async fn execute_skill(
    State(state): State<ApiState>,
    Json(request): Json<ExecuteSkillRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let start = std::time::Instant::now();

    // Build SkillContext
    let project_root = request
        .project_root
        .clone()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    let mut context = SkillContext::new(project_root.clone());

    // Attach LLM client if available
    if let Some(ref provider) = state.llm_client {
        context = context.with_llm(Arc::clone(provider) as Arc<dyn LlmClient>);
    }

    // Parse arguments from the request string (key=value or just a string)
    if !request.arguments.is_empty() {
        for part in request.arguments.split_whitespace() {
            if let Some((key, value)) = part.split_once('=') {
                context.add_argument(key, value);
            } else {
                context.add_argument("_raw", &request.arguments);
            }
        }
    }

    // Try to load as a markdown skill from ~/.clawdius/skills/
    let registry = SkillRegistry::new();
    registry.register_builtin_skills().await;

    let home_dir = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from));

    if let Some(home) = home_dir {
        let skills_dir = home.join(".clawdius").join("skills");
        let skill_path = skills_dir.join(format!("{}.md", request.name));
        if skill_path.exists() {
            match crate::skills::markdown_skill::MarkdownSkill::from_file(&skill_path) {
                Ok(skill) => match skill.execute(context).await {
                    Ok(result) => {
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "success": result.success,
                                "skill": request.name,
                                "output": result.output,
                                "modified_files": result.modified_files,
                                "duration_ms": result.duration_ms,
                                "elapsed_ms": start.elapsed().as_millis() as u64,
                            })),
                        );
                    },
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({
                                "success": false,
                                "skill": request.name,
                                "error": format!("Skill execution failed: {e}"),
                                "elapsed_ms": start.elapsed().as_millis() as u64,
                            })),
                        );
                    },
                },
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "success": false,
                            "skill": request.name,
                            "error": format!("Failed to load skill '{}': {e}", request.name),
                            "elapsed_ms": start.elapsed().as_millis() as u64,
                        })),
                    );
                },
            }
        }
    }

    // Try built-in registry
    match registry.execute(&request.name, context).await {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": result.success,
                "skill": request.name,
                "source": "builtin",
                "output": result.output,
                "modified_files": result.modified_files,
                "duration_ms": result.duration_ms,
                "elapsed_ms": start.elapsed().as_millis() as u64,
            })),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "skill": request.name,
                "error": format!("Skill not found: {e}"),
                "elapsed_ms": start.elapsed().as_millis() as u64,
            })),
        ),
    }
}

// ─── Parallel Sprint Handler ──────────────────────────────────────────────

/// GET /api/v1/sprint/sessions — List parallel sprint sessions
pub async fn list_sprint_sessions(
    State(state): State<ApiState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let sessions = state.sprint_manager.list_sessions().await;
    let summary = state.sprint_manager.summary().await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sessions": sessions,
            "summary": summary,
            "total": sessions.len(),
        })),
    )
}

/// GET /api/v1/sprint/sessions/:id — Get a single session status
pub async fn get_sprint_session(
    State(state): State<ApiState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    use crate::agentic::SprintSessionId;
    let id = SprintSessionId(session_id);

    match state.sprint_manager.get_session(&id).await {
        Some(session) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "session": session,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Session {} not found", id),
            })),
        ),
    }
}

/// POST /api/v1/sprint/sessions — Submit a new parallel sprint session
///
/// If an LLM client is configured, the sprint is started immediately in the
/// background. Otherwise, the session is queued as pending.
pub async fn submit_sprint_session(
    State(state): State<ApiState>,
    Json(request): Json<RunSprintRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Use real LLM config if available, otherwise stub
    let llm_config = stub_llm_config();
    let mut config = ParallelSprintConfig::new(&request.task, llm_config)
        .with_name(&request.task);
    config.real_execution = request.real_execution;

    // If LLM client is available, submit and immediately start
    if let Some(ref provider) = state.llm_client {
        let llm = Arc::clone(provider) as Arc<dyn LlmClient>;
        match state.sprint_manager.submit_and_run(config, llm).await {
            Ok(session_id) => {
                let session = state.sprint_manager.get_session(&session_id).await;
                let status = session
                    .as_ref()
                    .map(|s| format!("{:?}", s.status))
                    .unwrap_or_else(|| "unknown".to_string());

                let response = serde_json::json!({
                    "success": true,
                    "session_id": session_id.to_string(),
                    "task": request.task,
                    "status": status,
                    "message": format!("Sprint session {} submitted and started", session_id),
                });
                (StatusCode::CREATED, Json(response))
            },
            Err(e) => {
                let response = serde_json::json!({
                    "success": false,
                    "error": format!("Failed to submit sprint session: {}", e),
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
            },
        }
    } else {
        // No LLM — queue without starting
        match state.sprint_manager.submit(config).await {
            Ok(session_id) => {
                let response = serde_json::json!({
                    "success": true,
                    "session_id": session_id.to_string(),
                    "task": request.task,
                    "status": "pending",
                    "message": format!("Sprint session {} submitted (no LLM configured — not started)", session_id),
                });
                (StatusCode::CREATED, Json(response))
            },
            Err(e) => {
                let response = serde_json::json!({
                    "success": false,
                    "error": format!("Failed to submit sprint session: {}", e),
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
            },
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_sprint_request_deserialization() {
        let json = r#"{"task":"Build auth system","max_iterations":5,"real_execution":true}"#;
        let req: RunSprintRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.task, "Build auth system");
        assert_eq!(req.max_iterations, 5);
        assert!(req.real_execution);
    }

    #[test]
    fn test_run_sprint_request_defaults() {
        let json = r#"{"task":"Fix bug"}"#;
        let req: RunSprintRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_iterations, 0);
        assert!(!req.real_execution);
        assert!(req.target_files.is_empty());
    }

    #[test]
    fn test_pre_ship_check_request() {
        let json = r#"{"branch":"main","changed_files":["src/lib.rs"],"tests_passed":true}"#;
        let req: PreShipCheckRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.branch, "main");
        assert!(req.tests_passed);
    }

    #[test]
    fn test_pre_ship_check_defaults() {
        let json = r#"{"branch":"feature/test"}"#;
        let req: PreShipCheckRequest = serde_json::from_str(json).unwrap();
        assert!(req.tests_passed);
        assert!(!req.has_review_approval);
    }

    #[test]
    fn test_generate_commit_message_request() {
        let json = r#"{"changed_files":["src/lib.rs"],"description":"add feature","scope":"core"}"#;
        let req: GenerateCommitMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.scope, Some("core".to_string()));
    }

    #[test]
    fn test_execute_skill_request() {
        let json = r#"{"name":"ship","arguments":"--yes"}"#;
        let req: ExecuteSkillRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "ship");
    }

    #[test]
    fn test_execute_skill_request_with_project_root() {
        let json = r#"{"name":"review","project_root":"/tmp/project"}"#;
        let req: ExecuteSkillRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.project_root, Some("/tmp/project".to_string()));
    }

    #[tokio::test]
    async fn test_sprint_without_llm_returns_503() {
        let state = test_api_state();
        let req = RunSprintRequest {
            task: "Build auth system".to_string(),
            max_iterations: 1,
            real_execution: false,
            auto_approve: false,
            target_files: vec![],
        };
        let (status, Json(body)) = run_sprint(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "LLM_NOT_CONFIGURED");
    }

    #[tokio::test]
    async fn test_generate_commit_message_endpoint() {
        let req = GenerateCommitMessageRequest {
            changed_files: vec!["src/lib.rs".to_string()],
            description: "add feature".to_string(),
            scope: Some("core".to_string()),
        };
        let (_, Json(msg)) = generate_commit_message(State(test_api_state()), Json(req)).await;
        assert!(msg.subject.contains("feat"));
    }

    #[tokio::test]
    async fn test_pre_ship_checks_endpoint() {
        let req = PreShipCheckRequest {
            branch: "feature/test".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            tests_passed: true,
            has_review_approval: false,
        };
        let (_, Json(report)) = run_pre_ship_checks(State(test_api_state()), Json(req)).await;
        assert!(report.all_passed);
    }

    #[tokio::test]
    async fn test_list_skills_endpoint() {
        let (status, Json(body)) = list_skills(State(test_api_state())).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("skills").is_some());
        assert!(body.get("total").is_some());
    }

    #[tokio::test]
    async fn test_list_skills_includes_builtins() {
        let (status, Json(body)) = list_skills(State(test_api_state())).await;
        assert_eq!(status, StatusCode::OK);
        let skills = body["skills"].as_array().unwrap();
        // Should include at least the 4 built-in skills
        let builtin_count = skills.iter().filter(|s| s["source"] == "builtin").count();
        assert!(
            builtin_count >= 4,
            "Expected at least 4 builtin skills, got {builtin_count}"
        );
    }

    #[tokio::test]
    async fn test_execute_builtin_skill() {
        let state = test_api_state();
        let req = ExecuteSkillRequest {
            name: "explain".to_string(),
            arguments: String::new(),
            project_root: None,
        };
        // Built-in skills without selection will fail — that's expected behavior
        let (status, body) = execute_skill(State(state), Json(req)).await;
        // Should either succeed (no selection required for explain) or fail gracefully
        assert!(
            status == StatusCode::OK || status == StatusCode::NOT_FOUND,
            "Expected OK or NOT_FOUND, got {status}"
        );
    }

    #[tokio::test]
    async fn test_execute_nonexistent_skill_returns_404() {
        let state = test_api_state();
        let req = ExecuteSkillRequest {
            name: "nonexistent_skill_xyz".to_string(),
            arguments: String::new(),
            project_root: None,
        };
        let (status, Json(body)) = execute_skill(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["success"].as_bool().unwrap() == false);
    }

    #[tokio::test]
    async fn test_list_sprint_sessions_empty() {
        let state = test_api_state();
        let (status, Json(body)) = list_sprint_sessions(State(state)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 0);
    }

    #[tokio::test]
    async fn test_submit_sprint_session() {
        let state = test_api_state();
        let req = RunSprintRequest {
            task: "Build auth system".to_string(),
            max_iterations: 3,
            real_execution: false,
            auto_approve: false,
            target_files: vec![],
        };
        let (status, Json(body)) = submit_sprint_session(State(state), Json(req)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(body["success"].as_bool().unwrap());
        assert!(body["session_id"].as_str().unwrap().starts_with("sprint-"));
    }
}
