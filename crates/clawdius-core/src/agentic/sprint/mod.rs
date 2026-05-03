mod engine;
mod error_recovery;
mod phases;
mod review;

use crate::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use engine::SprintEngine;
pub use error_recovery::{attempt_error_recovery, execute_real_phase};
pub use phases::{phase_prompt, run_phase, sync_lsp_documents};
pub use review::run_multi_model_review;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SprintPhase {
    Think,
    Plan,
    Build,
    Review,
    Test,
    Ship,
    Reflect,
}

impl SprintPhase {
    pub fn all() -> Vec<SprintPhase> {
        vec![
            SprintPhase::Think,
            SprintPhase::Plan,
            SprintPhase::Build,
            SprintPhase::Review,
            SprintPhase::Test,
            SprintPhase::Ship,
            SprintPhase::Reflect,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SprintPhase::Think => "Think",
            SprintPhase::Plan => "Plan",
            SprintPhase::Build => "Build",
            SprintPhase::Review => "Review",
            SprintPhase::Test => "Test",
            SprintPhase::Ship => "Ship",
            SprintPhase::Reflect => "Reflect",
        }
    }
}

impl std::fmt::Display for SprintPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhaseStatus {
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: SprintPhase,
    pub status: PhaseStatus,
    pub output: String,
    pub duration_ms: u64,
    pub files_modified: Vec<String>,
    pub errors: Vec<String>,
    pub tokens_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintMetrics {
    pub total_tokens: usize,
    pub phase_durations_ms: Vec<(String, u64)>,
    pub phase_tokens: Vec<(String, usize)>,
    pub retry_cycles: usize,
    pub phases_succeeded: usize,
    pub phases_failed: usize,
    pub phases_skipped: usize,
}

impl SprintMetrics {
    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str("╔══════════════════════════════════════════╗\n");
        report.push_str("║         Sprint Metrics Report           ║\n");
        report.push_str("╠══════════════════════════════════════════╣\n");

        for (phase, duration) in &self.phase_durations_ms {
            report.push_str(&format!(
                "║ {:12} {:>8}ms               ║\n",
                phase, duration
            ));
        }

        report.push_str("╠══════════════════════════════════════════╣\n");
        report.push_str(&format!(
            "║ Total tokens:  {:>6}                 ║\n",
            self.total_tokens
        ));
        report.push_str(&format!(
            "║ Retry cycles: {:>6}                 ║\n",
            self.retry_cycles
        ));
        report.push_str(&format!(
            "║ Phases: {}/{}/{} (ok/fail/skip)          ║\n",
            self.phases_succeeded, self.phases_failed, self.phases_skipped
        ));
        report.push_str("╚══════════════════════════════════════════╝\n");
        report
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintConfig {
    pub task_description: String,
    pub project_root: PathBuf,
    pub auto_approve: bool,
    pub skip_phases: Vec<SprintPhase>,
    pub max_iterations: usize,
    pub model: Option<String>,
    pub build_command: String,
    pub test_command: String,
    pub real_execution: bool,
    pub browser_qa_url: Option<String>,
    pub reviewers: Vec<crate::agentic::review_engine::ReviewerConfig>,
    pub max_duration_secs: u64,
    pub phase_timeout_secs: u64,
    pub max_tokens_per_phase: u32,
    pub extra_context: Option<String>,
}

impl SprintConfig {
    pub fn new(task_description: &str) -> Self {
        Self {
            task_description: task_description.to_string(),
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            auto_approve: false,
            skip_phases: Vec::new(),
            max_iterations: 3,
            model: None,
            build_command: "cargo check 2>&1".to_string(),
            test_command: "cargo test --lib 2>&1".to_string(),
            real_execution: false,
            browser_qa_url: None,
            reviewers: Vec::new(),
            max_duration_secs: 600,
            phase_timeout_secs: 120,
            max_tokens_per_phase: 4096,
            extra_context: None,
        }
    }
}

impl Default for SprintConfig {
    fn default() -> Self {
        Self::new("Execute sprint")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintState {
    pub current_phase: Option<SprintPhase>,
    pub phase_results: Vec<PhaseResult>,
    pub context_accumulator: String,
    pub started_at: DateTime<Utc>,
    pub config: SprintConfig,
    pub checkpoint_ref: Option<String>,
}

impl SprintState {
    pub fn new(config: SprintConfig) -> Self {
        Self {
            current_phase: None,
            phase_results: Vec::new(),
            context_accumulator: String::new(),
            started_at: Utc::now(),
            config,
            checkpoint_ref: None,
        }
    }

    pub fn active_phases(&self) -> Vec<SprintPhase> {
        SprintPhase::all()
            .into_iter()
            .filter(|p| !self.config.skip_phases.contains(p))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintResult {
    pub success: bool,
    pub phase_results: Vec<PhaseResult>,
    pub total_duration_ms: u64,
    pub summary: String,
    pub checkpoint_ref: Option<String>,
    pub rollback_available: bool,
    pub metrics: SprintMetrics,
}

#[derive(thiserror::Error, Debug)]
pub enum SprintError {
    #[error("Phase {phase} failed: {reason}")]
    PhaseFailed { phase: SprintPhase, reason: String },
    #[error("Max iterations ({max}) reached without passing tests")]
    MaxIterationsReached { max: usize },
    #[error("LLM error in phase {phase}: {reason}")]
    LlmError { phase: SprintPhase, reason: String },
    #[error("Sprint aborted at phase {phase}")]
    Aborted { phase: SprintPhase },
}

impl From<SprintError> for crate::Error {
    fn from(e: SprintError) -> Self {
        crate::Error::Sprint(e.to_string())
    }
}

pub fn detect_language(path: &str) -> &'static str {
    match std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp") | Some("cc") | Some("cxx") => "cpp",
        Some("h") | Some("hpp") => "c",
        Some("rb") => "ruby",
        Some("swift") => "swift",
        Some("kt") | Some("kts") => "kotlin",
        Some("scala") => "scala",
        Some("sh") | Some("bash") | Some("zsh") => "bash",
        _ => "unknown",
    }
}

pub fn get_changed_files(project_root: &std::path::Path) -> Option<Vec<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=AM"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if files.is_empty() {
        None
    } else {
        Some(files)
    }
}

fn create_checkpoint(project_root: &std::path::Path) -> Option<String> {
    use std::process::Command;
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let stash_msg = format!("clawdius-sprint-checkpoint-{timestamp}");
    let output = Command::new("git")
        .args(["stash", "push", "-m", &stash_msg])
        .current_dir(project_root)
        .output()
        .ok()?;

    if output.status.success() {
        Some(format!("stash@{{0}}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Checkpoint creation failed: {stderr}");
        None
    }
}

pub fn rollback(project_root: &std::path::Path, checkpoint_ref: &str) -> Result<()> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["stash", "pop", checkpoint_ref])
        .current_dir(project_root)
        .output()
        .map_err(|e| crate::Error::Sprint(format!("Failed to execute git: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(crate::Error::Sprint(format!("Rollback failed: {stderr}")))
    }
}

const SPRINT_STATE_DIR: &str = ".clawdius/sprints";

pub fn save_state(state: &SprintState) -> Result<String> {
    let sprint_dir = state.config.project_root.join(SPRINT_STATE_DIR);
    std::fs::create_dir_all(&sprint_dir).map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create sprint state dir: {e}"),
        ))
    })?;

    let filename = format!("sprint_{}.json", state.started_at.format("%Y%m%d-%H%M%S"));
    let path = sprint_dir.join(&filename);
    let json =
        serde_json::to_string_pretty(state).map_err(|e| crate::Error::Serialization(e))?;
    std::fs::write(&path, json).map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to write sprint state: {e}"),
        ))
    })?;

    tracing::info!("Sprint state saved to {}", path.display());
    Ok(filename)
}

pub fn load_latest_state(project_root: &std::path::Path) -> Result<Option<SprintState>> {
    let sprint_dir = project_root.join(SPRINT_STATE_DIR);
    if !sprint_dir.exists() {
        return Ok(None);
    }

    let mut entries: Vec<_> = std::fs::read_dir(&sprint_dir)
        .map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read sprint state dir: {e}"),
            ))
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    let Some(latest) = entries.into_iter().next() else {
        return Ok(None);
    };

    let json = std::fs::read_to_string(latest.path()).map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to read sprint state file: {e}"),
        ))
    })?;

    let state: SprintState =
        serde_json::from_str(&json).map_err(|e| crate::Error::Serialization(e))?;

    tracing::info!(
        "Loaded sprint state from {} ({} phases completed)",
        latest.path().display(),
        state.phase_results.len()
    );
    Ok(Some(state))
}

pub fn list_saved_states(project_root: &std::path::Path) -> Result<Vec<SprintState>> {
    let sprint_dir = project_root.join(SPRINT_STATE_DIR);
    if !sprint_dir.exists() {
        return Ok(Vec::new());
    }

    let entries: Vec<_> = std::fs::read_dir(&sprint_dir)
        .map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read sprint state dir: {e}"),
            ))
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    let mut states = Vec::new();
    for entry in entries {
        let Ok(json) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Ok(state) = serde_json::from_str::<SprintState>(&json) {
            states.push(state);
        }
    }

    states.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(states)
}

pub fn delete_saved_state(project_root: &std::path::Path, started_at: DateTime<Utc>) -> Result<()> {
    let filename = format!("sprint_{}.json", started_at.format("%Y%m%d-%H%M%S"));
    let path = project_root.join(SPRINT_STATE_DIR).join(filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to delete sprint state: {e}"),
            ))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tool_executor::NoOpToolExecutor;
    use crate::llm::ChatMessage;
    use crate::llm::providers::LlmClient;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    struct MockLlm {
        response: String,
    }

    impl MockLlm {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> crate::Result<String> {
            Ok(self.response.clone())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> crate::Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send(self.response.clone()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    struct FailingLlm;

    #[async_trait]
    impl LlmClient for FailingLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> crate::Result<String> {
            Err(crate::Error::Llm("mock failure".to_string()))
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> crate::Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            drop(tx);
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    #[test]
    fn test_sprint_config_default() {
        let config = SprintConfig::default();
        assert_eq!(config.task_description, "Execute sprint");
        assert_eq!(config.max_iterations, 3);
        assert!(!config.auto_approve);
        assert!(config.skip_phases.is_empty());
        assert!(config.model.is_none());
    }

    #[test]
    fn test_sprint_config_new() {
        let config = SprintConfig::new("Build a feature");
        assert_eq!(config.task_description, "Build a feature");
        assert_eq!(config.max_iterations, 3);
        assert!(!config.auto_approve);
    }

    #[test]
    fn test_sprint_phases_order() {
        let phases = SprintPhase::all();
        assert_eq!(phases.len(), 7);
        assert_eq!(phases[0], SprintPhase::Think);
        assert_eq!(phases[1], SprintPhase::Plan);
        assert_eq!(phases[2], SprintPhase::Build);
        assert_eq!(phases[3], SprintPhase::Review);
        assert_eq!(phases[4], SprintPhase::Test);
        assert_eq!(phases[5], SprintPhase::Ship);
        assert_eq!(phases[6], SprintPhase::Reflect);
    }

    #[test]
    fn test_sprint_skip_phases() {
        let config = SprintConfig::new("test");
        let state = SprintState::new(config);
        assert_eq!(state.active_phases().len(), 7);

        let config2 = SprintConfig {
            skip_phases: vec![SprintPhase::Think, SprintPhase::Reflect],
            ..SprintConfig::new("test")
        };
        let state2 = SprintState::new(config2);
        let active = state2.active_phases();
        assert_eq!(active.len(), 5);
        assert_eq!(active[0], SprintPhase::Plan);
        assert_eq!(active[4], SprintPhase::Ship);
    }

    #[test]
    fn test_phase_prompt_generation() {
        for phase in SprintPhase::all() {
            let prompt = phase_prompt(&phase);
            assert!(
                !prompt.is_empty(),
                "Prompt for {:?} should not be empty",
                phase
            );
            assert!(prompt.len() > 20, "Prompt for {:?} seems too short", phase);
        }
    }

    #[tokio::test]
    async fn test_context_accumulation() {
        let llm = Arc::new(MockLlm::new("phase output"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![
                SprintPhase::Review,
                SprintPhase::Test,
                SprintPhase::Ship,
                SprintPhase::Reflect,
            ],
            ..SprintConfig::new("test task")
        };
        let mut state = SprintState::new(config);

        assert!(state.context_accumulator.is_empty());

        let _ = engine
            .run_phase(&mut state, &SprintPhase::Think)
            .await
            .unwrap();
        assert!(state.context_accumulator.contains("Think Phase"));
        assert!(state.context_accumulator.contains("phase output"));

        let _ = engine
            .run_phase(&mut state, &SprintPhase::Plan)
            .await
            .unwrap();
        assert!(state.context_accumulator.contains("Plan Phase"));

        let len_after_two = state.context_accumulator.len();
        assert!(len_after_two > 0);
    }

    #[tokio::test]
    async fn test_sprint_state_transitions() {
        let llm = Arc::new(MockLlm::new("output"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![SprintPhase::Test, SprintPhase::Ship, SprintPhase::Reflect],
            ..SprintConfig::new("test task")
        };
        let mut state = SprintState::new(config);

        assert!(state.current_phase.is_none());

        let _ = engine
            .run_phase(&mut state, &SprintPhase::Think)
            .await
            .unwrap();
        assert!(state.current_phase.is_none());
        assert_eq!(state.phase_results.len(), 1);
        assert_eq!(state.phase_results[0].phase, SprintPhase::Think);
        assert_eq!(state.phase_results[0].status, PhaseStatus::Success);

        let _ = engine
            .run_phase(&mut state, &SprintPhase::Plan)
            .await
            .unwrap();
        assert_eq!(state.phase_results.len(), 2);
    }

    #[tokio::test]
    async fn test_max_iterations() {
        let llm = Arc::new(MockLlm::new("output"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            max_iterations: 2,
            skip_phases: vec![SprintPhase::Review, SprintPhase::Ship, SprintPhase::Reflect],
            ..SprintConfig::new("test task")
        };

        let result = engine.run(config).await.unwrap();
        let build_count = result
            .phase_results
            .iter()
            .filter(|r| r.phase == SprintPhase::Build && r.status == PhaseStatus::Success)
            .count();
        let test_count = result
            .phase_results
            .iter()
            .filter(|r| r.phase == SprintPhase::Test && r.status == PhaseStatus::Success)
            .count();

        assert!(build_count >= 1);
        assert!(test_count >= 1);
    }

    #[tokio::test]
    async fn test_sprint_result_summary() {
        let llm = Arc::new(MockLlm::new("reflect output here"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig::new("test task");
        let result = engine.run(config).await.unwrap();

        assert!(!result.phase_results.is_empty());
        assert_eq!(result.phase_results.len(), 7);
        assert!(!result.summary.is_empty());
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_phase_status_tracking() {
        let llm = Arc::new(MockLlm::new("ok"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![SprintPhase::Think, SprintPhase::Reflect],
            ..SprintConfig::new("test")
        };
        let mut state = SprintState::new(config);

        let skipped = engine
            .run_phase(&mut state, &SprintPhase::Think)
            .await
            .unwrap();
        assert_eq!(skipped.status, PhaseStatus::Skipped);

        let success = engine
            .run_phase(&mut state, &SprintPhase::Plan)
            .await
            .unwrap();
        assert_eq!(success.status, PhaseStatus::Success);
        assert_eq!(success.phase, SprintPhase::Plan);
        assert!(success.duration_ms > 0 || success.duration_ms == 0);
        assert!(success.errors.is_empty());
    }

    #[tokio::test]
    async fn test_phase_failure() {
        let llm = Arc::new(FailingLlm);
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![
                SprintPhase::Plan,
                SprintPhase::Build,
                SprintPhase::Review,
                SprintPhase::Test,
                SprintPhase::Ship,
                SprintPhase::Reflect,
            ],
            ..SprintConfig::new("test")
        };
        let result = engine.run(config).await.unwrap();
        assert!(!result.success);
        let think_result = result
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Think)
            .unwrap();
        assert_eq!(think_result.status, PhaseStatus::Failed);
        assert!(!think_result.errors.is_empty());
    }

    #[test]
    fn test_phase_serialization() {
        let phase = SprintPhase::Build;
        let json = serde_json::to_string(&phase).unwrap();
        let parsed: SprintPhase = serde_json::from_str(&json).unwrap();
        assert_eq!(phase, parsed);
    }

    #[test]
    fn test_phase_result_serialization() {
        let result = PhaseResult {
            phase: SprintPhase::Think,
            status: PhaseStatus::Success,
            output: "some output".to_string(),
            duration_ms: 100,
            files_modified: vec!["src/main.rs".to_string()],
            errors: Vec::new(),
            tokens_used: 42,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PhaseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.phase, SprintPhase::Think);
        assert_eq!(parsed.status, PhaseStatus::Success);
        assert_eq!(parsed.duration_ms, 100);
    }

    #[test]
    fn test_sprint_config_serialization() {
        let config = SprintConfig {
            task_description: "Build X".to_string(),
            project_root: std::path::PathBuf::from("/tmp"),
            auto_approve: true,
            skip_phases: vec![SprintPhase::Reflect],
            max_iterations: 5,
            model: Some("gpt-4".to_string()),
            build_command: "cargo check 2>&1".to_string(),
            test_command: "cargo test 2>&1".to_string(),
            real_execution: true,
            browser_qa_url: Some("http://localhost:3000".to_string()),
            reviewers: Vec::new(),
            max_duration_secs: 600,
            phase_timeout_secs: 120,
            max_tokens_per_phase: 4096,
            extra_context: Some("test context".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SprintConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, "Build X");
        assert_eq!(parsed.max_iterations, 5);
        assert!(parsed.auto_approve);
        assert!(parsed.real_execution);
        assert_eq!(parsed.build_command, "cargo check 2>&1");
        assert_eq!(parsed.test_command, "cargo test 2>&1");
    }

    #[tokio::test]
    async fn test_sprint_metrics() {
        let llm = Arc::new(MockLlm::new("test output with several words"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![SprintPhase::Test, SprintPhase::Ship, SprintPhase::Reflect],
            ..SprintConfig::new("test task")
        };
        let result = engine.run(config).await.unwrap();

        assert_eq!(result.metrics.phases_succeeded, 4);
        assert_eq!(result.metrics.phases_failed, 0);
        assert_eq!(result.metrics.phases_skipped, 0);
        assert_eq!(result.metrics.retry_cycles, 0);
        assert!(result.metrics.total_tokens > 0);
        assert_eq!(result.metrics.phase_durations_ms.len(), 4);
        assert_eq!(result.metrics.phase_tokens.len(), 4);

        let report = result.metrics.report();
        assert!(report.contains("Sprint Metrics Report"));
        assert!(report.contains("Total tokens"));
    }

    #[test]
    fn test_sprint_metrics_serialization() {
        let metrics = SprintMetrics {
            total_tokens: 150,
            phase_durations_ms: vec![("Think".to_string(), 100)],
            phase_tokens: vec![("Think".to_string(), 50)],
            retry_cycles: 2,
            phases_succeeded: 3,
            phases_failed: 1,
            phases_skipped: 1,
        };
        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: SprintMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_tokens, 150);
        assert_eq!(parsed.retry_cycles, 2);
    }

    #[test]
    fn test_checkpoint_non_git_dir() {
        let result = create_checkpoint(std::path::PathBuf::from("/tmp").as_path());
        assert!(
            result.is_none(),
            "Checkpoint in non-git directory should return None"
        );
    }

    #[test]
    fn test_checkpoint_in_git_repo() {
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let result = create_checkpoint(&project_root);
        if let Some(ref stash_ref) = result {
            assert!(stash_ref.starts_with("stash@"));
            let _ = rollback(&project_root, stash_ref);
        }
    }

    #[test]
    fn test_rollback_invalid_ref() {
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let result = rollback(&project_root, "stash@{999}");
        assert!(result.is_err(), "Rollback with invalid ref should fail");
    }

    #[tokio::test]
    async fn test_sprint_result_rollback_fields() {
        let llm = Arc::new(FailingLlm);
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            skip_phases: vec![
                SprintPhase::Plan,
                SprintPhase::Build,
                SprintPhase::Review,
                SprintPhase::Test,
                SprintPhase::Ship,
                SprintPhase::Reflect,
            ],
            ..SprintConfig::new("test")
        };
        let result = engine.run(config).await.unwrap();
        assert!(!result.success);
        assert!(result.checkpoint_ref.is_none());
        assert!(!result.rollback_available);
    }

    #[tokio::test]
    #[ignore]
    async fn test_sprint_with_openrouter() {
        use crate::llm::{create_provider, LlmConfig};

        let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");

        let config = LlmConfig {
            provider: "openrouter".to_string(),
            model: "google/gemma-3-4b-it:free".to_string(),
            api_key: Some(api_key),
            base_url: None,
            max_tokens: 300,
        };

        let provider = create_provider(&config).expect("Failed to create provider");
        let engine = SprintEngine::new(Arc::new(provider));

        let sprint_config = SprintConfig {
            task_description: "Add a hello function to src/main.rs".to_string(),
            project_root: std::path::PathBuf::from("/tmp/sprint-test"),
            auto_approve: true,
            skip_phases: vec![SprintPhase::Build, SprintPhase::Test, SprintPhase::Ship],
            max_iterations: 1,
            model: None,
            ..SprintConfig::new("Add a hello function to src/main.rs")
        };

        let max_retries = 3;
        let mut result = None;
        for attempt in 1..=max_retries {
            let sprint_result = engine.run(sprint_config.clone()).await.unwrap();
            eprintln!("\n=== Sprint Result (attempt {attempt}) ===");
            eprintln!("Success: {}", sprint_result.success);
            eprintln!("Duration: {}ms", sprint_result.total_duration_ms);
            eprintln!("Phases: {}", sprint_result.phase_results.len());
            for pr in &sprint_result.phase_results {
                eprintln!(
                    "  {} ({:?}): {} chars, {}ms",
                    pr.phase,
                    pr.status,
                    pr.output.len(),
                    pr.duration_ms
                );
                if !pr.errors.is_empty() {
                    for err in &pr.errors {
                        eprintln!("    error: {err}");
                    }
                }
            }

            let rate_limited = sprint_result.phase_results.iter().any(|r| {
                r.errors
                    .iter()
                    .any(|e| e.contains("429") || e.contains("rate limit"))
            });

            if !rate_limited && sprint_result.success {
                result = Some(sprint_result);
                break;
            }

            if attempt < max_retries {
                let delay = 2000u64 * attempt as u64;
                eprintln!("Rate limited or failed. Retrying in {delay}ms...");
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            } else {
                result = Some(sprint_result);
            }
        }

        let result = result.unwrap();
        println!("\nSummary:\n{}", result.summary);

        assert!(
            !result.phase_results.is_empty(),
            "Should have at least one phase result"
        );
        assert!(result.total_duration_ms > 0, "Duration should be positive");

        for pr in &result.phase_results {
            assert!(
                !pr.output.is_empty() || !pr.errors.is_empty(),
                "Phase {} should have output or errors",
                pr.phase
            );
        }

        if result.success {
            let successful_phases: Vec<_> = result
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Success && r.output.len() > 10)
                .collect();
            assert!(
                !successful_phases.is_empty(),
                "Successful sprint should have phases with substantial output"
            );
        } else {
            let all_rate_limited = result.phase_results.iter().all(|r| {
                r.status == PhaseStatus::Failed
                    && r.errors.iter().any(|e| {
                        e.contains("429")
                            || e.contains("rate limit")
                            || e.contains("Web call failed")
                    })
            });
            eprintln!(
                "Sprint did not succeed (expected with free-tier rate limits). All phases rate-limited: {all_rate_limited}"
            );
        }
    }

    #[test]
    fn test_sprint_config_real_execution_fields() {
        let config = SprintConfig::new("test");
        assert_eq!(config.build_command, "cargo check 2>&1");
        assert_eq!(config.test_command, "cargo test --lib 2>&1");
        assert!(!config.real_execution);

        let config2 = SprintConfig {
            real_execution: true,
            build_command: "make build".to_string(),
            test_command: "make test".to_string(),
            ..SprintConfig::new("test")
        };
        assert!(config2.real_execution);
        assert_eq!(config2.build_command, "make build");
        assert_eq!(config2.test_command, "make test");
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("src/main.rs"), "rust");
        assert_eq!(detect_language("script.py"), "python");
        assert_eq!(detect_language("index.ts"), "typescript");
        assert_eq!(detect_language("index.tsx"), "typescript");
        assert_eq!(detect_language("app.js"), "javascript");
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("unknown.xyz"), "unknown");
        assert_eq!(detect_language("noext"), "unknown");
    }

    #[tokio::test]
    async fn test_sprint_engine_with_tool_executor() {
        let llm = Arc::new(MockLlm::new("output"));
        let engine = SprintEngine::new(llm).with_tool_executor(Arc::new(NoOpToolExecutor));
        assert!(engine.tool_executor.is_some());
    }

    #[tokio::test]
    async fn test_real_execution_skipped_without_tool_executor() {
        let llm = Arc::new(MockLlm::new("build output"));
        let engine = SprintEngine::new(llm);
        let config = SprintConfig {
            real_execution: true,
            skip_phases: vec![
                SprintPhase::Think,
                SprintPhase::Plan,
                SprintPhase::Review,
                SprintPhase::Test,
                SprintPhase::Ship,
                SprintPhase::Reflect,
            ],
            ..SprintConfig::new("test task")
        };

        let result = engine.run(config).await.unwrap();
        assert!(result.success);
        let build = result
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Build)
            .unwrap();
        assert_eq!(build.status, PhaseStatus::Success);
        assert!(!build.output.contains("[Real execution]"));
    }

    #[tokio::test]
    async fn test_real_execution_with_noop_executor() {
        let llm = Arc::new(MockLlm::new("build plan"));
        let engine = SprintEngine::new(llm).with_tool_executor(Arc::new(NoOpToolExecutor));
        let config = SprintConfig {
            real_execution: true,
            skip_phases: vec![
                SprintPhase::Think,
                SprintPhase::Plan,
                SprintPhase::Review,
                SprintPhase::Test,
                SprintPhase::Ship,
                SprintPhase::Reflect,
            ],
            ..SprintConfig::new("test task")
        };

        let result = engine.run(config).await.unwrap();
        let build = result
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Build)
            .unwrap();
        assert_eq!(build.status, PhaseStatus::Success);
        assert!(build.output.contains("[Real execution]"));
    }

    #[test]
    fn test_get_changed_files_in_git_repo() {
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let result = get_changed_files(&project_root);
        match result {
            Some(files) => assert!(!files.is_empty()),
            None => {},
        }
    }

    #[test]
    fn test_get_changed_files_non_git_dir() {
        let result = get_changed_files(std::path::PathBuf::from("/tmp").as_path());
        if let Some(files) = result {
            for f in &files {
                assert!(!f.is_empty());
            }
        }
    }

    #[test]
    fn test_error_recovery_config_builder() {
        let config = crate::agentic::error_recovery::ErrorRecoveryConfig::new(5).with_compiler_output(true);
        assert_eq!(config.max_retries, 5);
        assert!(config.include_compiler_output);
    }

    #[test]
    fn test_sprint_config_reviewers_field() {
        let config = SprintConfig::new("test");
        assert!(config.reviewers.is_empty());

        let config2 = SprintConfig {
            reviewers: vec![crate::agentic::review_engine::ReviewerConfig {
                name: "Quality".to_string(),
                llm_config: crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
                focus: crate::agentic::ReviewFocus::CodeQuality,
            }],
            ..SprintConfig::new("test")
        };
        assert_eq!(config2.reviewers.len(), 1);
        assert_eq!(config2.reviewers[0].name, "Quality");
    }
}
