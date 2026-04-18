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
pub mod code_parser;
pub mod error_recovery;
pub mod executor_agent;
pub mod file_ops;
pub mod generation_mode;
pub mod incremental;
pub mod llm_generator;
pub mod parallel_sprint;
pub mod planner_agent;
pub mod review_engine;
pub mod sprint;
pub mod streaming_generator;
pub mod test_execution;
pub mod tool_executor;
pub mod verifier_agent;

// Re-exports
pub use apply_workflow::{
    ApplyWorkflow, Checkpoint, CheckpointManager, TrustLevel, WorkflowResult,
};
pub use code_parser::ParsedFileChange;
pub use error_recovery::{
    CompilationError, ErrorRecovery, ErrorRecoveryConfig, ErrorRecoveryResult,
};
pub use executor_agent::{ExecutorAgent, StepResult};
pub use file_ops::{FileBackup, FileOperation, FileOperationResult, FileOperations};
pub use generation_mode::{GenerationMode, GenerationOptions, GenerationResult};
pub use llm_generator::{GeneratedCode, LlmCodeGenerator};
pub use parallel_sprint::{
    ParallelSprintConfig, ParallelSprintManager, ParallelSprintSummary, SessionState,
    SessionStatus, SprintSessionId,
};
pub use planner_agent::{
    AnalysisDepth, AnalysisScope, FileEdit, PlannerAgent, ReviewCriterion, RiskAssessment,
    StepAction, TaskPlan, TaskStep,
};
pub use review_engine::{
    FusedReview, ReviewEngine, ReviewFinding, ReviewFocus, ReviewResult, ReviewerConfig,
};
pub use sprint::{
    PhaseResult, PhaseStatus, SprintConfig, SprintEngine, SprintError, SprintMetrics, SprintPhase,
    SprintResult, SprintState,
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
