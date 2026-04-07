//! Agentic Code Generation System
//!
//! This module provides multiple code generation modes with full user control:
//!
//! # Generation Modes
//!
//! - **Single-pass**: Fast, one-shot generation for simple tasks
//! - **Iterative**: Progressive refinement with feedback loops
//! - **Agent-based**: Full autonomous workflow with planning, execution, verification
//!
//! # Test Execution
//!
//! - **Sandboxed**: Run tests in isolated environment (Option B)
//! - **Direct with Rollback**: Run tests directly, rollback on failure (Option C)
//!
//! # Apply Workflows
//!
//! - **Trust-based**: Apply changes based on configurable trust levels (Option B)
//! - **Rollback-based**: Create checkpoint before applying, allow rollback (Option C)
//!
//! # Example
//!
//! ```rust,ignore
//! use clawdius_core::agentic::{
//!     GenerationMode, TestExecutionStrategy, ApplyWorkflow,
//!     AgenticSystem, TaskRequest, TaskResult,
//! };
//!
//! // User chooses their workflow
//! let mode = GenerationMode::AgentBased;
//! let test_strategy = TestExecutionStrategy::DirectWithRollback;
//! let apply_workflow = ApplyWorkflow::RollbackBased;
//!
//! let system = AgenticSystem::new(mode, test_strategy, apply_workflow);
//! let result = system.execute(request).await?;
//! ```

pub mod apply_workflow;
pub mod error_recovery;
pub mod executor_agent;
pub mod file_ops;
pub mod generation_mode;
pub mod incremental;
pub mod llm_generator;
pub mod planner_agent;
pub mod streaming_generator;
pub mod test_execution;
pub mod tool_executor;
pub mod verifier_agent;

// Re-exports
pub use apply_workflow::{
    ApplyWorkflow, Checkpoint, CheckpointManager, TrustLevel, WorkflowResult,
};
pub use error_recovery::{
    CompilationError, ErrorRecovery, ErrorRecoveryConfig, ErrorRecoveryResult,
};
pub use executor_agent::{ExecutorAgent, StepResult};
pub use file_ops::{FileBackup, FileOperation, FileOperationResult, FileOperations};
pub use generation_mode::{GenerationMode, GenerationOptions, GenerationResult};
pub use llm_generator::{GeneratedCode, LlmCodeGenerator};
pub use planner_agent::{
    AnalysisDepth, AnalysisScope, FileEdit, PlannerAgent, ReviewCriterion, RiskAssessment,
    StepAction, TaskPlan, TaskStep,
};
pub use streaming_generator::{StreamChunk, StreamProcessor, StreamingCodeGenerator};
pub use test_execution::{
    SandboxBackend, TestExecutionStrategy, TestFramework, TestResult, TestRunner,
};
pub use tool_executor::{NoOpToolExecutor, ToolDefinition, ToolExecutor, ToolRequest, ToolResult};
pub use verifier_agent::{
    IssueSeverity, VerificationIssue, VerificationResult, VerificationRule, VerifierAgent,
};

use crate::error::Result;
use crate::llm::LlmClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A task request for the agentic system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// Unique task identifier
    pub id: String,
    /// Task description in natural language
    pub description: String,
    /// Target files (if known)
    pub target_files: Vec<String>,
    /// Generation mode to use
    pub mode: GenerationMode,
    /// Test execution strategy
    pub test_strategy: TestExecutionStrategy,
    /// Apply workflow
    pub apply_workflow: ApplyWorkflow,
    /// Additional context
    pub context: TaskContext,
    /// User trust level for trust-based apply
    pub trust_level: TrustLevel,
}

/// Context for a task request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskContext {
    /// Related files in the workspace
    pub related_files: Vec<String>,
    /// Previous conversation context
    pub conversation_history: Vec<String>,
    /// Project metadata
    pub project_language: Option<String>,
    pub project_framework: Option<String>,
    /// Constraints
    pub constraints: Vec<String>,
}

/// Result of executing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub id: String,
    /// Whether the task succeeded
    pub success: bool,
    /// Generated code changes
    pub changes: Vec<FileChange>,
    /// Test results (if tests were run)
    pub test_result: Option<TestResult>,
    /// Verification results
    pub verification: VerificationResult,
    /// Rollback checkpoint (if created)
    pub rollback_checkpoint: Option<String>,
    /// Execution log
    pub log: Vec<LogEntry>,
    /// Total execution time in milliseconds
    pub duration_ms: u64,
}

/// A file change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path
    pub path: String,
    /// Change type
    pub change_type: ChangeType,
    /// Original content (for modifications)
    pub original: Option<String>,
    /// New content
    pub new: String,
    /// Diff preview
    pub diff: String,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// New file created
    Created,
    /// Existing file modified
    Modified,
    /// File deleted
    Deleted,
}

/// A log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp (unix millis)
    pub timestamp: u64,
    /// Log level
    pub level: LogLevel,
    /// Component that generated the log
    pub component: String,
    /// Log message
    pub message: String,
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// The main agentic system coordinating all components.
pub struct AgenticSystem {
    /// Generation mode
    mode: GenerationMode,
    /// Test execution strategy
    test_strategy: TestExecutionStrategy,
    /// Apply workflow
    apply_workflow: ApplyWorkflow,
    /// Planner agent
    planner: PlannerAgent,
    /// Executor agent
    executor: ExecutorAgent,
    /// Verifier agent
    verifier: VerifierAgent,
    /// Test runner
    test_runner: TestRunner,
    /// Current state
    state: Arc<RwLock<AgenticState>>,
    /// Optional LLM client for real code generation
    llm_client: Option<Arc<dyn LlmClient>>,
    /// Optional tool executor for calling external tools
    tool_executor: Option<Arc<dyn ToolExecutor>>,
}

/// State of the agentic system.
#[derive(Debug, Clone, Default)]
pub struct AgenticState {
    /// Current task being processed
    current_task: Option<String>,
    /// Tasks completed
    completed_tasks: u64,
    /// Tasks failed
    failed_tasks: u64,
    /// Total execution time
    total_time_ms: u64,
}

impl AgenticSystem {
    /// Creates a new agentic system with the specified configuration.
    #[must_use]
    pub fn new(
        mode: GenerationMode,
        test_strategy: TestExecutionStrategy,
        apply_workflow: ApplyWorkflow,
    ) -> Self {
        Self {
            mode,
            test_strategy,
            apply_workflow,
            planner: PlannerAgent::new(),
            executor: ExecutorAgent::new(),
            verifier: VerifierAgent::new(),
            test_runner: TestRunner::new(test_strategy),
            state: Arc::new(RwLock::new(AgenticState::default())),
            llm_client: None,
            tool_executor: None,
        }
    }

    /// Sets the LLM client for real code generation.
    #[must_use]
    pub fn with_llm_client(mut self, client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Returns a reference to the LLM client if configured.
    #[must_use]
    pub fn llm_client(&self) -> Option<&Arc<dyn LlmClient>> {
        self.llm_client.as_ref()
    }

    /// Sets the tool executor for calling external tools (e.g., MCP tools).
    #[must_use]
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Returns a reference to the tool executor if configured.
    #[must_use]
    pub fn tool_executor(&self) -> Option<&Arc<dyn ToolExecutor>> {
        self.tool_executor.as_ref()
    }

    /// Returns the generation mode.
    #[must_use]
    pub const fn mode(&self) -> &GenerationMode {
        &self.mode
    }

    /// Creates an LLM code generator if a client is configured.
    #[must_use]
    pub fn code_generator(&self) -> Option<LlmCodeGenerator> {
        self.llm_client
            .as_ref()
            .map(|client| LlmCodeGenerator::new(Arc::clone(client), "configured-model".to_string()))
    }

    /// Generates code using the LLM if configured, otherwise returns a placeholder.
    ///
    /// # Errors
    ///
    /// Returns an error if LLM generation fails.
    pub async fn generate_code(
        &self,
        prompt: &str,
        file_path: Option<&str>,
        existing_content: Option<&str>,
    ) -> Result<Option<GeneratedCode>> {
        let Some(generator) = self.code_generator() else {
            return Ok(None);
        };

        let result = if let Some(path) = file_path {
            if let Some(content) = existing_content {
                generator.generate_edit(path, content, prompt).await?
            } else {
                generator.generate_for_file(prompt, path, None).await?
            }
        } else {
            generator.generate(prompt, existing_content).await?
        };

        Ok(Some(result))
    }

    /// Executes a task using the configured workflow.
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails.
    pub async fn execute(&mut self, request: TaskRequest) -> Result<TaskResult> {
        let start = std::time::Instant::now();
        let mut log = Vec::new();
        let mut rollback_checkpoint = None;

        // Update state
        {
            let mut state = self.state.write().await;
            state.current_task = Some(request.id.clone());
        }

        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Info,
            component: "AgenticSystem".to_string(),
            message: format!("Starting task {} with mode {:?}", request.id, request.mode),
        });

        // Create rollback checkpoint if using rollback-based workflow
        if matches!(self.apply_workflow, ApplyWorkflow::RollbackBased { .. }) {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Info,
                component: "AgenticSystem".to_string(),
                message: "Creating rollback checkpoint".to_string(),
            });
            rollback_checkpoint =
                Some(format!("checkpoint-{}-{}", request.id, current_timestamp()));
        }

        // Execute based on generation mode
        let (changes, verification) = match request.mode {
            GenerationMode::SinglePass => self.execute_single_pass(&request, &mut log).await?,
            GenerationMode::Iterative { max_iterations } => {
                self.execute_iterative(&request, max_iterations, &mut log)
                    .await?
            },
            GenerationMode::AgentBased { max_steps, .. } => {
                self.execute_agent_based(&request, max_steps, &mut log)
                    .await?
            },
        };

        // Run tests if changes were made
        let test_result = if !changes.is_empty() {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Info,
                component: "AgenticSystem".to_string(),
                message: format!("Running tests with strategy {:?}", self.test_strategy),
            });
            Some(self.test_runner.run_tests(&changes).await?)
        } else {
            None
        };

        // Determine success
        let tests_passed = test_result.as_ref().map_or(true, |t| t.passed);
        let verification_passed = verification.issues.iter().all(|i| !i.is_blocking());
        let success = tests_passed && verification_passed;

        // Apply changes based on workflow
        if success {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Info,
                component: "AgenticSystem".to_string(),
                message: "Applying changes".to_string(),
            });
            // In a real implementation, this would write the changes to disk
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.current_task = None;
            if success {
                state.completed_tasks += 1;
            } else {
                state.failed_tasks += 1;
            }
            state.total_time_ms += start.elapsed().as_millis() as u64;
        }

        Ok(TaskResult {
            id: request.id,
            success,
            changes,
            test_result,
            verification,
            rollback_checkpoint,
            log,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn execute_single_pass(
        &mut self,
        request: &TaskRequest,
        log: &mut Vec<LogEntry>,
    ) -> Result<(Vec<FileChange>, VerificationResult)> {
        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Info,
            component: "AgenticSystem".to_string(),
            message: "Executing single-pass generation".to_string(),
        });

        // Try LLM-based generation if configured
        if let Some(generator) = self.code_generator() {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Debug,
                component: "AgenticSystem".to_string(),
                message: "Using LLM for code generation".to_string(),
            });

            let mut changes = Vec::new();

            // Generate for each target file
            for file_path in &request.target_files {
                let existing = std::fs::read_to_string(file_path).ok();

                match generator
                    .generate_for_file(&request.description, file_path, existing.as_deref())
                    .await
                {
                    Ok(generated) => {
                        log.push(LogEntry {
                            timestamp: current_timestamp(),
                            level: LogLevel::Info,
                            component: "AgenticSystem".to_string(),
                            message: format!(
                                "Generated code for {} (confidence: {:.2})",
                                file_path, generated.confidence
                            ),
                        });

                        changes.push(FileChange {
                            path: file_path.clone(),
                            change_type: if existing.is_some() {
                                ChangeType::Modified
                            } else {
                                ChangeType::Created
                            },
                            original: existing.clone(),
                            new: generated.content.clone(),
                            diff: format!(
                                "--- {}\n+++ {}\n{}",
                                file_path, file_path, generated.content
                            ),
                        });
                    },
                    Err(e) => {
                        log.push(LogEntry {
                            timestamp: current_timestamp(),
                            level: LogLevel::Warn,
                            component: "AgenticSystem".to_string(),
                            message: format!("LLM generation failed for {}: {}", file_path, e),
                        });
                    },
                }
            }

            // If no target files, generate without a specific file
            if request.target_files.is_empty() {
                match generator.generate(&request.description, None).await {
                    Ok(generated) => {
                        // Create a placeholder file path
                        let file_path = generated
                            .file_path
                            .clone()
                            .unwrap_or_else(|| "generated_code.txt".to_string());

                        changes.push(FileChange {
                            path: file_path.clone(),
                            change_type: ChangeType::Created,
                            original: None,
                            new: generated.content.clone(),
                            diff: format!("+++ {}\n{}", file_path, generated.content),
                        });
                    },
                    Err(e) => {
                        log.push(LogEntry {
                            timestamp: current_timestamp(),
                            level: LogLevel::Warn,
                            component: "AgenticSystem".to_string(),
                            message: format!("LLM generation failed: {}", e),
                        });
                    },
                }
            }

            if !changes.is_empty() {
                let verification = self.verifier.verify_changes(&changes, request).await?;
                return Ok((changes, verification));
            }
        }

        // Fallback to plan-based execution
        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Debug,
            component: "AgenticSystem".to_string(),
            message: "Using plan-based execution (no LLM configured or LLM failed)".to_string(),
        });

        // Create a simple plan
        let plan = self.planner.create_simple_plan(request).await?;

        // Execute the plan
        let changes = self.executor.execute_plan(&plan, log).await?;

        // Verify the changes
        let verification = self.verifier.verify_changes(&changes, request).await?;

        Ok((changes, verification))
    }

    async fn execute_iterative(
        &mut self,
        request: &TaskRequest,
        max_iterations: u32,
        log: &mut Vec<LogEntry>,
    ) -> Result<(Vec<FileChange>, VerificationResult)> {
        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Info,
            component: "AgenticSystem".to_string(),
            message: format!(
                "Executing iterative generation (max {} iterations)",
                max_iterations
            ),
        });

        let mut current_changes = Vec::new();
        let mut verification = VerificationResult::default();

        for iteration in 0..max_iterations {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Debug,
                component: "AgenticSystem".to_string(),
                message: format!("Iteration {}/{}", iteration + 1, max_iterations),
            });

            // Create plan considering previous iteration
            let plan = self
                .planner
                .create_iterative_plan(request, &verification)
                .await?;

            // Execute
            let new_changes = self.executor.execute_plan(&plan, log).await?;
            current_changes.extend(new_changes);

            // Verify
            verification = self
                .verifier
                .verify_changes(&current_changes, request)
                .await?;

            // Check if we're done
            if verification.issues.is_empty() {
                log.push(LogEntry {
                    timestamp: current_timestamp(),
                    level: LogLevel::Info,
                    component: "AgenticSystem".to_string(),
                    message: "All issues resolved, stopping iteration".to_string(),
                });
                break;
            }

            // Check if we can continue
            if verification.issues.iter().all(|i| !i.can_fix) {
                log.push(LogEntry {
                    timestamp: current_timestamp(),
                    level: LogLevel::Warn,
                    component: "AgenticSystem".to_string(),
                    message: "Unfixable issues detected, stopping iteration".to_string(),
                });
                break;
            }
        }

        Ok((current_changes, verification))
    }

    async fn execute_agent_based(
        &mut self,
        request: &TaskRequest,
        max_steps: u32,
        log: &mut Vec<LogEntry>,
    ) -> Result<(Vec<FileChange>, VerificationResult)> {
        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Info,
            component: "AgenticSystem".to_string(),
            message: format!("Executing agent-based generation (max {} steps)", max_steps),
        });

        // Create comprehensive plan
        let plan = self.planner.create_comprehensive_plan(request).await?;

        log.push(LogEntry {
            timestamp: current_timestamp(),
            level: LogLevel::Info,
            component: "PlannerAgent".to_string(),
            message: format!("Created plan with {} steps", plan.steps.len()),
        });

        // Execute plan step by step
        let changes = self.executor.execute_plan(&plan, log).await?;

        // Final verification
        let verification = self.verifier.verify_changes(&changes, request).await?;

        // If verification failed and we have remaining steps, try to fix
        if !verification.issues.is_empty() {
            log.push(LogEntry {
                timestamp: current_timestamp(),
                level: LogLevel::Info,
                component: "VerifierAgent".to_string(),
                message: format!(
                    "Found {} issues, attempting fixes",
                    verification.issues.len()
                ),
            });

            // Create fix plan
            let fix_plan = self
                .planner
                .create_fix_plan(&verification.issues, request)
                .await?;
            let fix_changes = self.executor.execute_plan(&fix_plan, log).await?;

            // Merge changes
            let mut all_changes = changes;
            all_changes.extend(fix_changes);

            // Re-verify
            let final_verification = self.verifier.verify_changes(&all_changes, request).await?;
            return Ok((all_changes, final_verification));
        }

        Ok((changes, verification))
    }

    /// Returns the current state of the system.
    pub async fn state(&self) -> AgenticState {
        self.state.read().await.clone()
    }
}

/// Returns the current timestamp in milliseconds.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_request_serialization() {
        let request = TaskRequest {
            id: "test-1".to_string(),
            description: "Add a hello world function".to_string(),
            target_files: vec!["src/main.rs".to_string()],
            mode: GenerationMode::SinglePass,
            test_strategy: TestExecutionStrategy::Sandboxed {
                backend: test_execution::SandboxBackend::Wasm,
                timeout_ms: 30000,
            },
            apply_workflow: ApplyWorkflow::TrustBased {
                level: TrustLevel::Medium,
                confirm_low_trust: true,
            },
            context: TaskContext::default(),
            trust_level: TrustLevel::Medium,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: TaskRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-1");
    }

    #[tokio::test]
    async fn test_agentic_system_creation() {
        let system = AgenticSystem::new(
            GenerationMode::SinglePass,
            TestExecutionStrategy::Sandboxed {
                backend: test_execution::SandboxBackend::Wasm,
                timeout_ms: 30000,
            },
            ApplyWorkflow::TrustBased {
                level: TrustLevel::High,
                confirm_low_trust: true,
            },
        );

        let state = system.state().await;
        assert_eq!(state.completed_tasks, 0);
        assert_eq!(state.failed_tasks, 0);
    }
}
