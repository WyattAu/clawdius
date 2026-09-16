// Unwrap purge batch 1: execution-surface module — production code must not
// unwrap/expect; propagate, use invariant-expect with a written INVARIANT
// argument, or restructure.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use super::{
    create_checkpoint, get_changed_files, load_latest_state, run_multi_model_review, save_state,
    PhaseResult, PhaseStatus, SprintConfig, SprintError, SprintMetrics, SprintPhase, SprintResult,
    SprintState,
};
use crate::agentic::browser_daemon::BrowserDaemon;
use crate::agentic::tool_executor::{ToolExecutor, ToolRequest};
use crate::agentic::tool_use;
use crate::llm::model_router::AgentHook;
use crate::llm::providers::LlmClient;
use crate::Result;
use std::path::Path;
use std::sync::Arc;

pub struct SprintEngine {
    pub(crate) llm: Arc<dyn LlmClient>,
    pub(crate) tool_executor: Option<Arc<dyn ToolExecutor>>,
    pub(crate) browser_daemon: Option<Arc<BrowserDaemon>>,
    pub(crate) lsp_client: Option<Arc<tokio::sync::Mutex<crate::lsp::LspClient>>>,
    /// Hooks for intercepting tool calls.
    pub(crate) hooks: Vec<Arc<dyn AgentHook>>,
}

impl SprintEngine {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self {
            llm,
            tool_executor: None,
            browser_daemon: None,
            lsp_client: None,
            hooks: Vec::new(),
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

    /// Add a hook for intercepting tool calls.
    #[must_use]
    pub fn with_hook(mut self, hook: Arc<dyn AgentHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Execute a tool request with hook interception.
    pub async fn execute_tool_with_hooks(
        &self,
        request: ToolRequest,
    ) -> Result<crate::tools::ToolResult> {
        // Run before hooks
        for hook in &self.hooks {
            if !hook
                .before_tool_call(
                    &request.name,
                    &serde_json::to_string(&request.arguments).unwrap_or_default(),
                )
                .await
            {
                return Ok(crate::tools::ToolResult::error("Tool call blocked by hook"));
            }
        }

        // Execute the tool
        let result = if let Some(executor) = &self.tool_executor {
            executor.execute(request.clone()).await?
        } else {
            crate::tools::ToolResult::error("No tool executor configured")
        };

        // Run after hooks
        for hook in &self.hooks {
            hook.after_tool_call(&request.name, &result.content).await;
        }

        Ok(result)
    }

    /// Get browser accessibility snapshot if browser daemon is available.
    pub async fn get_browser_snapshot(&self) -> Option<String> {
        let daemon = self.browser_daemon.as_ref()?;
        match daemon.build_snapshot("sprint").await {
            Ok(snapshot) => Some(snapshot.to_ref_list()),
            Err(e) => {
                tracing::warn!("Failed to get browser snapshot: {e}");
                None
            },
        }
    }

    /// Navigate browser to a URL if browser daemon is available.
    pub async fn navigate_browser(&self, url: &str) -> Result<bool> {
        if let Some(daemon) = &self.browser_daemon {
            match daemon.navigate(url, None).await {
                Ok(_) => Ok(true),
                Err(e) => {
                    tracing::warn!("Browser navigation failed: {e}");
                    Ok(false)
                },
            }
        } else {
            Ok(false)
        }
    }

    pub(crate) async fn chat_collecting_stream(
        &self,
        messages: Vec<crate::llm::ChatMessage>,
    ) -> crate::Result<String> {
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
                tracing::debug!("streaming done");
                if output.is_empty() {
                    Err(crate::Error::Llm("LLM returned empty response".to_string()))
                } else {
                    Ok(output)
                }
            },
            Ok(Err(_)) => {
                let opts = crate::llm::LlmChatOptions::from_mode_and_config(0.7, 4096);
                self.llm
                    .chat_with_options(messages, opts)
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
                tracing::debug!("Phase {} error (will be retried or reported): {e}", phase);
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
                tracing::debug!(
                    "Phase {} timed out after {}s",
                    phase,
                    state.config.phase_timeout_secs
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
        if *phase != SprintPhase::Build || result.status != PhaseStatus::Success {
            return result;
        }

        // Fallible-site fix (unwrap purge batch 1): this used to be
        // `.expect("guarded by is_some() check above")`. The guard is now
        // expressed structurally — a Build/Success phase with no tool
        // executor passes the result through unchanged.
        let Some(executor) = self.tool_executor.as_ref() else {
            return result;
        };
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

        tracing::debug!("  [tool-use loop starting for Build phase (trying native first)]");

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
                tracing::debug!(
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
                tracing::debug!(
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
                        tracing::debug!(
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
                        tracing::debug!(
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
                tracing::debug!("Real execution error in phase {phase}: {e}");
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
                tracing::debug!(
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
                tracing::debug!(
                    "Sprint timeout: {}s elapsed, max {}s. Stopping.",
                    elapsed,
                    state.config.max_duration_secs
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
                    tracing::debug!("Checkpoint created: {}", checkpoint);
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
            match self.handle_test_retry(
                phase,
                &phases,
                &result,
                &mut state,
                &mut build_test_iterations,
            ) {
                TestRetryAction::RestartAt(new_idx) => {
                    idx = new_idx;
                    continue;
                },
                TestRetryAction::Break => break,
                TestRetryAction::Continue => {},
            }

            idx += 1;
        }

        Ok(Self::build_result(
            state,
            sprint_start,
            build_test_iterations,
        ))
    }

    pub async fn run_with_persistence(
        &self,
        config: SprintConfig,
        resume: bool,
    ) -> Result<SprintResult> {
        let mut state = if resume {
            match load_latest_state(&config.project_root) {
                Ok(Some(s)) => {
                    tracing::debug!(
                        "Resuming sprint from {} ({} phases already completed)",
                        s.started_at.format("%Y-%m-%d %H:%M:%S"),
                        s.phase_results.len()
                    );
                    s
                },
                Ok(None) => {
                    tracing::debug!("No saved sprint state found, starting fresh");
                    SprintState::new(config)
                },
                Err(e) => {
                    tracing::debug!("Failed to load sprint state: {e}, starting fresh");
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
        tracing::debug!(
            "Starting from phase index {idx} ({})",
            phases.get(idx).map_or("end", |p| p.display_name())
        );

        while idx < phases.len() {
            let phase = &phases[idx];

            // Create checkpoint before Build phase
            if *phase == SprintPhase::Build && state.checkpoint_ref.is_none() {
                if let Some(checkpoint) = create_checkpoint(&state.config.project_root) {
                    tracing::debug!("Checkpoint created: {}", checkpoint);
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
            match self.handle_test_retry(
                phase,
                &phases,
                &result,
                &mut state,
                &mut build_test_iterations,
            ) {
                TestRetryAction::RestartAt(new_idx) => {
                    idx = new_idx;
                    continue;
                },
                TestRetryAction::Break => break,
                TestRetryAction::Continue => {},
            }

            idx += 1;
        }

        Ok(Self::build_result(
            state,
            sprint_start,
            build_test_iterations,
        ))
    }
}

/// Action to take after evaluating a test phase result.
enum TestRetryAction {
    Continue,
    RestartAt(usize),
    Break,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tool_executor::NoOpToolExecutor;
    use crate::llm::providers::LlmClient;
    use crate::llm::ChatMessage;
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    /// LLM stub for guard-path tests: if a code change ever routes these
    /// passthrough scenarios into the tool-use loop, the error makes it loud.
    struct UnusedLlm;

    #[async_trait]
    impl LlmClient for UnusedLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> crate::Result<String> {
            Err(crate::Error::Llm(
                "guard tests must not reach the LLM".to_string(),
            ))
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> crate::Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            drop(tx);
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    fn passthrough_result(phase: SprintPhase, status: PhaseStatus) -> PhaseResult {
        PhaseResult {
            phase,
            status,
            output: "untouched output".to_string(),
            duration_ms: 1,
            files_modified: vec!["src/lib.rs".to_string()],
            errors: Vec::new(),
            tokens_used: 7,
        }
    }

    /// Unwrap-purge batch 1 regression: a Build/Success phase with NO tool
    /// executor passes the result through unchanged. This is the path that
    /// used to rely on `.expect("guarded by is_some() check above")`.
    #[tokio::test]
    async fn build_success_without_tool_executor_is_passthrough() {
        let engine = SprintEngine::new(Arc::new(UnusedLlm));
        let state = SprintState::new(SprintConfig::new("task"));
        let input = passthrough_result(SprintPhase::Build, PhaseStatus::Success);

        let out = engine
            .maybe_apply_tool_use(&state, &SprintPhase::Build, input.clone())
            .await;

        assert_eq!(out.phase, input.phase);
        assert_eq!(out.status, PhaseStatus::Success);
        assert_eq!(out.output, "untouched output");
        assert_eq!(out.files_modified, input.files_modified);
        assert_eq!(out.tokens_used, 7);
        assert!(out.errors.is_empty());
    }

    /// The tool-use loop only applies to the Build phase: other phases return
    /// the result untouched even when a tool executor is configured.
    #[tokio::test]
    async fn non_build_phase_with_tool_executor_is_passthrough() {
        let engine =
            SprintEngine::new(Arc::new(UnusedLlm)).with_tool_executor(Arc::new(NoOpToolExecutor));
        let state = SprintState::new(SprintConfig::new("task"));
        let input = passthrough_result(SprintPhase::Plan, PhaseStatus::Success);

        let out = engine
            .maybe_apply_tool_use(&state, &SprintPhase::Plan, input)
            .await;

        assert_eq!(out.status, PhaseStatus::Success);
        assert_eq!(out.output, "untouched output");
        assert!(out.errors.is_empty());
    }

    /// Failed Build phases never enter the tool-use loop, executor or not.
    #[tokio::test]
    async fn build_failure_with_tool_executor_is_passthrough() {
        let engine =
            SprintEngine::new(Arc::new(UnusedLlm)).with_tool_executor(Arc::new(NoOpToolExecutor));
        let state = SprintState::new(SprintConfig::new("task"));
        let input = passthrough_result(SprintPhase::Build, PhaseStatus::Failed);

        let out = engine
            .maybe_apply_tool_use(&state, &SprintPhase::Build, input)
            .await;

        assert_eq!(out.status, PhaseStatus::Failed);
        assert_eq!(out.output, "untouched output");
        assert!(out.errors.is_empty());
    }
}
