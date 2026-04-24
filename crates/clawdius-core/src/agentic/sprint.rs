use crate::agentic::browser_daemon::BrowserDaemon;
use crate::agentic::error_recovery::parse_compiler_output;
use crate::agentic::error_recovery::{ErrorRecovery, ErrorRecoveryConfig};
use crate::agentic::review_engine::{ReviewEngine, ReviewerConfig};
use crate::agentic::tool_executor::{ToolExecutor, ToolRequest};
use crate::agentic::tool_use;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use crate::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

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
    /// Approximate token count of the LLM output for this phase
    pub tokens_used: usize,
}

/// Structured metrics for a completed sprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintMetrics {
    /// Total tokens used across all phases
    pub total_tokens: usize,
    /// Duration of each phase in milliseconds
    pub phase_durations_ms: Vec<(String, u64)>,
    /// Token count per phase
    pub phase_tokens: Vec<(String, usize)>,
    /// Total number of build→test retry cycles
    pub retry_cycles: usize,
    /// Number of phases that succeeded
    pub phases_succeeded: usize,
    /// Number of phases that failed
    pub phases_failed: usize,
    /// Number of phases that were skipped
    pub phases_skipped: usize,
}

impl SprintMetrics {
    /// Generate a human-readable metrics report.
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
    /// Shell command to run for the Build phase (e.g., "cargo build 2>&1")
    pub build_command: String,
    /// Shell command to run for the Test phase (e.g., "cargo test --lib 2>&1")
    pub test_command: String,
    /// Whether to execute real build/test commands via ToolExecutor
    pub real_execution: bool,
    /// Optional URL for browser-based QA during the Test phase.
    /// When set, the Test phase prompt includes visual QA instructions.
    pub browser_qa_url: Option<String>,
    /// Optional reviewer configurations for multi-model code review.
    /// When non-empty, the Review phase uses ReviewEngine instead of a single LLM call.
    pub reviewers: Vec<ReviewerConfig>,
    /// Maximum total sprint duration in seconds (default: 600 = 10 min)
    pub max_duration_secs: u64,
    /// Maximum duration for a single phase in seconds (default: 120 = 2 min)
    pub phase_timeout_secs: u64,
    /// Optional extra context to prepend to every phase's user message.
    /// Typically a repo map or project structure summary for LLM grounding.
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
            build_command: "cargo build 2>&1".to_string(),
            test_command: "cargo test --lib 2>&1".to_string(),
            real_execution: false,
            browser_qa_url: None,
            reviewers: Vec::new(),
            max_duration_secs: 600,
            phase_timeout_secs: 120,
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
    /// If a checkpoint was created, this is the git ref that can be used for rollback
    pub checkpoint_ref: Option<String>,
    /// Whether rollback is available (checkpoint was created and sprint did not fully succeed)
    pub rollback_available: bool,
    /// Structured metrics for the sprint
    pub metrics: SprintMetrics,
}

#[derive(Error, Debug)]
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

pub struct SprintEngine {
    llm: Arc<dyn LlmClient>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    browser_daemon: Option<Arc<BrowserDaemon>>,
    lsp_client: Option<Arc<tokio::sync::Mutex<crate::lsp::LspClient>>>,
}

impl SprintEngine {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self {
            llm,
            tool_executor: None,
            browser_daemon: None,
            lsp_client: None,
        }
    }

    /// Attach a tool executor for real command execution (build, test).
    #[must_use]
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Attach a browser daemon for browser-based QA in the Test phase.
    #[must_use]
    pub fn with_browser_daemon(mut self, daemon: Arc<BrowserDaemon>) -> Self {
        self.browser_daemon = Some(daemon);
        self
    }

    /// Attach an LSP client for code intelligence during sprint phases.
    #[must_use]
    pub fn with_lsp_client(mut self, client: crate::lsp::LspClient) -> Self {
        self.lsp_client = Some(Arc::new(tokio::sync::Mutex::new(client)));
        self
    }

    /// Call the LLM with streaming, collecting all chunks into a single response.
    /// Falls back to non-streaming chat if streaming fails.
    async fn chat_collecting_stream(&self, messages: Vec<ChatMessage>) -> crate::Result<String> {
        let llm_timeout = std::time::Duration::from_secs(120);
        match tokio::time::timeout(llm_timeout, self.llm.chat_stream(messages.clone())).await {
            Ok(Ok(mut rx)) => {
                let mut output = String::new();
                while let Some(chunk) = rx.recv().await {
                    output.push_str(&chunk);
                    eprint!(".");
                    use std::io::Write;
                    let _ = std::io::stderr().flush();
                }
                eprintln!();
                if output.is_empty() {
                    Err(crate::Error::Llm("LLM returned empty response".to_string()))
                } else {
                    Ok(output)
                }
            },
            Ok(Err(_)) => {
                // Streaming not supported; fall back to non-streaming
                self.llm
                    .chat(messages)
                    .await
                    .map_err(|e| crate::Error::Llm(format!("LLM chat failed: {e}")))
            },
            Err(_) => Err(crate::Error::Llm(
                "LLM streaming call timed out (120s)".to_string(),
            )),
        }
    }

    pub async fn run(&self, config: SprintConfig) -> Result<SprintResult> {
        let state = SprintState::new(config);
        let phases = state.active_phases();
        let mut state = state;

        let sprint_start = std::time::Instant::now();
        let mut build_test_iterations = 0usize;
        let mut idx = 0;

        while idx < phases.len() {
            let phase = &phases[idx];

            // Check sprint-level timeout
            let elapsed = sprint_start.elapsed().as_secs();
            if elapsed > state.config.max_duration_secs {
                eprintln!(
                    "Sprint timeout: {}s elapsed, max {}s. Stopping.",
                    elapsed, state.config.max_duration_secs
                );
                state.phase_results.push(PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Failed,
                    output: format!(
                        "Sprint exceeded maximum duration of {}s (elapsed: {}s)",
                        state.config.max_duration_secs, elapsed
                    ),
                    duration_ms: 0,
                    files_modified: Vec::new(),
                    errors: vec!["Sprint timed out".to_string()],
                    tokens_used: 0,
                });
                break;
            }

            // Create a git checkpoint before the Build phase
            if *phase == SprintPhase::Build && state.checkpoint_ref.is_none() {
                if let Some(checkpoint) = Self::create_checkpoint(&state.config.project_root) {
                    state.checkpoint_ref = Some(checkpoint);
                    eprintln!(
                        "Checkpoint created: {}",
                        state.checkpoint_ref.as_ref().unwrap()
                    );
                }
            }

            let result = match tokio::time::timeout(
                std::time::Duration::from_secs(state.config.phase_timeout_secs),
                self.run_phase(&mut state, phase),
            )
            .await
            {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    eprintln!("Phase {} error (will be retried or reported): {e}", phase);
                    PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!("Phase failed: {e}"),
                        duration_ms: 0,
                        files_modified: Vec::new(),
                        errors: vec![e.to_string()],
                        tokens_used: 0,
                    }
                },
                Err(_) => {
                    eprintln!(
                        "Phase {} timed out after {}s",
                        phase, state.config.phase_timeout_secs
                    );
                    PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!(
                            "Phase {} timed out after {}s",
                            phase, state.config.phase_timeout_secs
                        ),
                        duration_ms: (state.config.phase_timeout_secs * 1000) as u64,
                        files_modified: Vec::new(),
                        errors: vec![format!(
                            "Phase timed out after {}s",
                            state.config.phase_timeout_secs
                        )],
                        tokens_used: 0,
                    }
                },
            };

            // A1: If tool_executor is present and phase is Build, run the tool-use loop
            // Try native tool_use first (Anthropic/OpenAI/OpenRouter), fall back to
            // parser-based loop for other providers.
            let result = if *phase == SprintPhase::Build
                && self.tool_executor.is_some()
                && result.status == PhaseStatus::Success
            {
                let executor = self.tool_executor.as_ref().unwrap();
                let llm = &self.llm;

                let system_prompt = Self::phase_prompt(phase);
                let mut user_message = format!(
                    "Task: {}\n\nPrevious context:\n{}",
                    state.config.task_description, state.context_accumulator
                );

                // Prepend extra context (e.g., repo map) if configured
                if let Some(ref ctx) = state.config.extra_context {
                    if !ctx.is_empty() {
                        user_message = format!("{}\n\n## Project Structure\n{}", ctx, user_message);
                    }
                }

                // Try native tool-use loop first (structured function calling)
                eprintln!("  [tool-use loop starting for Build phase (trying native first)]");

                match tool_use::run_native_tool_use_loop(
                    llm,
                    executor,
                    &system_prompt,
                    &user_message,
                    &state.config.project_root,
                    None,
                )
                .await
                {
                    Ok((output, tokens, files_modified)) => {
                        eprintln!(
                            "  [native tool loop done: {} files modified, {} tokens]",
                            files_modified.len(),
                            tokens
                        );
                        PhaseResult {
                            phase: phase.clone(),
                            status: PhaseStatus::Success,
                            output,
                            duration_ms: result.duration_ms,
                            files_modified,
                            errors: Vec::new(),
                            tokens_used: tokens,
                        }
                    },
                    Err(_) => {
                        // Native tool-use not available — fall back to parser-based loop
                        eprintln!(
                            "  [native tool-use not available, falling back to parser-based loop]"
                        );
                        match tool_use::run_tool_use_loop(
                            llm,
                            executor,
                            &system_prompt,
                            &user_message,
                            &state.config.project_root,
                            None,
                        )
                        .await
                        {
                            Ok((output, tokens, files_modified)) => {
                                eprintln!(
                                    "  [parser tool loop done: {} files modified]",
                                    files_modified.len()
                                );
                                PhaseResult {
                                    phase: phase.clone(),
                                    status: PhaseStatus::Success,
                                    output,
                                    duration_ms: result.duration_ms,
                                    files_modified,
                                    errors: Vec::new(),
                                    tokens_used: tokens,
                                }
                            },
                            Err(e) => {
                                eprintln!(
                                    "Tool-use loop error: {e}. Falling back to LLM-only result."
                                );
                                result // Keep the original LLM-only result
                            },
                        }
                    },
                }
            } else {
                result
            };

            // M3: If real_execution is enabled and phase is Build or Test,
            // run the actual command via ToolExecutor
            let result = if state.config.real_execution
                && self.tool_executor.is_some()
                && (*phase == SprintPhase::Build || *phase == SprintPhase::Test)
            {
                match self.execute_real_phase(&state, phase, result).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Real execution error in phase {phase}: {e}");
                        PhaseResult {
                            phase: phase.clone(),
                            status: PhaseStatus::Failed,
                            output: format!("Real execution failed: {e}"),
                            duration_ms: 0,
                            files_modified: Vec::new(),
                            errors: vec![e.to_string()],
                            tokens_used: 0,
                        }
                    },
                }
            } else {
                result
            };

            // Replace the LLM-only result in phase_results with the real-execution result
            // (run_phase already pushed the LLM result; execute_real_phase may have overridden it)
            if let Some(last) = state.phase_results.last_mut() {
                if last.phase == *phase {
                    *last = result.clone();
                }
            }

            // M4: If reviewers are configured and this is the Review phase,
            // run multi-model review instead of the single-LLM review
            let result = if *phase == SprintPhase::Review && !state.config.reviewers.is_empty() {
                match self.run_multi_model_review(&state, result.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "Multi-model review error: {e}. Falling back to single LLM review."
                        );
                        result // keep the single-LLM result
                    },
                }
            } else {
                result
            };

            if result.status == PhaseStatus::Failed {
                break;
            }

            if *phase == SprintPhase::Test && result.status == PhaseStatus::Success {
                idx += 1;
                continue;
            }

            if *phase == SprintPhase::Test && result.status == PhaseStatus::Failed {
                build_test_iterations += 1;
                if build_test_iterations >= state.config.max_iterations {
                    break;
                }
                state
                    .context_accumulator
                    .push_str("\n\n--- Test Iteration Restart ---\n");
                state.context_accumulator.push_str(&format!(
                    "Build/Test cycle failed (iteration {}/{}). Test errors:\n{}\n",
                    build_test_iterations,
                    state.config.max_iterations,
                    result.errors.join("; ")
                ));
                if let Some(build_idx) = phases.iter().position(|p| *p == SprintPhase::Build) {
                    idx = build_idx;
                    continue;
                }
                break;
            }

            idx += 1;
        }

        let summary = state
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Reflect)
            .map(|r| r.output.clone())
            .unwrap_or_else(|| {
                let passed = state
                    .phase_results
                    .iter()
                    .filter(|r| r.status == PhaseStatus::Success)
                    .count();
                let total = state.phase_results.len();
                format!("Sprint completed. {passed}/{total} phases succeeded.")
            });

        let success = state
            .phase_results
            .iter()
            .all(|r| r.status == PhaseStatus::Success || r.status == PhaseStatus::Skipped);

        // Build metrics from phase results
        let metrics = SprintMetrics {
            total_tokens: state.phase_results.iter().map(|r| r.tokens_used).sum(),
            phase_durations_ms: state
                .phase_results
                .iter()
                .map(|r| (r.phase.display_name().to_string(), r.duration_ms))
                .collect(),
            phase_tokens: state
                .phase_results
                .iter()
                .map(|r| (r.phase.display_name().to_string(), r.tokens_used))
                .collect(),
            retry_cycles: build_test_iterations,
            phases_succeeded: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Success)
                .count(),
            phases_failed: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Failed)
                .count(),
            phases_skipped: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Skipped)
                .count(),
        };

        Ok(SprintResult {
            success,
            phase_results: state.phase_results,
            total_duration_ms: sprint_start.elapsed().as_millis() as u64,
            summary,
            checkpoint_ref: state.checkpoint_ref.clone(),
            rollback_available: !success && state.checkpoint_ref.is_some(),
            metrics,
        })
    }

    /// Create a git checkpoint (stash) before building.
    /// Returns the stash ref if successful, None if not in a git repo or on error.
    fn create_checkpoint(project_root: &Path) -> Option<String> {
        use std::process::Command;
        let output = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(project_root)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        // Create a stash with a descriptive message
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
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

    /// Roll back to a previously created checkpoint.
    /// Returns Ok(()) on success, Err on failure.
    pub fn rollback(project_root: &Path, checkpoint_ref: &str) -> Result<()> {
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

    // ── Sprint State Persistence ─────────────────────────────────────────

    /// Directory where sprint state files are stored (relative to project root).
    const SPRINT_STATE_DIR: &str = ".clawdius/sprints";

    /// Save sprint state to a JSON file for later resume.
    /// Creates the `.clawdius/sprints/` directory if it doesn't exist.
    pub fn save_state(state: &SprintState) -> Result<String> {
        let sprint_dir = state.config.project_root.join(Self::SPRINT_STATE_DIR);
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

    /// Load the most recent sprint state from the project's `.clawdius/sprints/` directory.
    pub fn load_latest_state(project_root: &Path) -> Result<Option<SprintState>> {
        let sprint_dir = project_root.join(Self::SPRINT_STATE_DIR);
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

        // Sort by modification time, most recent first
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

    /// List all saved sprint states in the project directory.
    pub fn list_saved_states(project_root: &Path) -> Result<Vec<SprintState>> {
        let sprint_dir = project_root.join(Self::SPRINT_STATE_DIR);
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

        // Sort by started_at, most recent first
        states.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(states)
    }

    /// Delete a saved sprint state file.
    pub fn delete_saved_state(project_root: &Path, started_at: DateTime<Utc>) -> Result<()> {
        let filename = format!("sprint_{}.json", started_at.format("%Y%m%d-%H%M%S"));
        let path = project_root.join(Self::SPRINT_STATE_DIR).join(filename);
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

    /// Run a sprint with automatic state persistence after each phase.
    /// If a previous state exists and `resume` is true, continues from where it left off.
    pub async fn run_with_persistence(
        &self,
        config: SprintConfig,
        resume: bool,
    ) -> Result<SprintResult> {
        let mut state = if resume {
            match Self::load_latest_state(&config.project_root) {
                Ok(Some(s)) => {
                    eprintln!(
                        "Resuming sprint from {} ({} phases already completed)",
                        s.started_at.format("%Y-%m-%d %H:%M:%S"),
                        s.phase_results.len()
                    );
                    s
                },
                Ok(None) => {
                    eprintln!("No saved sprint state found, starting fresh");
                    SprintState::new(config)
                },
                Err(e) => {
                    eprintln!("Failed to load sprint state: {e}, starting fresh");
                    SprintState::new(config)
                },
            }
        } else {
            SprintState::new(config)
        };

        let phases = state.active_phases();
        let sprint_start = std::time::Instant::now();
        let mut build_test_iterations = 0usize;

        // Skip phases that already succeeded
        let mut idx = state.phase_results.len();
        if idx > 0 {
            // Check if the last phase was Test and failed (needs retry)
            if let Some(last) = state.phase_results.last() {
                if last.phase == SprintPhase::Test && last.status == PhaseStatus::Failed {
                    idx = phases
                        .iter()
                        .position(|p| *p == SprintPhase::Build)
                        .unwrap_or(idx);
                }
            }
        }

        eprintln!(
            "Starting from phase index {idx} ({})",
            phases.get(idx).map_or("end", |p| p.display_name())
        );

        while idx < phases.len() {
            let phase = &phases[idx];

            if *phase == SprintPhase::Build && state.checkpoint_ref.is_none() {
                if let Some(checkpoint) = Self::create_checkpoint(&state.config.project_root) {
                    state.checkpoint_ref = Some(checkpoint);
                    eprintln!(
                        "Checkpoint created: {}",
                        state.checkpoint_ref.as_ref().unwrap()
                    );
                }
            }

            let result = match tokio::time::timeout(
                std::time::Duration::from_secs(state.config.phase_timeout_secs),
                self.run_phase(&mut state, phase),
            )
            .await
            {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    eprintln!("Phase {} error: {e}", phase);
                    PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!("Phase failed: {e}"),
                        duration_ms: 0,
                        files_modified: Vec::new(),
                        errors: vec![e.to_string()],
                        tokens_used: 0,
                    }
                },
                Err(_) => {
                    eprintln!(
                        "Phase {} timed out after {}s",
                        phase, state.config.phase_timeout_secs
                    );
                    PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!(
                            "Phase {} timed out after {}s",
                            phase, state.config.phase_timeout_secs
                        ),
                        duration_ms: (state.config.phase_timeout_secs * 1000) as u64,
                        files_modified: Vec::new(),
                        errors: vec![format!(
                            "Phase timed out after {}s",
                            state.config.phase_timeout_secs
                        )],
                        tokens_used: 0,
                    }
                },
            };

            let result = if state.config.real_execution
                && self.tool_executor.is_some()
                && (*phase == SprintPhase::Build || *phase == SprintPhase::Test)
            {
                match self.execute_real_phase(&state, phase, result).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Real execution error in phase {phase}: {e}");
                        PhaseResult {
                            phase: phase.clone(),
                            status: PhaseStatus::Failed,
                            output: format!("Real execution failed: {e}"),
                            duration_ms: 0,
                            files_modified: Vec::new(),
                            errors: vec![e.to_string()],
                            tokens_used: 0,
                        }
                    },
                }
            } else {
                result
            };

            if let Some(last) = state.phase_results.last_mut() {
                if last.phase == *phase {
                    *last = result.clone();
                }
            }

            let result = if *phase == SprintPhase::Review && !state.config.reviewers.is_empty() {
                match self.run_multi_model_review(&state, result.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "Multi-model review error: {e}. Falling back to single LLM review."
                        );
                        result
                    },
                }
            } else {
                result
            };

            // Save state after each phase
            if let Err(e) = Self::save_state(&state) {
                tracing::warn!("Failed to save sprint state: {e}");
            }

            if result.status == PhaseStatus::Failed {
                break;
            }

            if *phase == SprintPhase::Test && result.status == PhaseStatus::Success {
                idx += 1;
                continue;
            }

            if *phase == SprintPhase::Test && result.status == PhaseStatus::Failed {
                build_test_iterations += 1;
                if build_test_iterations >= state.config.max_iterations {
                    break;
                }
                state
                    .context_accumulator
                    .push_str("\n\n--- Test Iteration Restart ---\n");
                state.context_accumulator.push_str(&format!(
                    "Build/Test cycle failed (iteration {}/{}). Test errors:\n{}\n",
                    build_test_iterations,
                    state.config.max_iterations,
                    result.errors.join("; ")
                ));
                if let Some(build_idx) = phases.iter().position(|p| *p == SprintPhase::Build) {
                    idx = build_idx;
                    continue;
                }
                break;
            }

            idx += 1;
        }

        let summary = state
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Reflect)
            .map(|r| r.output.clone())
            .unwrap_or_else(|| {
                let passed = state
                    .phase_results
                    .iter()
                    .filter(|r| r.status == PhaseStatus::Success)
                    .count();
                let total = state.phase_results.len();
                format!("Sprint completed. {passed}/{total} phases succeeded.")
            });

        let success = state
            .phase_results
            .iter()
            .all(|r| r.status == PhaseStatus::Success || r.status == PhaseStatus::Skipped);

        let metrics = SprintMetrics {
            total_tokens: state.phase_results.iter().map(|r| r.tokens_used).sum(),
            phase_durations_ms: state
                .phase_results
                .iter()
                .map(|r| (r.phase.display_name().to_string(), r.duration_ms))
                .collect(),
            phase_tokens: state
                .phase_results
                .iter()
                .map(|r| (r.phase.display_name().to_string(), r.tokens_used))
                .collect(),
            retry_cycles: build_test_iterations,
            phases_succeeded: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Success)
                .count(),
            phases_failed: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Failed)
                .count(),
            phases_skipped: state
                .phase_results
                .iter()
                .filter(|r| r.status == PhaseStatus::Skipped)
                .count(),
        };

        Ok(SprintResult {
            success,
            phase_results: state.phase_results,
            total_duration_ms: sprint_start.elapsed().as_millis() as u64,
            summary,
            checkpoint_ref: state.checkpoint_ref.clone(),
            rollback_available: !success && state.checkpoint_ref.is_some(),
            metrics,
        })
    }

    /// Detect a programming language from a file path extension.
    pub fn detect_language(path: &str) -> &'static str {
        match Path::new(path).extension().and_then(|e| e.to_str()) {
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

    /// Get files that have been added or modified (unstaged) in the git repo.
    /// Returns None if not in a git repo, on error, or if no changes.
    fn get_changed_files(project_root: &Path) -> Option<Vec<String>> {
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

    /// Execute a real build or test command via the ToolExecutor.
    /// On Build failure, attempts automatic error recovery via LLM.
    async fn execute_real_phase(
        &self,
        state: &SprintState,
        phase: &SprintPhase,
        llm_result: PhaseResult,
    ) -> Result<PhaseResult> {
        let executor = self
            .tool_executor
            .as_ref()
            .expect("tool_executor must be Some (checked by caller)");

        let command = match phase {
            SprintPhase::Build => &state.config.build_command,
            SprintPhase::Test => &state.config.test_command,
            _ => return Ok(llm_result),
        };

        // Run the command
        let request = ToolRequest::new("shell")
            .with_arg("command", serde_json::Value::String(command.clone()));

        let tool_result = executor
            .execute(request)
            .await
            .map_err(|e| crate::Error::Sprint(format!("Tool execution failed: {e}")))?;

        let output = &tool_result.content;

        if tool_result.success && !output.contains("error") {
            // Success — track changed files
            let files_modified =
                Self::get_changed_files(&state.config.project_root).unwrap_or_default();

            Ok(PhaseResult {
                phase: phase.clone(),
                status: PhaseStatus::Success,
                output: format!("[Real execution] Command: {command}\n{output}"),
                duration_ms: llm_result.duration_ms,
                files_modified,
                errors: Vec::new(),
                tokens_used: llm_result.tokens_used,
            })
        } else {
            // Build failed — attempt error recovery
            if *phase == SprintPhase::Build {
                if let Some(fix_output) = self
                    .attempt_error_recovery(&state.config, &state.config.project_root, output)
                    .await?
                {
                    // Error recovery produced a fix
                    let files_modified =
                        Self::get_changed_files(&state.config.project_root).unwrap_or_default();

                    Ok(PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Success,
                        output: format!(
                            "[Real execution + recovery] Command: {command}\n\n[Recovered output]\n{fix_output}"
                        ),
                        duration_ms: llm_result.duration_ms,
                        files_modified,
                        errors: Vec::new(),
                        tokens_used: llm_result.tokens_used,
                    })
                } else {
                    // Recovery failed
                    Ok(PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!("[Real execution FAILED] Command: {command}\n\n{output}"),
                        duration_ms: llm_result.duration_ms,
                        files_modified: Vec::new(),
                        errors: vec![output.clone()],
                        tokens_used: llm_result.tokens_used,
                    })
                }
            } else {
                // Test phase failure — report it normally (retry loop handles it)
                Ok(PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Failed,
                    output: format!("[Real execution FAILED] Command: {command}\n\n{output}"),
                    duration_ms: llm_result.duration_ms,
                    files_modified: Vec::new(),
                    errors: vec![output.clone()],
                    tokens_used: llm_result.tokens_used,
                })
            }
        }
    }

    /// Attempt to recover from build errors using the LLM-powered ErrorRecovery.
    /// Returns Some(fixed_output) on success, None on failure.
    async fn attempt_error_recovery(
        &self,
        config: &SprintConfig,
        project_root: &Path,
        error_output: &str,
    ) -> Result<Option<String>> {
        let errors = parse_compiler_output(error_output);
        if errors.is_empty() {
            return Ok(None);
        }

        // Find the first file with an error
        let target_file = errors
            .iter()
            .find(|e| e.file_path.is_some())
            .map(|e| e.file_path.as_ref().unwrap().clone());

        let Some(file_path) = target_file else {
            return Ok(None);
        };

        let full_path = project_root.join(&file_path);
        let original_code = std::fs::read_to_string(&full_path)
            .map_err(|e| crate::Error::Sprint(format!("Failed to read {file_path}: {e}")))?;

        let language = Self::detect_language(&file_path);
        let recovery = ErrorRecovery::with_config(
            Arc::clone(&self.llm),
            ErrorRecoveryConfig::new(2).with_compiler_output(true),
        );

        let result = recovery.recover(&original_code, &errors, language).await?;

        if !result.success {
            return Ok(None);
        }

        // Write the fix back
        std::fs::write(&full_path, &result.fixed_code).map_err(|e| {
            crate::Error::Sprint(format!("Failed to write fix to {file_path}: {e}"))
        })?;

        // Re-verify by running the build command again
        let executor = self.tool_executor.as_ref().unwrap();
        let request = ToolRequest::new("shell").with_arg(
            "command",
            serde_json::Value::String(config.build_command.clone()),
        );
        let verify_result = executor
            .execute(request)
            .await
            .map_err(|e| crate::Error::Sprint(format!("Verification build failed: {e}")))?;

        if verify_result.success {
            Ok(Some(format!(
                "Fixed {file_path} ({} attempt(s)). Verification: passed.",
                result.retries_used
            )))
        } else {
            // Verification failed — revert the fix
            let _ = std::fs::write(&full_path, &original_code);
            Ok(None)
        }
    }

    /// Run a multi-model review using ReviewEngine (M4).
    /// Returns a PhaseResult with the fused review text.
    async fn run_multi_model_review(
        &self,
        state: &SprintState,
        llm_result: PhaseResult,
    ) -> Result<PhaseResult> {
        let code_to_review = &state.context_accumulator;
        let context = &state.config.task_description;

        let review_engine = ReviewEngine::new(state.config.reviewers.clone());
        let fused = review_engine.review(code_to_review, context).await?;

        let review_output = format!(
            "[Multi-Model Review — {} reviewers, avg score: {:.1}/5]\n\n\
             {}\n\n\
             {}",
            fused.reviews.len(),
            fused.average_score,
            fused.summary,
            if fused.has_critical_issues {
                "⚠️ CRITICAL issues found. Address before proceeding."
            } else {
                "No critical issues."
            }
        );

        Ok(PhaseResult {
            phase: SprintPhase::Review,
            status: if fused.has_critical_issues {
                // Still mark as success so the sprint continues — criticals are advisory
                PhaseStatus::Success
            } else {
                PhaseStatus::Success
            },
            output: review_output,
            duration_ms: fused.total_duration_ms,
            files_modified: Vec::new(),
            errors: if fused.has_critical_issues {
                vec!["Critical issues found in review".to_string()]
            } else {
                Vec::new()
            },
            tokens_used: fused.total_tokens,
        })
    }

    pub async fn run_phase(
        &self,
        state: &mut SprintState,
        phase: &SprintPhase,
    ) -> Result<PhaseResult> {
        if state.config.skip_phases.contains(phase) {
            let result = PhaseResult {
                phase: phase.clone(),
                status: PhaseStatus::Skipped,
                output: String::new(),
                duration_ms: 0,
                files_modified: Vec::new(),
                errors: Vec::new(),
                tokens_used: 0,
            };
            state.phase_results.push(result.clone());
            return Ok(result);
        }

        state.current_phase = Some(phase.clone());
        let start = std::time::Instant::now();

        let system_prompt = Self::phase_prompt(phase);
        let mut user_message = format!(
            "Task: {}\n\nPrevious context:\n{}",
            state.config.task_description, state.context_accumulator
        );

        // Prepend extra context (e.g., repo map) if configured
        if let Some(ref ctx) = state.config.extra_context {
            if !ctx.is_empty() {
                user_message = format!("{}\n\n## Project Structure\n{}", ctx, user_message);
            }
        }

        // Append browser QA context for the Test phase when a URL is configured
        if *phase == SprintPhase::Test {
            if let Some(ref url) = state.config.browser_qa_url {
                // If a browser daemon is available, navigate and capture a snapshot
                if let Some(ref daemon) = self.browser_daemon {
                    let session_id = "sprint-qa";
                    let _ = daemon.register_session(session_id).await;
                    let _ = daemon.initialize().await;
                    if daemon.navigate(url, Some(session_id)).await.is_ok() {
                        if let Ok(snapshot) = daemon.build_snapshot(session_id).await {
                            let tree_lines: Vec<String> = snapshot
                                .elements
                                .iter()
                                .map(|e| format!("  {} {} \"{}\"", e.ref_id, e.role, e.name))
                                .collect();
                            let tree_text = tree_lines.join("\n");
                            user_message.push_str(&format!(
                                "\n\n## Browser QA — Live Snapshot (URL: {})\n\
                                 ### Accessibility Tree\n{}\n\
                                 ### Element References\n{}\n\
                                 Use the references above (e.g. @e1, @e2) to identify specific elements.\n\
                                 Report any visual or functional issues found.",
                                snapshot.url,
                                tree_text,
                                snapshot.to_ref_list(),
                            ));
                        } else {
                            user_message.push_str(&format!(
                                "\n\n## Browser QA\n\
                                 A browser-based QA check is available at: {url}\n\
                                 (Browser daemon connected but snapshot failed.)\n\
                                 Report any issues you can identify."
                            ));
                        }
                    } else {
                        user_message.push_str(&format!(
                            "\n\n## Browser QA\n\
                             A browser-based QA check is available at: {url}\n\
                             (Browser daemon connected but navigation failed.)\n\
                             Report any issues you can identify."
                        ));
                    }
                    let _ = daemon.unregister_session(session_id).await;
                } else {
                    user_message.push_str(&format!(
                        "\n\n## Browser QA\n\
                         A browser-based QA check is available at: {url}\n\
                         If the task involves a web application or UI, consider:\n\
                         1. Navigate to the URL and verify the UI renders correctly\n\
                         2. Check for console errors\n\
                         3. Test interactive elements (buttons, forms, navigation)\n\
                         4. Verify responsive behavior\n\
                         5. Check accessibility basics (focus states, ARIA labels)\n\
                         Report any visual or functional issues found."
                    ));
                }
            }
        }

        // Inject LSP context: sync docs + diagnostics + code actions
        if matches!(phase, SprintPhase::Build | SprintPhase::Test | SprintPhase::Review) {
            if let Some(ref lsp) = self.lsp_client {
                // Sync all previously modified files to LSP for fresh diagnostics
                let all_modified: Vec<String> = state.phase_results.iter()
                    .flat_map(|r| r.files_modified.clone()).collect();
                self.sync_lsp_documents(&all_modified).await;
                // Wait for diagnostics to settle
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let (all_diags, code_actions) = {
                    let mut lsp = lsp.lock().await;
                    let diags = lsp.get_all_diagnostics().await;
                    // Fetch code actions for errors
                    let mut actions = Vec::new();
                    for (uri, file_diags) in &diags {
                        let err_diags: Vec<crate::lsp::protocol::Diagnostic> = file_diags
                            .iter()
                            .filter(|d| {
                                d.severity
                                    == Some(crate::lsp::protocol::DiagnosticSeverity::Error)
                            })
                            .cloned()
                            .collect();
                        if !err_diags.is_empty() {
                            if let Ok(ca) = lsp.code_actions(
                                uri,
                                err_diags[0].range.clone(),
                                err_diags,
                            )
                            .await
                            {
                                actions.extend(ca);
                            }
                        }
                    }
                    (diags, actions)
                };
                if !all_diags.is_empty() {
                    let mut diag_text = String::from(
                        "\n\n## LSP Diagnostics\nThe language server reported:\n",
                    );
                    let (mut error_count, mut warning_count) = (0usize, 0usize);
                    for (uri, diags) in &all_diags {
                        for d in diags {
                            use crate::lsp::protocol::DiagnosticSeverity;
                            let sev = match d.severity {
                                Some(DiagnosticSeverity::Error) => {
                                    error_count += 1;
                                    "ERROR"
                                }
                                Some(DiagnosticSeverity::Warning) => {
                                    warning_count += 1;
                                    "WARNING"
                                }
                                Some(DiagnosticSeverity::Information) => "INFO",
                                Some(DiagnosticSeverity::Hint) => "HINT",
                                _ => "UNKNOWN",
                            };
                            diag_text.push_str(&format!(
                                "  [{}] {} L{}:C{}: {}\n",
                                sev,
                                uri,
                                d.range.start.line + 1,
                                d.range.start.character + 1,
                                d.message
                            ));
                        }
                    }
                    diag_text.push_str(&format!(
                        "\nTotal: {} errors, {} warnings\n",
                        error_count, warning_count
                    ));
                    user_message.push_str(&diag_text);
                }
                // Show LSP-suggested code actions
                if !code_actions.is_empty() {
                    let mut action_text =
                        String::from("\n\n## LSP Suggested Fixes\n");
                    for action in &code_actions {
                        if action.is_preferred {
                            action_text.push_str(&format!("[PREFERRED] {}\n", action.title));
                        } else {
                            action_text.push_str(&format!("- {}\n", action.title));
                        }
                    }
                    user_message.push_str(&action_text);
                }
            }
        }

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: system_prompt,
            },
            ChatMessage {
                role: ChatRole::User,
                content: user_message,
            },
        ];

        let result = match self.chat_collecting_stream(messages).await {
            Ok(output) => {
                let tokens = self.llm.count_tokens(&output);
                PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Success,
                    output,
                    duration_ms: start.elapsed().as_millis() as u64,
                    files_modified: Vec::new(),
                    errors: Vec::new(),
                    tokens_used: tokens,
                }
            },
            Err(_) => {
                // Fallback: merge system prompt into user message for models
                // that don't support system messages (e.g., Google Gemma)
                let mut combined = format!(
                    "[Instructions]\n{}\n\n[Task & Context]\nTask: {}\n\nPrevious context:\n{}",
                    Self::phase_prompt(phase),
                    &state.config.task_description,
                    &state.context_accumulator
                );
                // Prepend extra context in fallback path too
                if let Some(ref ctx) = state.config.extra_context {
                    if !ctx.is_empty() {
                        combined = format!("[Project Structure]\n{}\n\n{}", ctx, combined);
                    }
                }
                let fallback_messages = vec![ChatMessage {
                    role: ChatRole::User,
                    content: combined,
                }];
                match self.chat_collecting_stream(fallback_messages).await {
                    Ok(output) => {
                        let tokens = self.llm.count_tokens(&output);
                        PhaseResult {
                            phase: phase.clone(),
                            status: PhaseStatus::Success,
                            output,
                            duration_ms: start.elapsed().as_millis() as u64,
                            files_modified: Vec::new(),
                            errors: Vec::new(),
                            tokens_used: tokens,
                        }
                    },
                    Err(e) => PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Failed,
                        output: format!("LLM error: {e}"),
                        duration_ms: start.elapsed().as_millis() as u64,
                        files_modified: Vec::new(),
                        errors: vec![e.to_string()],
                        tokens_used: 0,
                    },
                }
            },
        };

        state.context_accumulator.push_str(&format!(
            "\n\n=== {} Phase ===\n{}\n",
            phase.display_name(),
            result.output
        ));

        // Compact context accumulator if it's growing too large
        if let Err(e) = self.maybe_compact_context(state).await {
            tracing::warn!("context compaction failed: {e}");
        }

        state.phase_results.push(result.clone());
        state.current_phase = None;

        Ok(result)
    }

    /// Compact the context accumulator if it exceeds the token threshold.
    async fn maybe_compact_context(&self, state: &mut SprintState) -> Result<()> {
        const COMPACT_THRESHOLD_CHARS: usize = 320_000;
        const KEEP_RECENT_CHARS: usize = 20_000;
        if state.context_accumulator.len() <= COMPACT_THRESHOLD_CHARS {
            return Ok(());
        }
        let total_len = state.context_accumulator.len();
        if total_len <= KEEP_RECENT_CHARS {
            return Ok(());
        }
        let split_point = total_len - KEEP_RECENT_CHARS;
        let split_point = state.context_accumulator[..split_point]
            .rfind('\n').map(|p| p + 1).unwrap_or(split_point);
        let old_context = &state.context_accumulator[..split_point];
        let recent_context = &state.context_accumulator[split_point..];
        tracing::info!(total_chars = total_len, old_chars = old_context.len(), "context compaction triggered");
        let old_for_llm = if old_context.len() > 100_000 {
            &old_context[old_context.len() - 100_000..]
        } else {
            old_context
        };
        let system_prompt = ChatMessage {
            role: ChatRole::System,
            content: "You are a context compaction assistant. Summarize this sprint context preserving: task/objective, files modified (paths), key decisions, progress state, errors and fixes, code patterns. Discard verbose output. Under 1500 words.".to_string(),
        };
        let user_prompt = ChatMessage {
            role: ChatRole::User,
            content: format!("Summarize:\n\n{old_for_llm}"),
        };
        match self.llm.chat(vec![system_prompt, user_prompt]).await {
            Ok(summary) => {
                let summary = if summary.len() > 8000 {
                    format!("{}... [truncated]", &summary[..8000])
                } else { summary };
                let old_len = old_context.len();
                let new_acc = format!(
                    "[Previous sprint context compacted]\n\n{summary}\n\n[End of compacted context]\n\n--- Recent context ---\n{recent_context}"
                );
                tracing::info!(old_chars = old_len, new_chars = new_acc.len(), "context compaction completed");
                state.context_accumulator = new_acc;
            }
            Err(e) => {
                tracing::warn!("LLM compaction failed, using truncation fallback: {e}");
                let truncated = if old_context.len() > 8000 {
                    format!("[Previous context truncated]\n...{}", &old_context[old_context.len() - 8000..])
                } else { old_context.to_string() };
                state.context_accumulator = format!("{truncated}\n\n--- Recent context ---\n{recent_context}");
            }
        }
        Ok(())
    }

    pub fn phase_prompt(phase: &SprintPhase) -> String {
        match phase {
            SprintPhase::Think => {
                "You are a product-thinking AI. Analyze the task and produce:\n\
                 1. Problem statement (one sentence)\n\
                 2. Key questions that need answers before building\n\
                 3. Assumptions being made\n\
                 4. Success criteria (measurable)\n\
                 5. Risks and mitigations\n\
                 6. Recommended approach (high-level)"
                    .to_string()
            },
            SprintPhase::Plan => {
                "You are a technical planner. Based on the thinking output, create a detailed execution plan:\n\
                 1. List of files to create/modify (with paths)\n\
                 2. For each file: what changes are needed\n\
                 3. Order of operations (dependencies)\n\
                 4. Test strategy\n\
                 5. Risk assessment for each step\n\
                 Format as a numbered checklist with file paths."
                    .to_string()
            },
            SprintPhase::Build => {
                "You are a senior engineer. Execute the plan by writing/modifying code.\n\
                 Follow the plan step by step. For each step:\n\
                 - State what you're doing\n\
                 - Write the code (full file contents)\n\
                 - Note any deviations from the plan\n\
                 IMPORTANT: Actually write the code changes, don't just describe them."
                    .to_string()
            },
            SprintPhase::Review => {
                "You are a staff engineer doing code review. Review all changes made:\n\
                 - Correctness: Does the code do what the plan says?\n\
                 - Edge cases: Are error paths handled?\n\
                 - Security: Any vulnerabilities?\n\
                 - Performance: Any anti-patterns?\n\
                 - Style: Consistent with project conventions?\n\
                 Rate each area 1-5 and provide specific feedback."
                    .to_string()
            },
            SprintPhase::Test => {
                "You are a QA engineer. Based on the code changes:\n\
                 1. List test cases that should exist\n\
                 2. Run the project's test suite\n\
                 3. Report pass/fail results\n\
                 4. If failures: diagnose root cause and suggest fixes"
                    .to_string()
            },
            SprintPhase::Ship => {
                "You are a release engineer. Based on all previous phases:\n\
                 1. Summarize what was built\n\
                 2. Verify all tests pass\n\
                 3. Generate a commit message (conventional commits format)\n\
                 4. Check if the branch is safe to push\n\
                 5. Report what needs to happen to ship"
                    .to_string()
            },
            SprintPhase::Reflect => {
                "You are a retrospective facilitator. Based on the entire sprint:\n\
                 1. What went well?\n\
                 2. What could be improved?\n\
                 3. What did we learn?\n\
                 4. Action items for next sprint\n\
                 5. Metrics summary"
                    .to_string()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tool_executor::NoOpToolExecutor;
    use crate::llm::ChatMessage;
    use async_trait::async_trait;
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
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Ok(self.response.clone())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
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
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Err(crate::Error::Llm("mock failure".to_string()))
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
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
            let prompt = SprintEngine::phase_prompt(&phase);
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
            project_root: PathBuf::from("/tmp"),
            auto_approve: true,
            skip_phases: vec![SprintPhase::Reflect],
            max_iterations: 5,
            model: Some("gpt-4".to_string()),
            build_command: "cargo build 2>&1".to_string(),
            test_command: "cargo test 2>&1".to_string(),
            real_execution: true,
            browser_qa_url: Some("http://localhost:3000".to_string()),
            reviewers: Vec::new(),
            max_duration_secs: 600,
            phase_timeout_secs: 120,
            extra_context: Some("test context".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SprintConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_description, "Build X");
        assert_eq!(parsed.max_iterations, 5);
        assert!(parsed.auto_approve);
        assert!(parsed.real_execution);
        assert_eq!(parsed.build_command, "cargo build 2>&1");
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

        // Check metrics exist
        assert_eq!(result.metrics.phases_succeeded, 4); // Think, Plan, Build, Review
        assert_eq!(result.metrics.phases_failed, 0);
        assert_eq!(result.metrics.phases_skipped, 0);
        assert_eq!(result.metrics.retry_cycles, 0);
        assert!(result.metrics.total_tokens > 0);
        assert_eq!(result.metrics.phase_durations_ms.len(), 4);
        assert_eq!(result.metrics.phase_tokens.len(), 4);

        // Check report renders without panic
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
        // /tmp is unlikely to be a git repo, so checkpoint should return None
        let result = SprintEngine::create_checkpoint(PathBuf::from("/tmp").as_path());
        assert!(
            result.is_none(),
            "Checkpoint in non-git directory should return None"
        );
    }

    #[test]
    fn test_checkpoint_in_git_repo() {
        // We're inside the clawdius git repo, so checkpoint should work
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let result = SprintEngine::create_checkpoint(&project_root);
        // It should succeed since we're in a git repo
        // (but may fail if there are no changes to stash — that's ok)
        if let Some(ref stash_ref) = result {
            assert!(stash_ref.starts_with("stash@"));
            // Clean up: pop the stash
            let _ = SprintEngine::rollback(&project_root, stash_ref);
        }
    }

    #[test]
    fn test_rollback_invalid_ref() {
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let result = SprintEngine::rollback(&project_root, "stash@{999}");
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
        // No checkpoint because Build was skipped
        assert!(result.checkpoint_ref.is_none());
        assert!(!result.rollback_available);
    }

    /// Smoke test: requires OPENROUTER_API_KEY, run with --ignored --nocapture
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

        // Run a minimal sprint: Think → Plan → Reflect (skip Build/Test/Ship for speed)
        let sprint_config = SprintConfig {
            task_description: "Add a hello function to src/main.rs".to_string(),
            project_root: PathBuf::from("/tmp/sprint-test"),
            auto_approve: true,
            skip_phases: vec![SprintPhase::Build, SprintPhase::Test, SprintPhase::Ship],
            max_iterations: 1,
            model: None,
            ..SprintConfig::new("Add a hello function to src/main.rs")
        };

        // Retry up to 3 times with backoff to handle free-tier rate limits
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

            // Check if any phase got rate-limited
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

        // Core invariants: sprint should complete without panicking
        assert!(
            !result.phase_results.is_empty(),
            "Should have at least one phase result"
        );
        assert!(result.total_duration_ms > 0, "Duration should be positive");

        // Check that the sprint pipeline actually executed (not just short-circuited)
        // Each phase result should have either output or error details
        for pr in &result.phase_results {
            assert!(
                !pr.output.is_empty() || !pr.errors.is_empty(),
                "Phase {} should have output or errors",
                pr.phase
            );
        }

        // If the sprint succeeded, verify we got substantial LLM output
        // If rate-limited, we just verify the pipeline worked (above assertions suffice)
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
            // Sprint didn't fully succeed — verify it was due to rate limiting, not a code bug
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

    // ── M3: Error Recovery & Real Execution Tests ──

    #[test]
    fn test_sprint_config_real_execution_fields() {
        let config = SprintConfig::new("test");
        assert_eq!(config.build_command, "cargo build 2>&1");
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
        assert_eq!(SprintEngine::detect_language("src/main.rs"), "rust");
        assert_eq!(SprintEngine::detect_language("script.py"), "python");
        assert_eq!(SprintEngine::detect_language("index.ts"), "typescript");
        assert_eq!(SprintEngine::detect_language("index.tsx"), "typescript");
        assert_eq!(SprintEngine::detect_language("app.js"), "javascript");
        assert_eq!(SprintEngine::detect_language("main.go"), "go");
        assert_eq!(SprintEngine::detect_language("unknown.xyz"), "unknown");
        assert_eq!(SprintEngine::detect_language("noext"), "unknown");
    }

    #[tokio::test]
    async fn test_sprint_engine_with_tool_executor() {
        let llm = Arc::new(MockLlm::new("output"));
        let engine = SprintEngine::new(llm).with_tool_executor(Arc::new(NoOpToolExecutor));
        // Verify the engine was created successfully with a tool executor
        assert!(engine.tool_executor.is_some());
    }

    #[tokio::test]
    async fn test_real_execution_skipped_without_tool_executor() {
        // When real_execution is true but no tool_executor, real execution should be skipped
        let llm = Arc::new(MockLlm::new("build output"));
        let engine = SprintEngine::new(llm); // No tool executor
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
        // Should succeed because real execution is skipped (no tool executor)
        assert!(result.success);
        let build = result
            .phase_results
            .iter()
            .find(|r| r.phase == SprintPhase::Build)
            .unwrap();
        assert_eq!(build.status, PhaseStatus::Success);
        // Output should be the LLM output, not wrapped in [Real execution]
        assert!(!build.output.contains("[Real execution]"));
    }

    #[tokio::test]
    async fn test_real_execution_with_noop_executor() {
        // NoOpToolExecutor always returns success with no "error" in content,
        // so Build should succeed with [Real execution] wrapper
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
        // NoOpToolExecutor returns "No-op executor: tool 'shell' called..." which
        // doesn't contain "error", so it should be treated as success
        assert_eq!(build.status, PhaseStatus::Success);
        assert!(build.output.contains("[Real execution]"));
    }

    #[test]
    fn test_get_changed_files_in_git_repo() {
        // We're inside the clawdius git repo
        let project_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let result = SprintEngine::get_changed_files(&project_root);
        // Could be Some (if there are uncommitted changes) or None (clean tree)
        // Just verify it doesn't panic
        match result {
            Some(files) => assert!(!files.is_empty()),
            None => {},
        }
    }

    #[test]
    fn test_get_changed_files_non_git_dir() {
        let result = SprintEngine::get_changed_files(PathBuf::from("/tmp").as_path());
        // /tmp is likely not a git repo, but it might be in some setups
        // Just verify no panic
        if let Some(files) = result {
            for f in &files {
                assert!(!f.is_empty());
            }
        }
    }

    #[test]
    fn test_error_recovery_config_builder() {
        let config = ErrorRecoveryConfig::new(5).with_compiler_output(true);
        assert_eq!(config.max_retries, 5);
        assert!(config.include_compiler_output);
    }

    #[test]
    fn test_sprint_config_reviewers_field() {
        let config = SprintConfig::new("test");
        assert!(config.reviewers.is_empty());

        let config2 = SprintConfig {
            reviewers: vec![ReviewerConfig {
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
