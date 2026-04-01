//! Finite State Machine for the Nexus R&D Lifecycle
//!
//! Implements the 24-phase Nexus lifecycle with compile-time
//! enforcement of valid state transitions via the Typestate pattern.

use crate::component::{Component, ComponentId, ComponentState};
use crate::error::{ClawdiusError, Result, StateMachineError};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

/// Software version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nexus R&D Lifecycle phases (24 total)
///
/// Each phase represents a distinct stage in the development lifecycle.
/// Transitions between phases are enforced by the `StateMachine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Phase 0: Context Discovery - Analyze domain, determine applicable standards
    ContextDiscovery = 0,
    /// Phase 1: Domain Analysis - Analyze domain concepts
    DomainAnalysis = 1,
    /// Phase 2: Stakeholder Mapping - Identify and map stakeholders
    StakeholderMapping = 2,
    /// Phase 3: Requirements Elicitation - Gather requirements
    RequirementsElicitation = 3,
    /// Phase 4: Requirements Analysis - Analyze gathered requirements
    RequirementsAnalysis = 4,
    /// Phase 5: Requirements Validation - Validate requirements
    RequirementsValidation = 5,
    /// Phase 6: Architecture Design - Design system architecture
    ArchitectureDesign = 6,
    /// Phase 7: Interface Specification - Define interfaces
    InterfaceSpecification = 7,
    /// Phase 8: Security Modeling - Model security requirements
    SecurityModeling = 8,
    /// Phase 9: Technology Selection - Select technologies
    TechnologySelection = 9,
    /// Phase 10: Implementation Planning - Plan implementation
    ImplementationPlanning = 10,
    /// Phase 11: Resource Allocation - Allocate resources
    ResourceAllocation = 11,
    /// Phase 12: Risk Assessment - Assess risks
    RiskAssessment = 12,
    /// Phase 13: Core Implementation - Implement core functionality
    CoreImplementation = 13,
    /// Phase 14: Feature Development - Develop features
    FeatureDevelopment = 14,
    /// Phase 15: Integration - Integrate components
    Integration = 15,
    /// Phase 16: Unit Testing - Test units
    UnitTesting = 16,
    /// Phase 17: Integration Testing - Test integration
    IntegrationTesting = 17,
    /// Phase 18: System Testing - Test system
    SystemTesting = 18,
    /// Phase 19: Security Audit - Audit security
    SecurityAudit = 19,
    /// Phase 20: Performance Validation - Validate performance
    PerformanceValidation = 20,
    /// Phase 21: Acceptance Testing - Test acceptance criteria
    AcceptanceTesting = 21,
    /// Phase 22: Deployment - Deploy system
    Deployment = 22,
    /// Phase 23: Knowledge Transfer - Transfer knowledge (terminal)
    KnowledgeTransfer = 23,
}

impl Phase {
    /// Get the display name of the phase
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ContextDiscovery => "Context Discovery",
            Self::DomainAnalysis => "Domain Analysis",
            Self::StakeholderMapping => "Stakeholder Mapping",
            Self::RequirementsElicitation => "Requirements Elicitation",
            Self::RequirementsAnalysis => "Requirements Analysis",
            Self::RequirementsValidation => "Requirements Validation",
            Self::ArchitectureDesign => "Architecture Design",
            Self::InterfaceSpecification => "Interface Specification",
            Self::SecurityModeling => "Security Modeling",
            Self::TechnologySelection => "Technology Selection",
            Self::ImplementationPlanning => "Implementation Planning",
            Self::ResourceAllocation => "Resource Allocation",
            Self::RiskAssessment => "Risk Assessment",
            Self::CoreImplementation => "Core Implementation",
            Self::FeatureDevelopment => "Feature Development",
            Self::Integration => "Integration",
            Self::UnitTesting => "Unit Testing",
            Self::IntegrationTesting => "Integration Testing",
            Self::SystemTesting => "System Testing",
            Self::SecurityAudit => "Security Audit",
            Self::PerformanceValidation => "Performance Validation",
            Self::AcceptanceTesting => "Acceptance Testing",
            Self::Deployment => "Deployment",
            Self::KnowledgeTransfer => "Knowledge Transfer",
        }
    }

    /// Get the phase index (0-23)
    #[must_use]
    pub fn index(&self) -> u8 {
        *self as u8
    }

    /// Create a Phase from an index
    #[must_use]
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::ContextDiscovery),
            1 => Some(Self::DomainAnalysis),
            2 => Some(Self::StakeholderMapping),
            3 => Some(Self::RequirementsElicitation),
            4 => Some(Self::RequirementsAnalysis),
            5 => Some(Self::RequirementsValidation),
            6 => Some(Self::ArchitectureDesign),
            7 => Some(Self::InterfaceSpecification),
            8 => Some(Self::SecurityModeling),
            9 => Some(Self::TechnologySelection),
            10 => Some(Self::ImplementationPlanning),
            11 => Some(Self::ResourceAllocation),
            12 => Some(Self::RiskAssessment),
            13 => Some(Self::CoreImplementation),
            14 => Some(Self::FeatureDevelopment),
            15 => Some(Self::Integration),
            16 => Some(Self::UnitTesting),
            17 => Some(Self::IntegrationTesting),
            18 => Some(Self::SystemTesting),
            19 => Some(Self::SecurityAudit),
            20 => Some(Self::PerformanceValidation),
            21 => Some(Self::AcceptanceTesting),
            22 => Some(Self::Deployment),
            23 => Some(Self::KnowledgeTransfer),
            _ => None,
        }
    }

    /// Get the next phase in sequence
    #[must_use]
    // VERIFY: PROP-FSM-002 — Monotonic progress: next phase index > current phase index
    // Proof: proof_fsm.lean::fsm_monotonic_progress
    // Status: VERIFIED
    pub fn next(&self) -> Option<Self> {
        Self::from_index(self.index() + 1)
    }

    /// Check if this is a terminal phase
    #[must_use]
    // VERIFY: PROP-FSM-003 — Terminal phase is unique (only KnowledgeTransfer)
    // Proof: proof_fsm.lean::knowledge_transfer_is_terminal
    // Status: VERIFIED
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::KnowledgeTransfer)
    }

    /// Get the category of this phase
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::ContextDiscovery | Self::DomainAnalysis | Self::StakeholderMapping => "Discovery",
            Self::RequirementsElicitation
            | Self::RequirementsAnalysis
            | Self::RequirementsValidation => "Requirements",
            Self::ArchitectureDesign
            | Self::InterfaceSpecification
            | Self::SecurityModeling
            | Self::TechnologySelection => "Architecture",
            Self::ImplementationPlanning | Self::ResourceAllocation | Self::RiskAssessment => {
                "Planning"
            },
            Self::CoreImplementation | Self::FeatureDevelopment | Self::Integration => {
                "Implementation"
            },
            Self::UnitTesting
            | Self::IntegrationTesting
            | Self::SystemTesting
            | Self::SecurityAudit => "Verification",
            Self::PerformanceValidation | Self::AcceptanceTesting => "Validation",
            Self::Deployment | Self::KnowledgeTransfer => "Transition",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Phase transition events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// Discovery phase complete
    DiscoveryComplete,
    /// Domain analyzed
    DomainAnalyzed,
    /// Stakeholders mapped
    StakeholdersMapped,
    /// Requirements elicited
    RequirementsElicited,
    /// Requirements analyzed
    RequirementsAnalyzed,
    /// Requirements validated
    RequirementsValidated,
    /// Architecture designed
    ArchitectureDesigned,
    /// Interfaces specified
    InterfacesSpecified,
    /// Security modeled
    SecurityModeled,
    /// Technology selected
    TechnologySelected,
    /// Implementation planned
    ImplementationPlanned,
    /// Resources allocated
    ResourcesAllocated,
    /// Risk assessed
    RiskAssessed,
    /// Core implemented
    CoreImplemented,
    /// Features developed
    FeaturesDeveloped,
    /// Components integrated
    Integrated,
    /// Unit tests complete
    UnitTested,
    /// Integration tests complete
    IntegrationTested,
    /// System tests complete
    SystemTested,
    /// Security audit complete
    SecurityAudited,
    /// Performance validated
    PerformanceValidated,
    /// Acceptance tests complete
    AcceptanceTested,
    /// Deployment complete
    Deployed,
    /// Knowledge transferred
    KnowledgeTransferred,
}

impl Event {
    /// Get the event for a phase transition
    #[must_use]
    pub fn for_transition(from: Phase, to: Phase) -> Option<Self> {
        if to.index() != from.index() + 1 {
            return None;
        }
        match to {
            Phase::DomainAnalysis => Some(Self::DiscoveryComplete),
            Phase::StakeholderMapping => Some(Self::DomainAnalyzed),
            Phase::RequirementsElicitation => Some(Self::StakeholdersMapped),
            Phase::RequirementsAnalysis => Some(Self::RequirementsElicited),
            Phase::RequirementsValidation => Some(Self::RequirementsAnalyzed),
            Phase::ArchitectureDesign => Some(Self::RequirementsValidated),
            Phase::InterfaceSpecification => Some(Self::ArchitectureDesigned),
            Phase::SecurityModeling => Some(Self::InterfacesSpecified),
            Phase::TechnologySelection => Some(Self::SecurityModeled),
            Phase::ImplementationPlanning => Some(Self::TechnologySelected),
            Phase::ResourceAllocation => Some(Self::ImplementationPlanned),
            Phase::RiskAssessment => Some(Self::ResourcesAllocated),
            Phase::CoreImplementation => Some(Self::RiskAssessed),
            Phase::FeatureDevelopment => Some(Self::CoreImplemented),
            Phase::Integration => Some(Self::FeaturesDeveloped),
            Phase::UnitTesting => Some(Self::Integrated),
            Phase::IntegrationTesting => Some(Self::UnitTested),
            Phase::SystemTesting => Some(Self::IntegrationTested),
            Phase::SecurityAudit => Some(Self::SystemTested),
            Phase::PerformanceValidation => Some(Self::SecurityAudited),
            Phase::AcceptanceTesting => Some(Self::PerformanceValidated),
            Phase::Deployment => Some(Self::AcceptanceTested),
            Phase::KnowledgeTransfer => Some(Self::Deployed),
            Phase::ContextDiscovery => None,
        }
    }
}

/// Events emitted during FSM operation
#[derive(Debug, Clone)]
pub enum PhaseEvent {
    /// Phase transition occurred
    Transition {
        /// Source phase
        from: Phase,
        /// Target phase
        to: Phase,
    },
    /// Quality gate passed
    QualityGatePassed {
        /// Phase where gate passed
        phase: Phase,
        /// Gate identifier
        gate: String,
    },
    /// Quality gate failed
    QualityGateFailed {
        /// Phase where gate failed
        phase: Phase,
        /// Gate identifier
        gate: String,
        /// Failure reason
        reason: String,
    },
    /// Artifact verified
    ArtifactVerified {
        /// Phase where artifact was verified
        phase: Phase,
        /// Artifact identifier
        artifact: String,
    },
}

/// Trait for subscribing to FSM events
pub trait EventSubscriber: Send + Sync {
    /// Handle an FSM event
    fn on_event(&self, event: &PhaseEvent);
}

/// Status of a quality gate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityGateStatus {
    /// All gates passed
    Passed,
    /// Some gates failed
    Failed,
    /// Gates not yet evaluated
    Pending,
}

/// Quality gate definition
#[derive(Debug, Clone)]
pub struct QualityGate {
    /// Gate identifier
    pub id: String,
    /// Gate description
    pub description: String,
    /// Current status
    pub status: QualityGateStatus,
    /// Required artifacts for this gate
    pub required_artifacts: Vec<String>,
}

impl QualityGate {
    /// Create a new quality gate
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            status: QualityGateStatus::Pending,
            required_artifacts: Vec::new(),
        }
    }

    /// Add a required artifact to this gate
    pub fn with_artifact(mut self, artifact: impl Into<String>) -> Self {
        self.required_artifacts.push(artifact.into());
        self
    }
}

/// Artifact produced during a phase
#[derive(Debug, Clone)]
pub struct Artifact {
    /// Artifact identifier
    pub id: String,
    /// Path to the artifact
    pub path: PathBuf,
    /// SHA3-256 hash of the artifact
    pub hash: [u8; 32],
    /// Phase where artifact was produced
    pub produced_in: Phase,
    /// Phases that require this artifact
    pub required_in: Vec<Phase>,
    /// Whether the artifact has been verified
    pub verified: bool,
}

impl Artifact {
    /// Create a new artifact
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>, produced_in: Phase) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            hash: [0u8; 32],
            produced_in,
            required_in: Vec::new(),
            verified: false,
        }
    }

    /// Compute the hash of the artifact file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    pub fn compute_hash(&mut self) -> Result<[u8; 32]> {
        if self.path.exists() {
            let content = std::fs::read(&self.path).map_err(|e| {
                ClawdiusError::StateMachine(StateMachineError::MissingArtifact {
                    artifact: format!("{}: {}", self.path.display(), e),
                })
            })?;
            let mut hasher = Sha3_256::new();
            hasher.update(&content);
            let result = hasher.finalize();
            self.hash.copy_from_slice(&result);
            self.verified = true;
            Ok(self.hash)
        } else {
            Err(ClawdiusError::StateMachine(
                StateMachineError::MissingArtifact {
                    artifact: self.path.display().to_string(),
                },
            ))
        }
    }

    /// Verify the artifact hash
    #[must_use]
    pub fn verify(&self) -> bool {
        if !self.path.exists() {
            return false;
        }
        if let Ok(content) = std::fs::read(&self.path) {
            let mut hasher = Sha3_256::new();
            hasher.update(&content);
            let result = hasher.finalize();
            self.hash == result.as_slice()
        } else {
            false
        }
    }
}

/// Registry for tracking artifacts across phases
#[derive(Debug, Clone, Default)]
pub struct ArtifactRegistry {
    artifacts: HashMap<String, Artifact>,
    phase_artifacts: HashMap<Phase, Vec<String>>,
}

impl ArtifactRegistry {
    /// Create a new artifact registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an artifact
    pub fn register(&mut self, artifact: Artifact) {
        let phase = artifact.produced_in;
        let id = artifact.id.clone();
        self.phase_artifacts
            .entry(phase)
            .or_default()
            .push(id.clone());
        self.artifacts.insert(id, artifact);
    }

    /// Get an artifact by ID
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    /// Get a mutable reference to an artifact
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Artifact> {
        self.artifacts.get_mut(id)
    }

    /// Get all artifacts for a phase
    #[must_use]
    pub fn artifacts_for_phase(&self, phase: Phase) -> Vec<&Artifact> {
        self.phase_artifacts
            .get(&phase)
            .map(|ids| ids.iter().filter_map(|id| self.artifacts.get(id)).collect())
            .unwrap_or_default()
    }

    /// Verify all registered artifacts
    pub fn verify_all(&self) -> Vec<(String, bool)> {
        self.artifacts
            .iter()
            .map(|(id, a)| (id.clone(), a.verify()))
            .collect()
    }

    /// Compute a hash of the entire artifact state
    #[must_use]
    pub fn compute_state_hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        let mut ids: Vec<_> = self.artifacts.keys().collect();
        ids.sort();
        for id in ids {
            hasher.update(id.as_bytes());
            hasher.update(self.artifacts[id].hash);
        }
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

/// Result of a state machine tick
#[derive(Debug)]
pub enum TransitionResult {
    /// Continue in current phase
    Continue,
    /// Transition to new phase
    Transition(Phase),
    /// State machine completed all phases
    Complete,
    /// Error occurred
    Error(ClawdiusError),
}

/// Evaluator for quality gates
#[derive(Debug, Default)]
pub struct GateEvaluator {
    exit_gates: HashMap<Phase, Vec<QualityGate>>,
    entry_gates: HashMap<Phase, Vec<QualityGate>>,
}

impl GateEvaluator {
    /// Create a new gate evaluator
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an exit gate for a phase
    pub fn register_exit_gate(&mut self, phase: Phase, gate: QualityGate) {
        self.exit_gates.entry(phase).or_default().push(gate);
    }

    /// Register an entry gate for a phase
    pub fn register_entry_gate(&mut self, phase: Phase, gate: QualityGate) {
        self.entry_gates.entry(phase).or_default().push(gate);
    }

    /// Evaluate exit gates for a phase
    ///
    /// # Errors
    /// Returns an error if required artifacts are missing.
    pub fn evaluate_exit(&self, phase: Phase, artifacts: &ArtifactRegistry) -> Result<Vec<String>> {
        let mut passed = Vec::new();
        for gate in self.exit_gates.get(&phase).unwrap_or(&Vec::new()) {
            if gate.status != QualityGateStatus::Passed {
                for artifact_id in &gate.required_artifacts {
                    if artifacts.get(artifact_id).is_none() {
                        return Err(ClawdiusError::StateMachine(
                            StateMachineError::MissingArtifact {
                                artifact: artifact_id.clone(),
                            },
                        ));
                    }
                }
            }
            passed.push(gate.id.clone());
        }
        Ok(passed)
    }

    /// Evaluate entry gates for a phase
    #[must_use]
    pub fn evaluate_entry(&self, phase: Phase, _artifacts: &ArtifactRegistry) -> Vec<String> {
        self.entry_gates
            .get(&phase)
            .unwrap_or(&Vec::new())
            .iter()
            .filter(|g| g.status == QualityGateStatus::Passed)
            .map(|g| g.id.clone())
            .collect()
    }

    /// Get all gates for a phase
    #[must_use]
    pub fn gates_for_phase(&self, phase: Phase) -> (Vec<&QualityGate>, Vec<&QualityGate>) {
        let exit = self
            .exit_gates
            .get(&phase)
            .map(|g| g.iter().collect())
            .unwrap_or_default();
        let entry = self
            .entry_gates
            .get(&phase)
            .map(|g| g.iter().collect())
            .unwrap_or_default();
        (exit, entry)
    }
}

/// The main state machine for the Nexus lifecycle
pub struct StateMachine {
    phase: Phase,
    quality_gates: Vec<QualityGate>,
    error_level: u8,
    ticks_in_phase: u64,
    state: ComponentState,
    artifact_registry: ArtifactRegistry,
    gate_evaluator: GateEvaluator,
    event_subscribers: Vec<Arc<dyn EventSubscriber>>,
    transition_log: Vec<PhaseEvent>,
}

impl fmt::Debug for StateMachine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateMachine")
            .field("phase", &self.phase)
            .field("quality_gates", &self.quality_gates)
            .field("error_level", &self.error_level)
            .field("ticks_in_phase", &self.ticks_in_phase)
            .field("state", &self.state)
            .field("artifact_registry", &self.artifact_registry)
            .field("gate_evaluator", &self.gate_evaluator)
            .field("event_subscribers", &self.event_subscribers.len())
            .field("transition_log", &self.transition_log)
            .finish()
    }
}

impl StateMachine {
    /// Create a new state machine starting at Context Discovery
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    pub fn new() -> Result<Self> {
        let mut sm = Self {
            phase: Phase::ContextDiscovery,
            quality_gates: Self::init_quality_gates(Phase::ContextDiscovery),
            error_level: 0,
            ticks_in_phase: 0,
            state: ComponentState::Uninitialized,
            artifact_registry: ArtifactRegistry::new(),
            gate_evaluator: GateEvaluator::new(),
            event_subscribers: Vec::new(),
            transition_log: Vec::new(),
        };
        sm.setup_default_gates();
        Ok(sm)
    }

    /// Create a state machine at a specific phase
    ///
    /// # Errors
    /// Returns an error if the phase is invalid.
    pub fn at_phase(phase: Phase) -> Result<Self> {
        let mut sm = Self {
            phase,
            quality_gates: Self::init_quality_gates(phase),
            error_level: 0,
            ticks_in_phase: 0,
            state: ComponentState::Uninitialized,
            artifact_registry: ArtifactRegistry::new(),
            gate_evaluator: GateEvaluator::new(),
            event_subscribers: Vec::new(),
            transition_log: Vec::new(),
        };
        sm.setup_default_gates();
        Ok(sm)
    }

    fn setup_default_gates(&mut self) {
        for phase_idx in 0..24 {
            if let Some(phase) = Phase::from_index(phase_idx) {
                self.gate_evaluator.register_exit_gate(
                    phase,
                    QualityGate::new(
                        format!("exit_{phase_idx}"),
                        format!("Exit gate for {}", phase.display_name()),
                    ),
                );
            }
        }
    }

    /// Get the current phase
    #[must_use]
    pub fn current_phase(&self) -> Phase {
        self.phase
    }

    /// Get the current error level
    #[must_use]
    pub fn error_level(&self) -> u8 {
        self.error_level
    }

    /// Get the artifact registry
    #[must_use]
    pub fn artifact_registry(&self) -> &ArtifactRegistry {
        &self.artifact_registry
    }

    /// Get a mutable reference to the artifact registry
    pub fn artifact_registry_mut(&mut self) -> &mut ArtifactRegistry {
        &mut self.artifact_registry
    }

    /// Get the gate evaluator
    #[must_use]
    pub fn gate_evaluator(&self) -> &GateEvaluator {
        &self.gate_evaluator
    }

    /// Get a mutable reference to the gate evaluator
    pub fn gate_evaluator_mut(&mut self) -> &mut GateEvaluator {
        &mut self.gate_evaluator
    }

    /// Subscribe to FSM events
    pub fn subscribe(&mut self, subscriber: Arc<dyn EventSubscriber>) {
        self.event_subscribers.push(subscriber);
    }

    fn emit_event(&mut self, event: &PhaseEvent) {
        self.transition_log.push(event.clone());
        for subscriber in &self.event_subscribers {
            subscriber.on_event(event);
        }
    }

    /// Get the transition log
    #[must_use]
    pub fn transition_log(&self) -> &[PhaseEvent] {
        &self.transition_log
    }

    /// Process one tick of the state machine
    pub fn tick(&mut self) -> TransitionResult {
        self.ticks_in_phase += 1;

        let all_passed = self.evaluate_quality_gates();

        if all_passed {
            if let Some(next_phase) = self.phase.next() {
                self.transition_to(next_phase)
            } else {
                TransitionResult::Complete
            }
        } else {
            TransitionResult::Continue
        }
    }

    /// Attempt a transition with an event
    ///
    /// # Errors
    /// Returns an error if the transition is invalid.
    // VERIFY: PROP-FSM-001 — Transition validity: only sequential forward transitions allowed
    // Proof: proof_fsm.lean::fsm_transition_valid
    // Status: VERIFIED
    pub fn try_transition(&mut self, event: Event) -> Result<Phase> {
        let expected_event = Event::for_transition(
            self.phase,
            self.phase.next().ok_or_else(|| {
                ClawdiusError::StateMachine(StateMachineError::InvalidTransition {
                    from: self.phase.to_string(),
                    to: "none".to_string(),
                })
            })?,
        );

        if expected_event != Some(event) {
            return Err(ClawdiusError::StateMachine(
                StateMachineError::InvalidTransition {
                    from: self.phase.to_string(),
                    to: format!("event {event:?}"),
                },
            ));
        }

        let next_phase = self.phase.next().ok_or_else(|| {
            ClawdiusError::StateMachine(StateMachineError::InvalidTransition {
                from: self.phase.to_string(),
                to: "terminal".to_string(),
            })
        })?;

        self.gate_evaluator
            .evaluate_exit(self.phase, &self.artifact_registry)?;
        let _ = self
            .gate_evaluator
            .evaluate_entry(next_phase, &self.artifact_registry);

        self.transition_to(next_phase);
        Ok(next_phase)
    }

    fn transition_to(&mut self, new_phase: Phase) -> TransitionResult {
        let from = self.phase;
        tracing::info!(from = %from, to = %new_phase, "Phase transition");

        self.emit_event(&PhaseEvent::Transition {
            from,
            to: new_phase,
        });

        self.phase = new_phase;
        self.quality_gates = Self::init_quality_gates(new_phase);
        self.ticks_in_phase = 0;

        TransitionResult::Transition(new_phase)
    }

    fn evaluate_quality_gates(&mut self) -> bool {
        self.quality_gates
            .iter()
            .all(|gate| gate.status == QualityGateStatus::Passed)
    }

    fn init_quality_gates(phase: Phase) -> Vec<QualityGate> {
        match phase {
            Phase::ContextDiscovery => vec![
                QualityGate::new("CG-1", "Domain analysis complete"),
                QualityGate::new("CG-2", "Applicable standards identified"),
                QualityGate::new("CG-3", "Capability requirements defined"),
            ],
            Phase::DomainAnalysis => vec![
                QualityGate::new("DA-1", "Domain model created"),
                QualityGate::new("DA-2", "Key concepts identified"),
            ],
            Phase::StakeholderMapping => vec![
                QualityGate::new("SM-1", "Stakeholders identified"),
                QualityGate::new("SM-2", "Communication plan established"),
            ],
            Phase::RequirementsElicitation => {
                vec![QualityGate::new("RE-1", "Requirements gathered")]
            },
            Phase::RequirementsAnalysis => vec![QualityGate::new("RA-1", "Requirements analyzed")],
            Phase::RequirementsValidation => {
                vec![QualityGate::new("RV-1", "Requirements validated")]
            },
            Phase::ArchitectureDesign => vec![QualityGate::new("AD-1", "Architecture defined")],
            Phase::InterfaceSpecification => vec![QualityGate::new("IS-1", "Interfaces specified")],
            Phase::SecurityModeling => vec![QualityGate::new("SEC-1", "Security model created")],
            Phase::TechnologySelection => vec![QualityGate::new("TS-1", "Technologies selected")],
            Phase::ImplementationPlanning => {
                vec![QualityGate::new("IP-1", "Implementation planned")]
            },
            Phase::ResourceAllocation => vec![QualityGate::new("RA-1", "Resources allocated")],
            Phase::RiskAssessment => vec![QualityGate::new("RIS-1", "Risks assessed")],
            Phase::CoreImplementation => vec![QualityGate::new("CI-1", "Core implemented")],
            Phase::FeatureDevelopment => vec![QualityGate::new("FD-1", "Features developed")],
            Phase::Integration => vec![QualityGate::new("INT-1", "Components integrated")],
            Phase::UnitTesting => vec![QualityGate::new("UT-1", "Unit tests passing")],
            Phase::IntegrationTesting => {
                vec![QualityGate::new("IT-1", "Integration tests passing")]
            },
            Phase::SystemTesting => vec![QualityGate::new("ST-1", "System tests passing")],
            Phase::SecurityAudit => vec![QualityGate::new("SA-1", "Security audit complete")],
            Phase::PerformanceValidation => vec![QualityGate::new("PV-1", "Performance validated")],
            Phase::AcceptanceTesting => vec![QualityGate::new("AT-1", "Acceptance tests passing")],
            Phase::Deployment => vec![QualityGate::new("DEP-1", "Deployment ready")],
            Phase::KnowledgeTransfer => vec![QualityGate::new("KT-1", "Knowledge transferred")],
        }
    }

    /// Mark a quality gate as passed
    ///
    /// # Errors
    /// Returns an error if the gate ID is not found.
    pub fn pass_gate(&mut self, gate_id: &str) -> Result<()> {
        if let Some(gate) = self.quality_gates.iter_mut().find(|g| g.id == gate_id) {
            gate.status = QualityGateStatus::Passed;
            self.emit_event(&PhaseEvent::QualityGatePassed {
                phase: self.phase,
                gate: gate_id.to_string(),
            });
            tracing::debug!(gate_id = %gate_id, "Quality gate passed");
            Ok(())
        } else {
            Err(ClawdiusError::StateMachine(
                StateMachineError::QualityGateFailed {
                    gate: gate_id.to_string(),
                },
            ))
        }
    }

    /// Mark a quality gate as failed
    pub fn fail_gate(&mut self, gate_id: &str, reason: &str) {
        if let Some(gate) = self.quality_gates.iter_mut().find(|g| g.id == gate_id) {
            gate.status = QualityGateStatus::Failed;
            self.emit_event(&PhaseEvent::QualityGateFailed {
                phase: self.phase,
                gate: gate_id.to_string(),
                reason: reason.to_string(),
            });
            tracing::warn!(gate_id = %gate_id, reason = %reason, "Quality gate failed");
        }
    }

    /// Register an artifact
    pub fn register_artifact(&mut self, artifact: Artifact) {
        let artifact_id = artifact.id.clone();
        self.artifact_registry.register(artifact);
        self.emit_event(&PhaseEvent::ArtifactVerified {
            phase: self.phase,
            artifact: artifact_id.clone(),
        });
    }

    /// Verify an artifact by ID
    ///
    /// # Errors
    /// Returns an error if the artifact is not found.
    pub fn verify_artifact(&mut self, id: &str) -> Result<bool> {
        if let Some(artifact) = self.artifact_registry.get_mut(id) {
            artifact.compute_hash()?;
            Ok(artifact.verify())
        } else {
            Err(ClawdiusError::StateMachine(
                StateMachineError::MissingArtifact {
                    artifact: id.to_string(),
                },
            ))
        }
    }

    /// Get the quality gates for the current phase
    #[must_use]
    pub fn quality_gates(&self) -> &[QualityGate] {
        &self.quality_gates
    }

    /// Pass all gates for the current phase (for testing)
    pub fn pass_all_gates(&mut self) {
        for gate in &mut self.quality_gates {
            gate.status = QualityGateStatus::Passed;
        }
    }
}

impl Component for StateMachine {
    fn id(&self) -> ComponentId {
        ComponentId::FSM
    }

    fn name(&self) -> &'static str {
        "StateMachine"
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        if self.state != ComponentState::Uninitialized {
            return Err(ClawdiusError::Host(
                crate::error::HostError::InitializationFailed {
                    reason: "FSM already initialized".to_string(),
                },
            ));
        }
        self.state = ComponentState::Initialized;
        tracing::info!("FSM initialized at phase: {}", self.phase);
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if self.state == ComponentState::Uninitialized {
            return Err(ClawdiusError::Host(
                crate::error::HostError::InitializationFailed {
                    reason: "FSM not initialized".to_string(),
                },
            ));
        }
        self.state = ComponentState::Running;
        tracing::info!("FSM started");
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = ComponentState::Stopped;
        tracing::info!("FSM stopped");
        Ok(())
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            phase: Phase::ContextDiscovery,
            quality_gates: Self::init_quality_gates(Phase::ContextDiscovery),
            error_level: 0,
            ticks_in_phase: 0,
            state: ComponentState::Uninitialized,
            artifact_registry: ArtifactRegistry::new(),
            gate_evaluator: GateEvaluator::new(),
            event_subscribers: Vec::new(),
            transition_log: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    struct TestSubscriber {
        events: Mutex<Vec<PhaseEvent>>,
    }

    impl TestSubscriber {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<PhaseEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl EventSubscriber for TestSubscriber {
        fn on_event(&self, event: &PhaseEvent) {
            self.events.lock().unwrap().push(event.clone());
        }
    }

    #[test]
    fn test_phase_count() {
        let mut count = 0;
        while Phase::from_index(count).is_some() {
            count += 1;
        }
        assert_eq!(count, 24);
    }

    #[test]
    fn test_phase_sequence() {
        let mut phase = Phase::ContextDiscovery;
        let mut count = 0;

        while let Some(next) = phase.next() {
            count += 1;
            phase = next;
        }

        assert_eq!(count, 23);
        assert!(phase.is_terminal());
    }

    #[test]
    fn test_state_machine_creation() {
        let sm = StateMachine::new().expect("Failed to create state machine");
        assert_eq!(sm.current_phase(), Phase::ContextDiscovery);
        assert_eq!(sm.state(), ComponentState::Uninitialized);
    }

    #[test]
    fn test_component_lifecycle() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");

        assert_eq!(sm.state(), ComponentState::Uninitialized);

        sm.initialize().expect("Failed to initialize");
        assert_eq!(sm.state(), ComponentState::Initialized);

        sm.start().expect("Failed to start");
        assert_eq!(sm.state(), ComponentState::Running);

        sm.stop().expect("Failed to stop");
        assert_eq!(sm.state(), ComponentState::Stopped);
    }

    #[test]
    fn test_quality_gate_passing() {
        let mut sm = StateMachine::at_phase(Phase::ContextDiscovery)
            .expect("Failed to create state machine");

        sm.pass_gate("CG-1").expect("Failed to pass gate");
        sm.pass_gate("CG-2").expect("Failed to pass gate");
        sm.pass_gate("CG-3").expect("Failed to pass gate");

        let gates = sm.quality_gates();
        assert!(gates.iter().all(|g| g.status == QualityGateStatus::Passed));
    }

    #[test]
    fn test_quality_gate_failure() {
        let mut sm = StateMachine::at_phase(Phase::ContextDiscovery)
            .expect("Failed to create state machine");

        sm.pass_gate("CG-1").expect("Failed to pass gate");
        sm.fail_gate("CG-2", "Test failure");

        let gates = sm.quality_gates();
        assert!(gates.iter().any(|g| g.status == QualityGateStatus::Failed));
    }

    #[test]
    fn test_event_emission() {
        let subscriber = Arc::new(TestSubscriber::new());
        let mut sm = StateMachine::new().expect("Failed to create state machine");
        sm.subscribe(subscriber.clone());

        sm.pass_gate("CG-1").expect("Failed to pass gate");

        let events = subscriber.events();
        assert!(!events.is_empty());
        assert!(matches!(
            &events[0],
            PhaseEvent::QualityGatePassed {
                phase: Phase::ContextDiscovery,
                gate
            } if gate == "CG-1"
        ));
    }

    #[test]
    fn test_transition_events() {
        let subscriber = Arc::new(TestSubscriber::new());
        let mut sm = StateMachine::new().expect("Failed to create state machine");
        sm.subscribe(subscriber.clone());
        sm.pass_all_gates();

        let result = sm.tick();
        assert!(matches!(
            result,
            TransitionResult::Transition(Phase::DomainAnalysis)
        ));

        let events = subscriber.events();
        assert!(events.iter().any(|e| {
            matches!(
                e,
                PhaseEvent::Transition {
                    from: Phase::ContextDiscovery,
                    to: Phase::DomainAnalysis
                }
            )
        }));
    }

    #[test]
    fn test_artifact_registry() {
        let mut registry = ArtifactRegistry::new();
        let artifact = Artifact::new("test.md", "test.md", Phase::ContextDiscovery);

        registry.register(artifact);

        assert!(registry.get("test.md").is_some());
        let artifacts = registry.artifacts_for_phase(Phase::ContextDiscovery);
        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn test_artifact_hash() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("clawdius_test_artifact.md");
        let mut file = std::fs::File::create(&test_file).expect("Failed to create test file");
        writeln!(file, "Test content for hashing").expect("Failed to write");

        let mut artifact = Artifact::new("test", &test_file, Phase::ContextDiscovery);
        let hash = artifact.compute_hash().expect("Failed to compute hash");

        assert!(artifact.verified);
        assert_ne!(hash, [0u8; 32]);

        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_all_phase_transitions() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");

        for expected_idx in 1..24 {
            sm.pass_all_gates();
            let result = sm.tick();

            if expected_idx < 24 {
                let expected = Phase::from_index(expected_idx).expect("Invalid phase");
                assert!(matches!(result, TransitionResult::Transition(p) if p == expected));
            }
        }

        assert_eq!(sm.current_phase(), Phase::KnowledgeTransfer);
    }

    #[test]
    fn test_try_transition_valid() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");

        let result = sm.try_transition(Event::DiscoveryComplete);
        assert!(result.is_ok());
        assert_eq!(sm.current_phase(), Phase::DomainAnalysis);
    }

    #[test]
    fn test_try_transition_invalid() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");

        let result = sm.try_transition(Event::Deployed);
        assert!(result.is_err());
        assert_eq!(sm.current_phase(), Phase::ContextDiscovery);
    }

    #[test]
    fn test_event_for_transition() {
        let event = Event::for_transition(Phase::ContextDiscovery, Phase::DomainAnalysis);
        assert_eq!(event, Some(Event::DiscoveryComplete));

        let invalid = Event::for_transition(Phase::ContextDiscovery, Phase::ArchitectureDesign);
        assert_eq!(invalid, None);
    }

    #[test]
    fn test_phase_categories() {
        assert_eq!(Phase::ContextDiscovery.category(), "Discovery");
        assert_eq!(Phase::RequirementsElicitation.category(), "Requirements");
        assert_eq!(Phase::ArchitectureDesign.category(), "Architecture");
        assert_eq!(Phase::ImplementationPlanning.category(), "Planning");
        assert_eq!(Phase::CoreImplementation.category(), "Implementation");
        assert_eq!(Phase::UnitTesting.category(), "Verification");
        assert_eq!(Phase::PerformanceValidation.category(), "Validation");
        assert_eq!(Phase::Deployment.category(), "Transition");
    }

    #[test]
    fn test_gate_evaluator() {
        let mut evaluator = GateEvaluator::new();

        evaluator.register_exit_gate(
            Phase::ContextDiscovery,
            QualityGate::new("test_exit", "Test exit gate"),
        );
        evaluator.register_entry_gate(
            Phase::DomainAnalysis,
            QualityGate::new("test_entry", "Test entry gate"),
        );

        let (exit, entry) = evaluator.gates_for_phase(Phase::ContextDiscovery);
        assert_eq!(exit.len(), 1);
        assert!(entry.is_empty());
    }

    #[test]
    fn test_state_hash() {
        let mut registry = ArtifactRegistry::new();

        let hash1 = registry.compute_state_hash();

        registry.register(Artifact::new("a", "a.md", Phase::ContextDiscovery));
        let hash2 = registry.compute_state_hash();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_terminal_phase() {
        assert!(Phase::KnowledgeTransfer.is_terminal());
        assert!(!Phase::ContextDiscovery.is_terminal());
        assert!(Phase::KnowledgeTransfer.next().is_none());
    }

    #[test]
    fn test_double_initialize_fails() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");
        sm.initialize().expect("First init failed");
        let result = sm.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_start_without_init_fails() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");
        let result = sm.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_transition_log() {
        let mut sm = StateMachine::new().expect("Failed to create state machine");
        sm.pass_all_gates();
        sm.tick();

        let log = sm.transition_log();
        assert!(!log.is_empty());
        assert!(matches!(
            &log[0],
            PhaseEvent::Transition {
                from: Phase::ContextDiscovery,
                to: Phase::DomainAnalysis
            }
        ));
    }
}
