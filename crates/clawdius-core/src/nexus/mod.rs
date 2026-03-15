//! Nexus FSM - 24-phase R&D Lifecycle Engine
//!
//! This module implements a Finite State Machine (FSM) that enforces formal development
//! practices through compile-time safety using the Typestate pattern. The FSM ensures
//! deterministic, traceable, and verifiable software development by enforcing phase
//! transitions through quality gates and artifact tracking.
//!
//! # Architecture
//!
//! The Nexus FSM consists of:
//! - **Phase State Machine**: 24 discrete phases using Typestate pattern
//! - **Transition Engine**: Validation and execution of phase transitions
//! - **Quality Gate Evaluator**: Pass/fail checks at each phase
//! - **Artifact Tracker**: In-memory artifact storage with dependency tracking
//! - **Event Bus**: Async event dispatch for notifications
//!
//! # The 24 Phases
//!
//! | Phase | Name | Category |
//! |-------|------|----------|
//! | 0 | Context Discovery | Discovery |
//! | 1 | Environment Materialization | Discovery |
//! | 2 | Requirements Engineering | Discovery |
//! | 3 | Epistemological Discovery (Yellow) | Requirements |
//! | 4 | Cross-Lingual Integration | Requirements |
//! | 5 | Supply Chain Hardening | Requirements |
//! | 6 | Architectural Specification (Blue) | Architecture |
//! | 7 | Concurrency Analysis | Architecture |
//! | 8 | Security Engineering (Red) | Architecture |
//! | 9 | Resource Management | Architecture |
//! | 10 | Performance Engineering (Green) | Planning |
//! | 11 | Cross-Platform Compatibility | Planning |
//! | 12 | Adversarial Loop | Planning |
//! | 13 | CI/CD Engineering | Implementation |
//! | 14 | Documentation Verification | Implementation |
//! | 15 | Knowledge Base Update | Implementation |
//! | 16 | Execution Graph Generation | Verification |
//! | 17 | Supply Chain Monitoring | Verification |
//! | 18 | Deployment & Operations | Verification |
//! | 19 | Operations | Verification |
//! | 20 | Project Closure | Validation |
//! | 21 | Continuous Monitoring | Validation |
//! | 22 | Knowledge Transfer | Transition |
//! | 23 | Archive | Transition |
//!
//! # Example
//!
//! ```rust,ignore
//! use clawdius_core::nexus::{NexusEngine, Phase0ContextDiscovery};
//!
//! // Create engine in Phase 0
//! let engine = NexusEngine::new(project_root)?;
//!
//! // Transition through phases with typestate safety
//! let engine = engine
//!     .transition_to_environment("my_domain", vec!["ISO9001"])?
//!     .transition_to_requirements("cargo", vec![], true)?;
//! // ... continue through all 24 phases
//!
//! // Finalize the project
//! let finalized = engine.finalize()?;
//! ```

pub mod artifacts;
pub mod config;
pub mod engine;
pub mod event_bus;
pub mod events;
pub mod gates;
pub mod metrics;
pub mod phases;
pub mod recovery;
pub mod transition;

pub mod advanced_gates;
pub mod event_sourcing;
pub mod persistence;
pub mod workflow;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use artifacts::{
    Artifact, ArtifactId, ArtifactMetadata, ArtifactQuery, ArtifactTracker, ArtifactType,
};
pub use engine::{FinalizedProject, InterfaceData, NexusEngine, RequirementData};
pub use event_bus::EventBus as SimpleEventBus;
pub use events::{AuditHandler, LoggingHandler, MetricsHandler};
pub use events::{EventBus, EventHandler, EventHandlerError, EventType, NexusEvent};
pub use gates::{default_gates, GateContext, GateEvaluator, GateResult, GateSeverity, QualityGate};
pub use gates::{
    BluePaperGate, CompilationGate, DeploymentReadinessGate, DocumentationGate,
    DomainIdentifiedGate, EnvironmentReproducibleGate, RequirementsCompleteGate, SecurityScanGate,
    StandardsMappedGate, TestCoverageGate, YellowPaperGate,
};
pub use phases::{all_phases, get_phase_by_id, PhaseCategory, PhaseId, PhaseState};
pub use phases::{
    Phase0ContextDiscovery, Phase10PerformanceEngineering, Phase11CrossPlatformCompatibility,
    Phase12AdversarialLoop, Phase13CICDEngineering, Phase14Documentation, Phase15KnowledgeBase,
    Phase16ExecutionGraph, Phase17SupplyMonitoring, Phase18Deployment, Phase19Operations,
    Phase1EnvironmentMaterialization, Phase20Closure, Phase21ContinuousMonitoring,
    Phase22KnowledgeTransfer, Phase23Archive, Phase2RequirementsEngineering,
    Phase3EpistemologicalDiscovery, Phase4CrossLingualIntegration, Phase5SupplyChainHardening,
    Phase6Architecture, Phase7ConcurrencyAnalysis, Phase8SecurityEngineering,
    Phase9ResourceManagement,
};
pub use transition::{
    TransitionEngine, TransitionError, TransitionHistory, TransitionRecord, TransitionSnapshot,
    TransitionTable,
};

pub use advanced_gates::{
    create_default_phase_configs, create_sample_custom_gates, AdvancedGateEvaluator,
    CompositeCondition, ConditionOperator, CustomGate, CustomGateConfig, ExtendedGateSeverity,
    GateAction, GateActionType, GateBuilder, GateCondition, GateConditionSpec, LogicalOperator,
    PhaseGateConfig,
};
pub use event_sourcing::{
    nexus_event_to_envelope, EventEnvelope, EventMetadata, EventProjection, EventStore,
    PhaseStatisticsProjection, ReplaySession, ReplayStatus, Snapshot,
};
pub use persistence::{
    Checkpoint, CheckpointId, CrashRecovery, FsmState, PhaseStateRecord, PhaseStatus,
    RecoveryEvent, SessionId, SessionStatus, SnapshotId, SnapshotType, StatePersistence,
    StateSnapshot,
};
pub use workflow::{
    create_standard_workflow, DependencyGraph, ParallelConfig, PhaseWorkflow, TaskDefinition,
    TaskExecution, TaskStatus, WorkflowDefinition, WorkflowExecution, WorkflowId,
    WorkflowOrchestrator, WorkflowStatus,
};

#[derive(Debug, thiserror::Error)]
pub enum NexusError {
    #[error("Artifact not found: {0}")]
    ArtifactNotFound(ArtifactId),

    #[error("Transition failed: {0}")]
    TransitionFailed(#[from] TransitionError),

    #[error("Quality gate failed: {gate}")]
    GateFailed { gate: String, message: String },

    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Invalid phase: expected {expected}, got {actual}")]
    InvalidPhase { expected: u8, actual: u8 },

    #[error("Missing required artifact: {0}")]
    MissingArtifact(String),

    #[error("Event bus error: {0}")]
    EventBusError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Workflow error: {0}")]
    WorkflowError(String),

    #[error("Persistence error: {0}")]
    PersistenceError(String),

    #[error("Event sourcing error: {0}")]
    EventSourcingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, NexusError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAnalysis {
    pub domain: String,
    pub standards: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub build_system: String,
    pub dependencies: Vec<String>,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementsSpec {
    pub requirements: Vec<Requirement>,
    pub stakeholders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub description: String,
    pub priority: String,
    pub testable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSynthesis {
    pub yellow_papers: Vec<String>,
    pub test_vectors: Vec<String>,
    pub knowledge_graph: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureSpec {
    pub blue_papers: Vec<String>,
    pub interfaces: Vec<Interface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub signature: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAnalysis {
    pub threats: Vec<Threat>,
    pub compliance_matrix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threat {
    pub id: String,
    pub threat_type: String,
    pub severity: String,
    pub mitigation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub benchmarks: Vec<Benchmark>,
    pub slas: Vec<SLA>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Benchmark {
    pub name: String,
    pub target: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLA {
    pub metric: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub strategy: String,
    pub environment: String,
    pub monitoring: bool,
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_all_phases_defined() {
        let phases = all_phases();
        assert_eq!(phases.len(), 24);

        for (i, phase) in phases.iter().enumerate() {
            assert_eq!(phase.phase_number(), i as u8);
        }
    }

    #[test]
    fn test_phase_id_validation() {
        assert!(PhaseId::new(0).is_ok());
        assert!(PhaseId::new(23).is_ok());
        assert!(PhaseId::new(24).is_err());
        assert!(PhaseId::new(100).is_err());
    }

    #[test]
    fn test_phase_id_properties() {
        let phase = PhaseId(5);
        assert_eq!(phase.next(), Some(PhaseId(6)));
        assert!(!phase.is_terminal());

        let terminal = PhaseId(23);
        assert_eq!(terminal.next(), None);
        assert!(terminal.is_terminal());
    }

    #[test]
    fn test_phase_categories() {
        assert_eq!(
            PhaseCategory::from_phase_number(0),
            PhaseCategory::Discovery
        );
        assert_eq!(
            PhaseCategory::from_phase_number(3),
            PhaseCategory::Requirements
        );
        assert_eq!(
            PhaseCategory::from_phase_number(6),
            PhaseCategory::Architecture
        );
        assert_eq!(
            PhaseCategory::from_phase_number(10),
            PhaseCategory::Planning
        );
        assert_eq!(
            PhaseCategory::from_phase_number(13),
            PhaseCategory::Implementation
        );
        assert_eq!(
            PhaseCategory::from_phase_number(16),
            PhaseCategory::Verification
        );
        assert_eq!(
            PhaseCategory::from_phase_number(20),
            PhaseCategory::Validation
        );
        assert_eq!(
            PhaseCategory::from_phase_number(22),
            PhaseCategory::Transition
        );
    }

    #[test]
    fn test_artifact_types() {
        let types = ArtifactType::all();
        assert_eq!(types.len(), 8);

        for t in &types {
            let s = t.to_string();
            let parsed = ArtifactType::parse_artifact_type(&s);
            assert_eq!(parsed, Some(t.clone()));
        }
    }

    #[test]
    fn test_default_gates_count() {
        let gates = default_gates();
        assert_eq!(gates.len(), 11);
    }

    #[test]
    fn test_nexus_error_display() {
        let error = NexusError::InvalidPhase {
            expected: 0,
            actual: 25,
        };
        assert!(error.to_string().contains("Invalid phase"));

        let error = NexusError::MissingArtifact("test".to_string());
        assert!(error.to_string().contains("Missing required artifact"));
    }

    #[test]
    fn test_domain_analysis_serialization() {
        let analysis = DomainAnalysis {
            domain: "test".to_string(),
            standards: vec!["ISO9001".to_string()],
            description: Some("Test domain".to_string()),
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let parsed: DomainAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.domain, "test");
    }

    #[test]
    fn test_requirements_spec_serialization() {
        let spec = RequirementsSpec {
            requirements: vec![Requirement {
                id: "REQ-001".to_string(),
                description: "Test".to_string(),
                priority: "High".to_string(),
                testable: true,
            }],
            stakeholders: vec!["User".to_string()],
        };

        let json = serde_json::to_string(&spec).unwrap();
        let parsed: RequirementsSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.requirements.len(), 1);
    }

    #[test]
    fn test_performance_baseline_serialization() {
        let baseline = PerformanceBaseline {
            benchmarks: vec![Benchmark {
                name: "latency".to_string(),
                target: 100.0,
                unit: "ms".to_string(),
            }],
            slas: vec![SLA {
                metric: "uptime".to_string(),
                target: "99.9%".to_string(),
            }],
        };

        let json = serde_json::to_string(&baseline).unwrap();
        let parsed: PerformanceBaseline = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.benchmarks.len(), 1);
    }
}
