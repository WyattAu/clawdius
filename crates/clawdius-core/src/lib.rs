#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![allow(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://github.com/clawdius/clawdius/raw/main/docs/logo.png",
    html_favicon_url = "https://github.com/clawdius/clawdius/raw/main/docs/favicon.ico"
)]

pub mod actions;
pub mod agentic;
pub mod agents;
pub mod analysis;
pub mod api;
pub mod audit;
pub mod capability;
pub mod checkpoint;
pub mod commands;
pub mod completions;
pub mod config;
pub mod context;
pub mod diff;
pub mod error;
pub mod graph_rag;
pub mod i18n;
pub mod llm;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod modes;
pub mod orchestrator;
pub mod onboarding;
pub mod output;
pub mod proof;
pub mod retry;
pub mod rpc;
pub mod sandbox;
pub mod session;
pub mod simd;
pub mod skills;
pub mod telemetry;
pub mod timeline;
pub mod timeout;
pub mod tokenize;
pub mod tools;
pub mod watch;
pub mod webhooks;
pub mod workspace;

// Re-exports for convenience
pub use agents::{
    AgentError, AgentMessage, AgentRole, AgentStatus, AgentTeam, TeamConfig, TeamResult,
};
pub use api::{ApiConfig, ApiGateway, ChatRequest, ChatResponse, HealthResponse};
pub use config::Config;
#[cfg(feature = "vector-db")]
pub use context::{AggregatedContext, ContextAggregator};
pub use context::{
    CompactResult, Context, ContextCompactor, ContextCompactorConfig, ContextItem,
    ContextWindowManager, FileInfo, Mention, MentionResolver, ProviderTokenLimits,
};
pub use diff::{DiffPreview, DiffRenderer, DiffStats, DiffTheme, FileDiff};
pub use error::{EnhancedError, Error, ErrorHelpers, Result};
pub use memory::{MemoryEntry, MemoryMetadata, ProjectMemory};
pub use onboarding::{Onboarding, OnboardingStatus};
pub use output::OutputFormat;
pub use proof::{LeanVerifier, ProofDefinition, ProofTemplate};
pub use retry::{with_retry_and_circuit, CircuitBreaker, CircuitState};
pub use session::{Session, SessionManager, SessionStore};
pub use skills::{Skill, SkillContext, SkillError, SkillMeta, SkillRegistry, SkillResult};
pub use telemetry::{CrashReporter, TelemetryConfig};
pub use timeline::{CheckpointId, TimelineManager};
#[cfg(feature = "vector-db")]
pub use workspace::{IndexStats, WorkspaceIndexer};

// Agentic module re-exports
pub use agentic::{
    AgenticState, AgenticSystem, ApplyWorkflow, ChangeType, FileChange, GenerationMode,
    GenerationOptions, GenerationResult, LogEntry, LogLevel, TaskContext, TaskRequest, TaskResult,
    TestExecutionStrategy, TestResult as AgenticTestResult, TrustLevel, WorkflowResult,
};
pub use agentic::{ExecutorAgent, StepResult};
pub use agentic::{
    IssueSeverity, VerificationIssue, VerificationResult as AgenticVerificationResult,
    VerifierAgent,
};
pub use agentic::{PlannerAgent, RiskAssessment, StepAction, TaskPlan, TaskStep};

// Analysis module re-exports
pub use analysis::{
    AnalysisError, AnalysisResult, ArchitectureDrift, DebtAnalyzer, DebtItem, DebtReport, DebtRule,
    DebtType, DriftCategory, DriftDetector, DriftReport, DriftRule, DriftSeverity,
};

/// Current version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name
pub const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");
