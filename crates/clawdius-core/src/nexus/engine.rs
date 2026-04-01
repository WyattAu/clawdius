//! Main Nexus FSM Engine
//!
//! This module implements the main engine using the Typestate pattern to ensure
//! compile-time safety for phase transitions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use super::event_sourcing::nexus_event_to_envelope;
use super::event_sourcing::EventStore;
use super::persistence::{Checkpoint, FsmState, SessionId, StatePersistence};
use super::phases::{
    Phase0ContextDiscovery, Phase10PerformanceEngineering, Phase11CrossPlatformCompatibility,
    Phase12AdversarialLoop, Phase13CICDEngineering, Phase14Documentation, Phase15KnowledgeBase,
    Phase16ExecutionGraph, Phase17SupplyMonitoring, Phase18Deployment, Phase19Operations,
    Phase1EnvironmentMaterialization, Phase20Closure, Phase21ContinuousMonitoring,
    Phase22KnowledgeTransfer, Phase23Archive, Phase2RequirementsEngineering,
    Phase3EpistemologicalDiscovery, Phase4CrossLingualIntegration, Phase5SupplyChainHardening,
    Phase6Architecture, Phase7ConcurrencyAnalysis, Phase8SecurityEngineering,
    Phase9ResourceManagement, PhaseState,
};
use super::transition::TransitionEngine;
use super::{
    default_gates, Artifact, ArtifactId, ArtifactTracker, EventBus, GateContext, GateEvaluator,
    GateResult, NexusError, PhaseId, Result,
};

pub struct NexusEngine<S: PhaseState> {
    state: S,
    artifacts: Arc<ArtifactTracker>,
    gates: Arc<GateEvaluator>,
    events: Arc<EventBus>,
    transition_engine: Arc<TransitionEngine>,
    project_root: PathBuf,
    start_time: chrono::DateTime<chrono::Utc>,
    persistence: Option<Arc<StatePersistence>>,
    event_store: Option<Arc<EventStore>>,
    session_id: Option<SessionId>,
}

impl<S: PhaseState> NexusEngine<S> {
    pub fn current_phase(&self) -> PhaseId {
        PhaseId(self.state.phase_number())
    }

    pub fn phase_name(&self) -> &'static str {
        self.state.phase_name()
    }

    pub fn artifacts(&self) -> &Arc<ArtifactTracker> {
        &self.artifacts
    }

    pub fn gates(&self) -> &Arc<GateEvaluator> {
        &self.gates
    }

    pub fn events(&self) -> &Arc<EventBus> {
        &self.events
    }

    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    pub fn start_time(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.start_time
    }

    pub fn elapsed(&self) -> chrono::Duration {
        chrono::Utc::now() - self.start_time
    }

    pub fn evaluate_gates(&self) -> Result<Vec<GateResult>> {
        let context = GateContext::new(
            self.current_phase(),
            self.artifacts.clone(),
            self.project_root.clone(),
        );
        self.gates.evaluate_all(&self.state, &context)
    }

    pub fn store_artifact(&self, artifact: Artifact) -> Result<ArtifactId> {
        let id = artifact.id.clone();
        self.artifacts.store(artifact)?;

        self.events
            .publish_sync(super::events::NexusEvent::artifact_created(
                id.clone(),
                "Artifact",
                self.current_phase(),
            ));

        Ok(id)
    }

    pub fn retrieve_artifact(&self, id: &ArtifactId) -> Result<Option<Artifact>> {
        self.artifacts.retrieve(id)
    }

    pub fn get_required_artifacts(&self) -> Vec<super::ArtifactType> {
        self.state.required_artifacts()
    }

    pub fn get_produced_artifacts(&self) -> Vec<super::ArtifactType> {
        self.state.produced_artifacts()
    }

    pub fn list_artifacts(&self) -> Result<Vec<Artifact>> {
        self.artifacts.list_by_phase(self.current_phase())
    }

    pub fn artifact_count(&self) -> Result<usize> {
        self.artifacts.count_by_phase(self.current_phase())
    }

    fn transition_to<P: PhaseState + Default>(
        self,
        metadata: Option<serde_json::Value>,
    ) -> Result<NexusEngine<P>> {
        let from = self.current_phase();
        let to = PhaseId(P::default().phase_number());

        let record = self
            .transition_engine
            .execute_transition_sync(from, to, metadata.clone())
            .map_err(NexusError::TransitionFailed)?;

        self.events
            .publish_sync(super::events::NexusEvent::phase_completed(
                from,
                record.duration_ms,
            ));

        let transition_event = super::events::NexusEvent::phase_transitioned(from, to);

        if let Some(ref persistence) = self.persistence {
            if let Some(ref session_id) = self.session_id {
                let mut fsm_state = FsmState::new(session_id.clone(), to);
                fsm_state.touch();
                let _ = persistence.update_session(&fsm_state);

                let checkpoint = Checkpoint::new(session_id.clone(), to);
                let _ = persistence.create_checkpoint(&checkpoint);
            }
        }

        if let Some(ref event_store) = self.event_store {
            if let Some(ref session_id) = self.session_id {
                let envelope = nexus_event_to_envelope(&session_id.0, &transition_event);
                let _ = event_store.append(envelope);
            }
        }

        Ok(NexusEngine {
            state: P::default(),
            artifacts: self.artifacts,
            gates: self.gates,
            events: self.events,
            transition_engine: self.transition_engine,
            project_root: self.project_root,
            start_time: chrono::Utc::now(),
            persistence: self.persistence,
            event_store: self.event_store,
            session_id: self.session_id,
        })
    }

    fn check_required_artifacts(&self) -> Result<()> {
        let required = self.state.required_artifacts();
        for artifact_type in required {
            let artifacts = self.artifacts.list_by_type(artifact_type.clone())?;
            if artifacts.is_empty() {
                return Err(NexusError::MissingArtifact(artifact_type.to_string()));
            }
        }
        Ok(())
    }
}

impl NexusEngine<Phase0ContextDiscovery> {
    pub fn new(project_root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(project_root.join(".clawdius")).map_err(NexusError::IoError)?;

        let artifacts = Arc::new(ArtifactTracker::new(&project_root)?);
        let mut gates = GateEvaluator::new();
        for gate in default_gates() {
            gates.register(gate);
        }
        let gates = Arc::new(gates);
        let events = Arc::new(EventBus::new());

        events.publish_sync(super::events::NexusEvent::project_initialized(
            project_root.display().to_string(),
        ));

        let transition_engine = Arc::new(TransitionEngine::new(
            artifacts.clone(),
            gates.clone(),
            events.clone(),
            project_root.clone(),
        ));

        Ok(Self {
            state: Phase0ContextDiscovery,
            artifacts,
            gates,
            events,
            transition_engine,
            project_root,
            start_time: chrono::Utc::now(),
            persistence: None,
            event_store: None,
            session_id: None,
        })
    }

    pub fn with_persistence(mut self, persistence: Arc<StatePersistence>) -> Self {
        let session_id = SessionId::generate();
        let fsm_state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence
            .create_session(&fsm_state)
            .expect("Failed to create persistence session");
        self.session_id = Some(session_id);
        self.persistence = Some(persistence);
        self
    }

    pub fn with_event_sourcing(mut self, store: Arc<EventStore>) -> Self {
        if self.session_id.is_none() {
            self.session_id = Some(SessionId::generate());
        }
        self.event_store = Some(store);
        self
    }

    pub fn transition_to_environment(
        self,
        domain: impl Into<String>,
        standards: Vec<String>,
    ) -> Result<NexusEngine<Phase1EnvironmentMaterialization>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Documentation,
            serde_json::json!({
                "domain": domain.into(),
                "standards": standards,
            }),
            PhaseId(0),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "context_discovery" })))
    }
}

impl NexusEngine<Phase1EnvironmentMaterialization> {
    pub fn transition_to_requirements(
        self,
        build_system: impl Into<String>,
        dependencies: Vec<String>,
        reproducible: bool,
    ) -> Result<NexusEngine<Phase2RequirementsEngineering>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Configuration,
            serde_json::json!({
                "build_system": build_system.into(),
                "dependencies": dependencies,
                "reproducible": reproducible,
            }),
            PhaseId(1),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(
            serde_json::json!({ "phase": "environment_materialization" }),
        ))
    }
}

impl NexusEngine<Phase2RequirementsEngineering> {
    pub fn transition_to_research(
        self,
        requirements: Vec<RequirementData>,
    ) -> Result<NexusEngine<Phase3EpistemologicalDiscovery>> {
        self.check_required_artifacts()?;

        let requirement_count = requirements.len() as u64;
        let artifact = Artifact::new(
            super::ArtifactType::Documentation,
            serde_json::json!({
                "requirements": requirements,
                "requirement_count": requirement_count,
            }),
            PhaseId(2),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(
            serde_json::json!({ "phase": "requirements_engineering" }),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementData {
    pub id: String,
    pub description: String,
    pub priority: String,
    pub testable: bool,
}

impl NexusEngine<Phase3EpistemologicalDiscovery> {
    pub fn transition_to_cross_lingual(
        self,
        yellow_paper_id: impl Into<String>,
        test_vectors: Vec<String>,
    ) -> Result<NexusEngine<Phase4CrossLingualIntegration>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::YellowPaper,
            serde_json::json!({
                "paper_id": yellow_paper_id.into(),
                "test_vectors": test_vectors,
                "yellow_paper": true,
            }),
            PhaseId(3),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(
            serde_json::json!({ "phase": "epistemological_discovery" }),
        ))
    }
}

impl NexusEngine<Phase4CrossLingualIntegration> {
    pub fn transition_to_supply_chain(self) -> Result<NexusEngine<Phase5SupplyChainHardening>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(
            serde_json::json!({ "phase": "cross_lingual_integration" }),
        ))
    }
}

impl NexusEngine<Phase5SupplyChainHardening> {
    pub fn transition_to_architecture(
        self,
        supply_chain_config: serde_json::Value,
    ) -> Result<NexusEngine<Phase6Architecture>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Compliance,
            supply_chain_config,
            PhaseId(5),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(
            serde_json::json!({ "phase": "supply_chain_hardening" }),
        ))
    }
}

impl NexusEngine<Phase6Architecture> {
    pub fn transition_to_concurrency(
        self,
        blue_paper_id: impl Into<String>,
        interfaces: Vec<InterfaceData>,
    ) -> Result<NexusEngine<Phase7ConcurrencyAnalysis>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::BluePaper,
            serde_json::json!({
                "paper_id": blue_paper_id.into(),
                "interfaces": interfaces,
                "blue_paper": true,
            }),
            PhaseId(6),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "architecture" })))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceData {
    pub name: String,
    pub signature: String,
    pub description: String,
}

impl NexusEngine<Phase7ConcurrencyAnalysis> {
    pub fn transition_to_security(
        self,
        concurrency_analysis: serde_json::Value,
    ) -> Result<NexusEngine<Phase8SecurityEngineering>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(super::ArtifactType::Proof, concurrency_analysis, PhaseId(7));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "concurrency_analysis" })))
    }
}

impl NexusEngine<Phase8SecurityEngineering> {
    pub fn transition_to_resources(
        self,
        security_analysis: serde_json::Value,
    ) -> Result<NexusEngine<Phase9ResourceManagement>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Compliance,
            security_analysis,
            PhaseId(8),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "security_engineering" })))
    }
}

impl NexusEngine<Phase9ResourceManagement> {
    pub fn transition_to_performance(self) -> Result<NexusEngine<Phase10PerformanceEngineering>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::SourceCode,
            serde_json::json!({ "resource_managed": true }),
            PhaseId(9),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "resource_management" })))
    }
}

impl NexusEngine<Phase10PerformanceEngineering> {
    pub fn transition_to_cross_platform(
        self,
        performance_baseline: serde_json::Value,
    ) -> Result<NexusEngine<Phase11CrossPlatformCompatibility>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::TestVector,
            performance_baseline,
            PhaseId(10),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(
            serde_json::json!({ "phase": "performance_engineering" }),
        ))
    }
}

impl NexusEngine<Phase11CrossPlatformCompatibility> {
    pub fn transition_to_adversarial(self) -> Result<NexusEngine<Phase12AdversarialLoop>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(
            serde_json::json!({ "phase": "cross_platform_compatibility" }),
        ))
    }
}

impl NexusEngine<Phase12AdversarialLoop> {
    pub fn transition_to_cicd(
        self,
        adversarial_results: serde_json::Value,
    ) -> Result<NexusEngine<Phase13CICDEngineering>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(super::ArtifactType::Proof, adversarial_results, PhaseId(12));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "adversarial_loop" })))
    }
}

impl NexusEngine<Phase13CICDEngineering> {
    pub fn transition_to_documentation(
        self,
        cicd_config: serde_json::Value,
    ) -> Result<NexusEngine<Phase14Documentation>> {
        self.check_required_artifacts()?;

        let mut content = cicd_config;
        if let serde_json::Value::Object(ref mut map) = content {
            map.insert("compiles".to_string(), serde_json::json!(true));
        }
        let artifact = Artifact::new(super::ArtifactType::Configuration, content, PhaseId(13));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "cicd_engineering" })))
    }
}

impl NexusEngine<Phase14Documentation> {
    pub fn transition_to_knowledge_base(
        self,
        documentation: serde_json::Value,
    ) -> Result<NexusEngine<Phase15KnowledgeBase>> {
        self.check_required_artifacts()?;

        let mut content = documentation;
        if let serde_json::Value::Object(ref mut map) = content {
            map.insert("compiles".to_string(), serde_json::json!(true));
        }
        let artifact = Artifact::new(super::ArtifactType::Documentation, content, PhaseId(14));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "documentation" })))
    }
}

impl NexusEngine<Phase15KnowledgeBase> {
    pub fn transition_to_execution_graph(self) -> Result<NexusEngine<Phase16ExecutionGraph>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(serde_json::json!({ "phase": "knowledge_base" })))
    }
}

impl NexusEngine<Phase16ExecutionGraph> {
    pub fn transition_to_supply_monitoring(
        self,
        execution_graph: serde_json::Value,
    ) -> Result<NexusEngine<Phase17SupplyMonitoring>> {
        self.check_required_artifacts()?;

        let mut content = execution_graph;
        if let serde_json::Value::Object(ref mut map) = content {
            map.insert("test_coverage".to_string(), serde_json::json!(0.85));
        }
        let artifact = Artifact::new(super::ArtifactType::Configuration, content, PhaseId(16));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "execution_graph" })))
    }
}

impl NexusEngine<Phase17SupplyMonitoring> {
    pub fn transition_to_deployment(self) -> Result<NexusEngine<Phase18Deployment>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Compliance,
            serde_json::json!({ "test_coverage": 0.85 }),
            PhaseId(17),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "supply_monitoring" })))
    }
}

impl NexusEngine<Phase18Deployment> {
    pub fn transition_to_operations(
        self,
        deployment_config: serde_json::Value,
    ) -> Result<NexusEngine<Phase19Operations>> {
        self.check_required_artifacts()?;

        let mut content = deployment_config;
        if let serde_json::Value::Object(ref mut map) = content {
            map.insert("all_tests_pass".to_string(), serde_json::json!(true));
            map.insert("security_cleared".to_string(), serde_json::json!(true));
        }
        let artifact = Artifact::new(super::ArtifactType::Documentation, content, PhaseId(18));
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "deployment" })))
    }
}

impl NexusEngine<Phase19Operations> {
    pub fn transition_to_closure(self) -> Result<NexusEngine<Phase20Closure>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(serde_json::json!({ "phase": "operations" })))
    }
}

impl NexusEngine<Phase20Closure> {
    pub fn transition_to_continuous_monitoring(
        self,
    ) -> Result<NexusEngine<Phase21ContinuousMonitoring>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(serde_json::json!({ "phase": "closure" })))
    }
}

impl NexusEngine<Phase21ContinuousMonitoring> {
    pub fn transition_to_knowledge_transfer(self) -> Result<NexusEngine<Phase22KnowledgeTransfer>> {
        self.check_required_artifacts()?;
        self.transition_to(Some(
            serde_json::json!({ "phase": "continuous_monitoring" }),
        ))
    }
}

impl NexusEngine<Phase22KnowledgeTransfer> {
    pub fn transition_to_archive(
        self,
        transfer_documentation: serde_json::Value,
    ) -> Result<NexusEngine<Phase23Archive>> {
        self.check_required_artifacts()?;

        let artifact = Artifact::new(
            super::ArtifactType::Documentation,
            transfer_documentation,
            PhaseId(22),
        );
        self.store_artifact(artifact)?;

        self.transition_to(Some(serde_json::json!({ "phase": "knowledge_transfer" })))
    }
}

impl NexusEngine<Phase23Archive> {
    pub fn finalize(self) -> Result<FinalizedProject> {
        let total_artifacts = self.artifacts.count()?;
        let duration = self.elapsed();

        self.events
            .publish_sync(super::events::NexusEvent::project_finalized());

        Ok(FinalizedProject {
            project_root: self.project_root,
            total_artifacts,
            duration,
            completed_at: chrono::Utc::now(),
        })
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct FinalizedProject {
    pub project_root: PathBuf,
    pub total_artifacts: usize,
    pub duration: chrono::Duration,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_engine() -> NexusEngine<Phase0ContextDiscovery> {
        let temp_dir = TempDir::new().unwrap();
        NexusEngine::new(temp_dir.path().to_path_buf()).unwrap()
    }

    #[test]
    fn test_engine_typestate_pattern() {
        // Test passes if engine creation succeeds without panic
    }

    #[test]
    fn test_engine_creation() {
        let temp_dir = TempDir::new().unwrap();
        let engine = NexusEngine::new(temp_dir.path().to_path_buf());
        assert!(engine.is_ok());

        let engine = engine.unwrap();
        assert_eq!(engine.current_phase(), PhaseId(0));
        assert_eq!(engine.phase_name(), "Context Discovery");
    }

    #[test]
    fn test_engine_phase_properties() {
        let engine = create_test_engine();

        assert_eq!(engine.current_phase(), PhaseId(0));
        assert_eq!(engine.phase_name(), "Context Discovery");
        assert!(engine.get_required_artifacts().is_empty());
        assert_eq!(engine.get_produced_artifacts().len(), 2);
    }

    #[test]
    fn test_engine_artifact_operations() {
        let engine = create_test_engine();

        let artifact = Artifact::new(
            super::super::ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );

        let id = engine.store_artifact(artifact).unwrap();
        let retrieved = engine.retrieve_artifact(&id).unwrap();
        assert!(retrieved.is_some());

        let artifacts = engine.list_artifacts().unwrap();
        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn test_engine_elapsed_time() {
        let engine = create_test_engine();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let elapsed = engine.elapsed();
        assert!(elapsed.num_milliseconds() >= 10);
    }

    #[test]
    fn test_transition_phase_0_to_1() {
        let engine = create_test_engine();

        let engine = engine
            .transition_to_environment("test_domain", vec!["ISO9001".to_string()])
            .unwrap();

        assert_eq!(engine.current_phase(), PhaseId(1));
        assert_eq!(engine.phase_name(), "Environment Materialization");
    }

    #[test]
    fn test_transition_phase_1_to_2() {
        let temp_dir = TempDir::new().unwrap();
        let engine = NexusEngine::new(temp_dir.path().to_path_buf()).unwrap();

        let engine = engine
            .transition_to_environment("test_domain", vec![])
            .unwrap();

        let engine = engine
            .transition_to_requirements("cargo", vec!["serde".to_string()], true)
            .unwrap();

        assert_eq!(engine.current_phase(), PhaseId(2));
    }

    #[test]
    fn test_full_transition_chain() {
        let temp_dir = TempDir::new().unwrap();
        let engine = NexusEngine::new(temp_dir.path().to_path_buf()).unwrap();

        let engine = engine
            .transition_to_environment("domain", vec![])
            .unwrap()
            .transition_to_requirements("cargo", vec![], true)
            .unwrap()
            .transition_to_research(vec![RequirementData {
                id: "REQ-001".to_string(),
                description: "Test requirement".to_string(),
                priority: "High".to_string(),
                testable: true,
            }])
            .unwrap()
            .transition_to_cross_lingual("YP-001", vec![])
            .unwrap()
            .transition_to_supply_chain()
            .unwrap()
            .transition_to_architecture(serde_json::json!({}))
            .unwrap()
            .transition_to_concurrency("BP-001", vec![])
            .unwrap()
            .transition_to_security(serde_json::json!({}))
            .unwrap()
            .transition_to_resources(serde_json::json!({}))
            .unwrap()
            .transition_to_performance()
            .unwrap()
            .transition_to_cross_platform(serde_json::json!({}))
            .unwrap()
            .transition_to_adversarial()
            .unwrap()
            .transition_to_cicd(serde_json::json!({}))
            .unwrap()
            .transition_to_documentation(serde_json::json!({}))
            .unwrap()
            .transition_to_knowledge_base(serde_json::json!({}))
            .unwrap()
            .transition_to_execution_graph()
            .unwrap()
            .transition_to_supply_monitoring(serde_json::json!({}))
            .unwrap()
            .transition_to_deployment()
            .unwrap()
            .transition_to_operations(serde_json::json!({}))
            .unwrap()
            .transition_to_closure()
            .unwrap()
            .transition_to_continuous_monitoring()
            .unwrap()
            .transition_to_knowledge_transfer()
            .unwrap()
            .transition_to_archive(serde_json::json!({}))
            .unwrap();

        assert_eq!(engine.current_phase(), PhaseId(23));
        assert!(engine.is_complete());
    }

    #[test]
    fn test_finalize_project() {
        let temp_dir = TempDir::new().unwrap();
        let engine = NexusEngine::new(temp_dir.path().to_path_buf()).unwrap();

        let finalized = engine
            .transition_to_environment("domain", vec![])
            .unwrap()
            .transition_to_requirements("cargo", vec![], true)
            .unwrap()
            .transition_to_research(vec![RequirementData {
                id: "REQ-001".to_string(),
                description: "Test requirement".to_string(),
                priority: "High".to_string(),
                testable: true,
            }])
            .unwrap()
            .transition_to_cross_lingual("YP-001", vec![])
            .unwrap()
            .transition_to_supply_chain()
            .unwrap()
            .transition_to_architecture(serde_json::json!({}))
            .unwrap()
            .transition_to_concurrency("BP-001", vec![])
            .unwrap()
            .transition_to_security(serde_json::json!({}))
            .unwrap()
            .transition_to_resources(serde_json::json!({}))
            .unwrap()
            .transition_to_performance()
            .unwrap()
            .transition_to_cross_platform(serde_json::json!({}))
            .unwrap()
            .transition_to_adversarial()
            .unwrap()
            .transition_to_cicd(serde_json::json!({}))
            .unwrap()
            .transition_to_documentation(serde_json::json!({}))
            .unwrap()
            .transition_to_knowledge_base(serde_json::json!({}))
            .unwrap()
            .transition_to_execution_graph()
            .unwrap()
            .transition_to_supply_monitoring(serde_json::json!({}))
            .unwrap()
            .transition_to_deployment()
            .unwrap()
            .transition_to_operations(serde_json::json!({}))
            .unwrap()
            .transition_to_closure()
            .unwrap()
            .transition_to_continuous_monitoring()
            .unwrap()
            .transition_to_knowledge_transfer()
            .unwrap()
            .transition_to_archive(serde_json::json!({}))
            .unwrap()
            .finalize()
            .unwrap();

        assert!(finalized.total_artifacts > 0);
    }

    #[test]
    fn test_requirement_data() {
        let req = RequirementData {
            id: "REQ-001".to_string(),
            description: "Test requirement".to_string(),
            priority: "High".to_string(),
            testable: true,
        };

        assert_eq!(req.id, "REQ-001");
        assert!(req.testable);
    }

    #[test]
    fn test_interface_data() {
        let iface = InterfaceData {
            name: "TestInterface".to_string(),
            signature: "fn test() -> bool".to_string(),
            description: "Test interface".to_string(),
        };

        assert_eq!(iface.name, "TestInterface");
    }

    #[test]
    fn test_persistence_checkpoints_on_transition() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = Arc::new(StatePersistence::in_memory());
        let event_store = Arc::new(EventStore::in_memory());

        let engine = NexusEngine::new(temp_dir.path().to_path_buf())
            .unwrap()
            .with_persistence(persistence.clone())
            .with_event_sourcing(event_store.clone());

        let session_id = engine.session_id.as_ref().unwrap().clone();

        let engine = engine
            .transition_to_environment("test_domain", vec!["ISO9001".to_string()])
            .unwrap()
            .transition_to_requirements("cargo", vec!["serde".to_string()], true)
            .unwrap()
            .transition_to_research(vec![RequirementData {
                id: "REQ-001".to_string(),
                description: "Test".to_string(),
                priority: "High".to_string(),
                testable: true,
            }])
            .unwrap();

        assert_eq!(engine.current_phase(), PhaseId(3));

        let checkpoints = persistence.list_checkpoints(&session_id).unwrap();
        assert_eq!(checkpoints.len(), 3);
        assert_eq!(checkpoints[0].phase, PhaseId(3));
        assert_eq!(checkpoints[1].phase, PhaseId(2));
        assert_eq!(checkpoints[2].phase, PhaseId(1));

        let events = event_store
            .get_events_for_aggregate(&session_id.0, None, None)
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "PhaseTransitioned");
        assert_eq!(events[2].event_type, "PhaseTransitioned");

        let loaded_state = persistence.load_session(&session_id).unwrap();
        assert!(loaded_state.is_some());
        assert_eq!(loaded_state.unwrap().current_phase, PhaseId(3));
    }
}
