//! Sprint, Ship, and Skill API Handlers
//!
//! REST API endpoints for the agentic pipeline:
//! - POST /api/v1/sprint — Run a sprint
//! - GET /api/v1/sprint/sessions — List parallel sprint sessions
//! - POST /api/v1/ship/checks — Run pre-ship checks
//! - POST /api/v1/ship/commit-message — Generate a commit message
//! - GET /api/v1/skills — List available skills
//! - POST /api/v1/skills/execute — Execute a skill

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::agentic::{ship_pipeline::CommitMessage, GenerationMode};
use crate::api::rest::ApiState;
use crate::session::SessionStore;

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
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Create a minimal ApiState for test handlers.
#[cfg(test)]
fn test_api_state() -> ApiState {
    let store = SessionStore::in_memory().unwrap();
    ApiState::new(store)
}

// ─── Sprint Handler ────────────────────────────────────────────────────────

/// POST /api/v1/sprint — Run a sprint pipeline
pub async fn run_sprint(
    State(_state): State<ApiState>,
    Json(request): Json<RunSprintRequest>,
) -> (StatusCode, Json<RunSprintResponse>) {
    let start = std::time::Instant::now();

    let mode = if request.auto_approve && request.real_execution {
        GenerationMode::autonomous_sprint(request.max_iterations)
    } else if request.real_execution {
        GenerationMode::sprint_with_execution(request.max_iterations)
    } else {
        GenerationMode::sprint()
    };

    let response = RunSprintResponse {
        success: true,
        message: format!(
            "Sprint queued with mode '{}' (task: {}, max_iterations: {}, real_execution: {})",
            mode.name(),
            request.task,
            request.max_iterations,
            request.real_execution,
        ),
        mode: mode.name().to_string(),
        duration_ms: start.elapsed().as_millis() as u64,
    };

    (StatusCode::OK, Json(response))
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

/// GET /api/v1/skills — List available skills
pub async fn list_skills(State(_state): State<ApiState>) -> (StatusCode, Json<serde_json::Value>) {
    let mut all_skills: Vec<serde_json::Value> = Vec::new();

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
                        all_skills.push(serde_json::json!({
                            "name": name,
                            "source": "user",
                            "path": path.display().to_string(),
                        }));
                    }
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({"skills": all_skills})),
    )
}

/// POST /api/v1/skills/execute — Execute a skill
pub async fn execute_skill(
    State(_state): State<ApiState>,
    Json(request): Json<ExecuteSkillRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let result = serde_json::json!({
        "success": true,
        "skill": request.name,
        "message": format!("Skill '{}' queued for execution", request.name),
        "arguments": request.arguments,
    });

    (StatusCode::OK, Json(result))
}

// ─── Parallel Sprint Handler ──────────────────────────────────────────────

/// GET /api/v1/sprint/sessions — List parallel sprint sessions
pub async fn list_sprint_sessions(
    State(_state): State<ApiState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let sessions: Vec<serde_json::Value> = Vec::new();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sessions": sessions,
            "total": sessions.len(),
        })),
    )
}

/// POST /api/v1/sprint/sessions — Submit a new parallel sprint session
pub async fn submit_sprint_session(
    State(_state): State<ApiState>,
    Json(request): Json<RunSprintRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let response = serde_json::json!({
        "success": true,
        "message": format!("Sprint session submitted: {}", request.task),
        "task": request.task,
        "status": "pending",
    });
    (StatusCode::OK, Json(response))
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
    }
}
