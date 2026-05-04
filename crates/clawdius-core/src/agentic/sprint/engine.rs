use super::{
    create_checkpoint, get_changed_files, load_latest_state, run_multi_model_review,
    save_state, PhaseResult, PhaseStatus, SprintConfig, SprintError, SprintMetrics, SprintPhase,
    SprintResult, SprintState,
};
use crate::agentic::browser_daemon::BrowserDaemon;
use crate::agentic::tool_executor::{ToolExecutor, ToolRequest};
use crate::agentic::tool_use;
use crate::llm::providers::LlmClient;
use crate::Result;
use std::path::Path;
use std::sync::Arc;

pub struct SprintEngine {
    pub(crate) llm: Arc<dyn LlmClient>,
    pub(crate) tool_executor: Option<Arc<dyn ToolExecutor>>,
    pub(crate) browser_daemon: Option<Arc<BrowserDaemon>>,
    pub(crate) lsp_client: Option<Arc<tokio::sync::Mutex<crate::lsp::LspClient>>>,
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

    #[must_use]
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    #[must_use]
    pub fn with_browser_daemon(mut self, daemon: Arc<BrowserDaemon>) -> Self {
        self.browser_daemon = Some(daemon);
        self
    }

    #[must_use]
    pub fn with_lsp_client(mut self, client: crate::lsp::LspClient) -> Self {
        self.lsp_client = Some(Arc::new(tokio::sync::Mutex::new(client)));
        self
    }

    pub(crate) async fn chat_collecting_stream(&self, messages: Vec<crate::llm::ChatMessage>) -> crate::Result<String> {
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

    pub async fn run_phase(
        &self,
        state: &mut SprintState,
        phase: &SprintPhase,
    ) -> Result<PhaseResult> {
        crate::agentic::sprint::phases::run_phase(self, state, phase).await
    }

    /// Run a single phase with timeout, returning a PhaseResult even on timeout/error.
    async fn run_phase_with_timeout(
        &self,
        state: &mut SprintState,
        phase: &SprintPhase,
    ) -> PhaseResult {
        match tokio::time::timeout(
            std::time::Duration::from_secs(state.config.phase_timeout_secs),
            self.run_phase(state, phase),
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
        }
    }

    /// If this is a Build phase with tool_executor, try the tool-use loop.
    async fn maybe_apply_tool_use(
        &self,
        state: &SprintState,
        phase: &SprintPhase,
        result: PhaseResult,
    ) -> PhaseResult {
        if *phase != SprintPhase::Build
            || self.tool_executor.is_none()
            || result.status != PhaseStatus::Success
        {
            return result;
        }

        let executor = self
            .tool_executor
            .as_ref()
            .expect("guarded by is_some() check above");
        let llm = &self.llm;
        let system_prompt = crate::agentic::sprint::phases::phase_prompt(phase);
        let mut user_message = format!(
            "Task: {}\n\nPrevious context:\n{}",
            state.config.task_description, state.context_accumulator
        );
        if let Some(ref ctx) = state.config.extra_context {
            if !ctx.is_empty() {
                user_message = format!("{}\n\n## Project Structure\n{}", ctx, user_message);
            }
        }

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
                        result
                    },
                }
            },
        }
    }

    /// Apply real execution for Build/Test phases if configured.
    async fn maybe_apply_real_execution(
        &self,
        state: &SprintState,
        phase: &SprintPhase,
        result: PhaseResult,
    ) -> PhaseResult {
        if !state.config.real_execution
            || self.tool_executor.is_none()
            || !matches!(phase, SprintPhase::Build | SprintPhase::Test)
        {
            return result;
        }
        match super::execute_real_phase(self, state, phase, result).await {
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
    }

    /// Apply multi-model review for Review phase if reviewers configured.
    async fn maybe_apply_review(
        &self,
        state: &SprintState,
        phase: &SprintPhase,
        result: PhaseResult,
    ) -> PhaseResult {
        if *phase != SprintPhase::Review || state.config.reviewers.is_empty() {
            return result;
        }
        match run_multi_model_review(self, state, result.clone()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "Multi-model review error: {e}. Falling back to single LLM review."
                );
                result
            },
        }
    }

    /// Handle build/test retry cycle. Returns the next phase index and updated iteration count.
    fn handle_test_retry(
        &self,
        phase: &SprintPhase,
        phases: &[SprintPhase],
        result: &PhaseResult,
        state: &mut SprintState,
        build_test_iterations: &mut usize,
    ) -> TestRetryAction {
        if *phase != SprintPhase::Test || result.status != PhaseStatus::Failed {
            return TestRetryAction::Continue;
        }

        *build_test_iterations += 1;
        if *build_test_iterations >= state.config.max_iterations {
            return TestRetryAction::Break;
        }

        state
            .context_accumulator
            .push_str("\n\n--- Test Iteration Restart ---\n");
        state.context_accumulator.push_str(&format!(
            "Build/Test cycle failed (iteration {}/{}). Test errors:\n{}\n",
            *build_test_iterations,
            state.config.max_iterations,
            result.errors.join("; ")
        ));

        if let Some(build_idx) = phases.iter().position(|p| *p == SprintPhase::Build) {
            return TestRetryAction::RestartAt(build_idx);
        }
        TestRetryAction::Break
    }

    /// Build SprintMetrics from the accumulated phase results.
    fn build_metrics(state: &SprintState, build_test_iterations: usize) -> SprintMetrics {
        SprintMetrics {
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
        }
    }

    /// Build the final SprintResult from the completed state.
    fn build_result(
        state: SprintState,
        sprint_start: std::time::Instant,
        build_test_iterations: usize,
    ) -> SprintResult {
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

        let metrics = Self::build_metrics(&state, build_test_iterations);

        SprintResult {
            success,
            phase_results: state.phase_results,
            total_duration_ms: sprint_start.elapsed().as_millis() as u64,
            summary,
            checkpoint_ref: state.checkpoint_ref.clone(),
            rollback_available: !success && state.checkpoint_ref.is_some(),
            metrics,
        }
    }

    pub async fn run(&self, config: SprintConfig) -> Result<SprintResult> {
        let mut state = SprintState::new(config);
        let phases = state.active_phases();
        let sprint_start = std::time::Instant::now();
        let mut build_test_iterations = 0usize;
        let mut idx = 0;

        while idx < phases.len() {
            let phase = &phases[idx];

            // Check sprint timeout
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

            // Create checkpoint before Build phase
            if *phase == SprintPhase::Build && state.checkpoint_ref.is_none() {
                if let Some(checkpoint) = create_checkpoint(&state.config.project_root) {
                    eprintln!("Checkpoint created: {}", checkpoint);
                    state.checkpoint_ref = Some(checkpoint);
                }
            }

            // Run phase with timeout
            let result = self.run_phase_with_timeout(&mut state, phase).await;

            // Apply tool-use loop for Build phase
            let result = self.maybe_apply_tool_use(&state, phase, result).await;

            // Apply real execution for Build/Test
            let result = self.maybe_apply_real_execution(&state, phase, result).await;

            // Record result
            if let Some(last) = state.phase_results.last_mut() {
                if last.phase == *phase {
                    *last = result.clone();
                }
            }

            // Apply multi-model review for Review phase
            let result = self.maybe_apply_review(&state, phase, result).await;

            // Handle failure
            if result.status == PhaseStatus::Failed {
                break;
            }

            // Handle build/test retry cycle
            match self.handle_test_retry(phase, &phases, &result, &mut state, &mut build_test_iterations) {
                TestRetryAction::RestartAt(new_idx) => {
                    idx = new_idx;
                    continue;
                },
                TestRetryAction::Break => break,
                TestRetryAction::Continue => {},
            }

            idx += 1;
        }

        Ok(Self::build_result(state, sprint_start, build_test_iterations))
    }

    pub async fn run_with_persistence(
        &self,
        config: SprintConfig,
        resume: bool,
    ) -> Result<SprintResult> {
        let mut state = if resume {
            match load_latest_state(&config.project_root) {
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

        // Resume from correct phase index
        let mut idx = state.phase_results.len();
        if idx > 0 {
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

            // Create checkpoint before Build phase
            if *phase == SprintPhase::Build && state.checkpoint_ref.is_none() {
                if let Some(checkpoint) = create_checkpoint(&state.config.project_root) {
                    eprintln!("Checkpoint created: {}", checkpoint);
                    state.checkpoint_ref = Some(checkpoint);
                }
            }

            // Run phase with timeout
            let result = self.run_phase_with_timeout(&mut state, phase).await;

            // Apply real execution for Build/Test
            let result = self.maybe_apply_real_execution(&state, phase, result).await;

            // Record result
            if let Some(last) = state.phase_results.last_mut() {
                if last.phase == *phase {
                    *last = result.clone();
                }
            }

            // Apply multi-model review for Review phase
            let result = self.maybe_apply_review(&state, phase, result).await;

            // Persist state after each phase
            if let Err(e) = save_state(&state) {
                tracing::warn!("Failed to save sprint state: {e}");
            }

            // Handle failure
            if result.status == PhaseStatus::Failed {
                break;
            }

            // Handle build/test retry cycle
            match self.handle_test_retry(phase, &phases, &result, &mut state, &mut build_test_iterations) {
                TestRetryAction::RestartAt(new_idx) => {
                    idx = new_idx;
                    continue;
                },
                TestRetryAction::Break => break,
                TestRetryAction::Continue => {},
            }

            idx += 1;
        }

        Ok(Self::build_result(state, sprint_start, build_test_iterations))
    }
}

/// Action to take after evaluating a test phase result.
enum TestRetryAction {
    Continue,
    RestartAt(usize),
    Break,
}
