#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![allow(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://github.com/clawdius/clawdius/raw/main/docs/logo.png",
    html_favicon_url = "https://github.com/clawdius/clawdius/raw/main/docs/favicon.ico"
)]
// Clippy: allow all warnings crate-wide.
// The codebase prioritizes correctness, safety, and feature completeness over lint adherence.
// Individual modules may opt back into specific lints where beneficial.
// Deny .unwrap() in production code - use ? or .expect("invariant: ...") instead.
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::cargo)]
#![allow(clippy::nursery)]
// NOTE: Do NOT blanket-allow clippy::restriction — it overrides deny(unwrap_used).
// Restriction-group lints are suppressed individually below or in module-level allows.
#![allow(clippy::expect_used)]
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(private_interfaces)]

pub mod actions;
pub mod agentic;
pub mod agents;
#[cfg(feature = "airgap")]
#[doc(hidden)]
pub mod airgap;
pub mod analysis;
pub mod api;
#[cfg(feature = "audit")]
#[doc(hidden)]
pub mod audit;
#[cfg(feature = "billing")]
#[doc(hidden)]
pub mod billing;
pub mod capability;
pub mod checkpoint;
pub mod commands;
pub mod completions;
#[cfg(feature = "compliance")]
#[doc(hidden)]
pub mod compliance;
pub mod config;
pub mod context;
pub mod diff;
pub mod distributed;
pub mod encryption;
pub mod error;
pub mod graph_rag;
#[cfg(feature = "i18n")]
#[doc(hidden)]
pub mod i18n;
pub mod integrity;
#[cfg(feature = "billing")]
#[doc(hidden)]
pub mod invoice;
pub mod llm;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod metrics;
pub mod modes;
pub mod multimodal;
#[cfg(feature = "onboarding")]
#[doc(hidden)]
pub mod onboarding;
pub mod orchestrator;
pub mod output;
#[cfg(feature = "proof")]
#[doc(hidden)]
pub mod proof;
pub mod retry;
#[cfg(feature = "rpc")]
#[doc(hidden)]
pub mod rpc;
#[doc(hidden)]
pub mod sandbox;
pub mod session;
pub mod simd;
pub mod skills;
pub mod storage;
#[doc(hidden)]
pub mod telemetry;
pub mod timeline;
pub mod timeout;
pub mod tokenize;
pub mod tokenizer;
pub mod tools;
#[cfg(feature = "billing")]
#[doc(hidden)]
pub mod usage;
#[cfg(feature = "watch")]
#[doc(hidden)]
pub mod watch;
#[cfg(feature = "webhooks")]
#[doc(hidden)]
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
#[cfg(feature = "onboarding")]
pub use onboarding::{Onboarding, OnboardingStatus};
pub use output::OutputFormat;
#[cfg(feature = "proof")]
pub use proof::{LeanVerifier, ProofDefinition, ProofTemplate};
pub use retry::{with_retry_and_circuit, CircuitBreaker, CircuitState};
pub use session::{Session, SessionManager, SessionStore};
pub use skills::{Skill, SkillContext, SkillError, SkillMeta, SkillRegistry, SkillResult};
pub use storage::{
    GraphRepository, InMemoryBackend, SessionRepository as StorageSessionRepository, SqliteBackend,
    StorageBackend, TimelineRepository,
};
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
