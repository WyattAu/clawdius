//! Phase definitions for Nexus FSM
//!
//! This module defines all 24 phases of the R&D lifecycle using the Typestate pattern.
//! Each phase is represented as a distinct type, making illegal states unrepresentable
//! at compile time.

use super::{ArtifactType, NexusError, Result};
use serde::{Deserialize, Serialize};

pub mod sealed {
    pub trait Sealed {}
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PhaseId(pub u8);

impl PhaseId {
    pub fn new(id: u8) -> Result<Self> {
        if id > 23 {
            return Err(NexusError::InvalidPhase {
                expected: 0,
                actual: id,
            });
        }
        Ok(PhaseId(id))
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.0 == 23
    }

    #[must_use]
    pub fn next(&self) -> Option<PhaseId> {
        if self.0 < 23 {
            Some(PhaseId(self.0 + 1))
        } else {
            None
        }
    }
}

impl std::fmt::Display for PhaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Phase{}", self.0)
    }
}

pub trait PhaseState: sealed::Sealed + Send + Sync + std::fmt::Debug + 'static {
    fn phase_number(&self) -> u8;
    fn phase_name(&self) -> &'static str;
    fn phase_id(&self) -> PhaseId {
        PhaseId(self.phase_number())
    }

    fn required_artifacts(&self) -> Vec<ArtifactType> {
        vec![]
    }

    fn produced_artifacts(&self) -> Vec<ArtifactType> {
        vec![]
    }

    fn category(&self) -> PhaseCategory {
        PhaseCategory::from_phase_number(self.phase_number())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseCategory {
    Discovery,
    Requirements,
    Architecture,
    Planning,
    Implementation,
    Verification,
    Validation,
    Transition,
}

impl PhaseCategory {
    #[must_use]
    pub fn from_phase_number(n: u8) -> Self {
        match n {
            0..=2 => PhaseCategory::Discovery,
            3..=5 => PhaseCategory::Requirements,
            6..=9 => PhaseCategory::Architecture,
            10..=12 => PhaseCategory::Planning,
            13..=15 => PhaseCategory::Implementation,
            16..=19 => PhaseCategory::Verification,
            20..=21 => PhaseCategory::Validation,
            22..=23 => PhaseCategory::Transition,
            _ => panic!("Invalid phase number"),
        }
    }
}

macro_rules! define_phase {
    ($phase_num:expr, $struct_name:ident, $name:expr, [$($required:expr),*], [$($produced:expr),*]) => {
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        pub struct $struct_name;

        impl sealed::Sealed for $struct_name {}

        impl PhaseState for $struct_name {
            fn phase_number(&self) -> u8 { $phase_num }
            fn phase_name(&self) -> &'static str { $name }

            fn required_artifacts(&self) -> Vec<ArtifactType> {
                vec![$($required),*]
            }

            fn produced_artifacts(&self) -> Vec<ArtifactType> {
                vec![$($produced),*]
            }
        }
    };
}

define_phase!(
    0,
    Phase0ContextDiscovery,
    "Context Discovery",
    [],
    [ArtifactType::Documentation, ArtifactType::Configuration]
);

define_phase!(
    1,
    Phase1EnvironmentMaterialization,
    "Environment Materialization",
    [ArtifactType::Documentation],
    [ArtifactType::Configuration]
);

define_phase!(
    2,
    Phase2RequirementsEngineering,
    "Requirements Engineering",
    [ArtifactType::Configuration],
    [ArtifactType::Documentation]
);

define_phase!(
    3,
    Phase3EpistemologicalDiscovery,
    "Epistemological Discovery (Yellow Phase)",
    [ArtifactType::Documentation],
    [ArtifactType::YellowPaper, ArtifactType::TestVector]
);

define_phase!(
    4,
    Phase4CrossLingualIntegration,
    "Cross-Lingual Knowledge Integration",
    [ArtifactType::YellowPaper],
    [ArtifactType::Documentation]
);

define_phase!(
    5,
    Phase5SupplyChainHardening,
    "Supply Chain Hardening",
    [ArtifactType::Documentation],
    [ArtifactType::Configuration, ArtifactType::Compliance]
);

define_phase!(
    6,
    Phase6Architecture,
    "Architectural Specification (Blue Phase)",
    [ArtifactType::Configuration, ArtifactType::Compliance],
    [ArtifactType::BluePaper]
);

define_phase!(
    7,
    Phase7ConcurrencyAnalysis,
    "Concurrency Analysis",
    [ArtifactType::BluePaper],
    [ArtifactType::Documentation, ArtifactType::Proof]
);

define_phase!(
    8,
    Phase8SecurityEngineering,
    "Security Engineering (Red Phase)",
    [ArtifactType::BluePaper, ArtifactType::Proof],
    [ArtifactType::Compliance, ArtifactType::Documentation]
);

define_phase!(
    9,
    Phase9ResourceManagement,
    "Resource Management Analysis",
    [ArtifactType::BluePaper],
    [
        ArtifactType::Documentation,
        ArtifactType::Configuration,
        ArtifactType::SourceCode
    ]
);

define_phase!(
    10,
    Phase10PerformanceEngineering,
    "Performance Engineering (Green Phase)",
    [ArtifactType::SourceCode, ArtifactType::Configuration],
    [ArtifactType::Documentation, ArtifactType::TestVector]
);

define_phase!(
    11,
    Phase11CrossPlatformCompatibility,
    "Cross-Platform Compatibility",
    [ArtifactType::SourceCode],
    [ArtifactType::Documentation, ArtifactType::TestVector]
);

define_phase!(
    12,
    Phase12AdversarialLoop,
    "Adversarial Loop (Feasibility Spike)",
    [ArtifactType::SourceCode, ArtifactType::TestVector],
    [ArtifactType::Proof, ArtifactType::TestVector]
);

define_phase!(
    13,
    Phase13CICDEngineering,
    "CI/CD Engineering",
    [ArtifactType::SourceCode, ArtifactType::TestVector],
    [ArtifactType::Configuration]
);

define_phase!(
    14,
    Phase14Documentation,
    "Documentation Verification",
    [ArtifactType::SourceCode],
    [ArtifactType::Documentation]
);

define_phase!(
    15,
    Phase15KnowledgeBase,
    "Knowledge Base Update",
    [ArtifactType::Documentation],
    [ArtifactType::Documentation]
);

define_phase!(
    16,
    Phase16ExecutionGraph,
    "Execution Graph Generation",
    [ArtifactType::Documentation, ArtifactType::Configuration],
    [ArtifactType::Configuration]
);

define_phase!(
    17,
    Phase17SupplyMonitoring,
    "Supply Chain Monitoring",
    [ArtifactType::Configuration],
    [ArtifactType::Compliance]
);

define_phase!(
    18,
    Phase18Deployment,
    "Deployment & Operations",
    [ArtifactType::Configuration, ArtifactType::Compliance],
    [ArtifactType::Documentation]
);

define_phase!(
    19,
    Phase19Operations,
    "Operations",
    [ArtifactType::Documentation],
    [ArtifactType::Documentation]
);

define_phase!(
    20,
    Phase20Closure,
    "Project Closure",
    [ArtifactType::Documentation],
    [ArtifactType::Documentation]
);

define_phase!(
    21,
    Phase21ContinuousMonitoring,
    "Continuous Monitoring",
    [ArtifactType::Documentation],
    [ArtifactType::Compliance]
);

define_phase!(
    22,
    Phase22KnowledgeTransfer,
    "Knowledge Transfer",
    [ArtifactType::Documentation],
    [ArtifactType::Documentation]
);

define_phase!(
    23,
    Phase23Archive,
    "Archive",
    [ArtifactType::Documentation],
    [ArtifactType::Compliance]
);

#[must_use]
pub fn get_phase_by_id(id: PhaseId) -> &'static dyn PhaseState {
    match id.0 {
        0 => &Phase0ContextDiscovery,
        1 => &Phase1EnvironmentMaterialization,
        2 => &Phase2RequirementsEngineering,
        3 => &Phase3EpistemologicalDiscovery,
        4 => &Phase4CrossLingualIntegration,
        5 => &Phase5SupplyChainHardening,
        6 => &Phase6Architecture,
        7 => &Phase7ConcurrencyAnalysis,
        8 => &Phase8SecurityEngineering,
        9 => &Phase9ResourceManagement,
        10 => &Phase10PerformanceEngineering,
        11 => &Phase11CrossPlatformCompatibility,
        12 => &Phase12AdversarialLoop,
        13 => &Phase13CICDEngineering,
        14 => &Phase14Documentation,
        15 => &Phase15KnowledgeBase,
        16 => &Phase16ExecutionGraph,
        17 => &Phase17SupplyMonitoring,
        18 => &Phase18Deployment,
        19 => &Phase19Operations,
        20 => &Phase20Closure,
        21 => &Phase21ContinuousMonitoring,
        22 => &Phase22KnowledgeTransfer,
        23 => &Phase23Archive,
        _ => panic!("Invalid phase ID"),
    }
}

#[must_use]
pub fn all_phases() -> Vec<&'static dyn PhaseState> {
    (0..=23).map(|i| get_phase_by_id(PhaseId(i))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase0_properties() {
        let phase = Phase0ContextDiscovery;
        assert_eq!(phase.phase_number(), 0);
        assert_eq!(phase.phase_name(), "Context Discovery");
        assert!(phase.required_artifacts().is_empty());
        assert_eq!(phase.produced_artifacts().len(), 2);
    }

    #[test]
    fn test_phase1_properties() {
        let phase = Phase1EnvironmentMaterialization;
        assert_eq!(phase.phase_number(), 1);
        assert_eq!(phase.phase_name(), "Environment Materialization");
        assert_eq!(phase.required_artifacts().len(), 1);
        assert_eq!(phase.produced_artifacts().len(), 1);
    }

    #[test]
    fn test_phase23_properties() {
        let phase = Phase23Archive;
        assert_eq!(phase.phase_number(), 23);
        assert_eq!(phase.phase_name(), "Archive");
        assert!(phase.phase_id().is_terminal());
    }

    #[test]
    fn test_phase_id_validation() {
        assert!(PhaseId::new(23).is_ok());
        assert!(PhaseId::new(24).is_err());
    }

    #[test]
    fn test_phase_id_next() {
        let phase = PhaseId(5);
        assert_eq!(phase.next(), Some(PhaseId(6)));

        let terminal = PhaseId(23);
        assert_eq!(terminal.next(), None);
    }

    #[test]
    fn test_all_phases_defined() {
        let phases: Vec<Box<dyn PhaseState>> = vec![
            Box::new(Phase0ContextDiscovery),
            Box::new(Phase1EnvironmentMaterialization),
            Box::new(Phase2RequirementsEngineering),
            Box::new(Phase3EpistemologicalDiscovery),
            Box::new(Phase4CrossLingualIntegration),
            Box::new(Phase5SupplyChainHardening),
            Box::new(Phase6Architecture),
            Box::new(Phase7ConcurrencyAnalysis),
            Box::new(Phase8SecurityEngineering),
            Box::new(Phase9ResourceManagement),
            Box::new(Phase10PerformanceEngineering),
            Box::new(Phase11CrossPlatformCompatibility),
            Box::new(Phase12AdversarialLoop),
            Box::new(Phase13CICDEngineering),
            Box::new(Phase14Documentation),
            Box::new(Phase15KnowledgeBase),
            Box::new(Phase16ExecutionGraph),
            Box::new(Phase17SupplyMonitoring),
            Box::new(Phase18Deployment),
            Box::new(Phase19Operations),
            Box::new(Phase20Closure),
            Box::new(Phase21ContinuousMonitoring),
            Box::new(Phase22KnowledgeTransfer),
            Box::new(Phase23Archive),
        ];

        assert_eq!(phases.len(), 24);

        for (i, phase) in phases.iter().enumerate() {
            assert_eq!(phase.phase_number() as usize, i);
        }
    }

    #[test]
    fn test_phase_categories() {
        assert_eq!(Phase0ContextDiscovery.category(), PhaseCategory::Discovery);
        assert_eq!(
            Phase3EpistemologicalDiscovery.category(),
            PhaseCategory::Requirements
        );
        assert_eq!(Phase6Architecture.category(), PhaseCategory::Architecture);
        assert_eq!(
            Phase10PerformanceEngineering.category(),
            PhaseCategory::Planning
        );
        assert_eq!(
            Phase13CICDEngineering.category(),
            PhaseCategory::Implementation
        );
        assert_eq!(
            Phase16ExecutionGraph.category(),
            PhaseCategory::Verification
        );
        assert_eq!(Phase20Closure.category(), PhaseCategory::Validation);
        assert_eq!(
            Phase22KnowledgeTransfer.category(),
            PhaseCategory::Transition
        );
    }

    #[test]
    fn test_get_phase_by_id() {
        let phase = get_phase_by_id(PhaseId(0));
        assert_eq!(phase.phase_name(), "Context Discovery");

        let phase = get_phase_by_id(PhaseId(6));
        assert_eq!(
            phase.phase_name(),
            "Architectural Specification (Blue Phase)"
        );
    }

    #[test]
    fn test_all_phases_function() {
        let phases = all_phases();
        assert_eq!(phases.len(), 24);
    }
}
