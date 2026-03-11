//! Transition engine for Nexus FSM
//!
//! This module handles validation and execution of phase transitions,
//! ensuring that all requirements are met before allowing a transition.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::phases::{get_phase_by_id, PhaseState};
use super::{ArtifactTracker, EventBus, GateContext, GateEvaluator, GateResult, PhaseId, Result};

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("Quality gates failed: {0}")]
    GatesFailed(String),

    #[error("Missing required artifacts: {0:?}")]
    MissingArtifacts(Vec<String>),

    #[error("Invalid transition: cannot transition from phase {from} to {to}")]
    InvalidTransition { from: u8, to: u8 },

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("State validation failed: {0}")]
    ValidationFailed(String),

    #[error("Cannot transition from terminal phase")]
    TerminalPhase,

    #[error("Event bus error: {0}")]
    EventBusError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub from_phase: PhaseId,
    pub to_phase: PhaseId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub artifacts_created: Vec<String>,
    pub gates_passed: Vec<String>,
    pub gates_failed: Vec<String>,
    pub metadata: serde_json::Value,
    pub duration_ms: u64,
}

impl TransitionRecord {
    pub fn new(from: PhaseId, to: PhaseId) -> Self {
        Self {
            from_phase: from,
            to_phase: to,
            timestamp: chrono::Utc::now(),
            artifacts_created: Vec::new(),
            gates_passed: Vec::new(),
            gates_failed: Vec::new(),
            metadata: serde_json::json!({}),
            duration_ms: 0,
        }
    }

    pub fn with_artifacts(mut self, artifacts: Vec<String>) -> Self {
        self.artifacts_created = artifacts;
        self
    }

    pub fn with_gates(mut self, passed: Vec<String>, failed: Vec<String>) -> Self {
        self.gates_passed = passed;
        self.gates_failed = failed;
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSnapshot {
    pub phase: PhaseId,
    pub artifacts: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checksum: String,
}

impl TransitionSnapshot {
    pub fn new(phase: PhaseId, artifacts: Vec<String>) -> Self {
        Self {
            phase,
            artifacts,
            timestamp: chrono::Utc::now(),
            checksum: String::new(),
        }
    }

    pub fn compute_checksum(&mut self) {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        for artifact in &self.artifacts {
            hasher.update(artifact.as_bytes());
        }
        self.checksum = format!("{:x}", hasher.finalize());
    }
}

pub struct TransitionEngine {
    artifact_tracker: Arc<ArtifactTracker>,
    gate_evaluator: Arc<GateEvaluator>,
    event_bus: Arc<EventBus>,
    project_root: PathBuf,
    history: Arc<tokio::sync::RwLock<Vec<TransitionRecord>>>,
    max_history: usize,
}

impl TransitionEngine {
    pub fn new(
        artifact_tracker: Arc<ArtifactTracker>,
        gate_evaluator: Arc<GateEvaluator>,
        event_bus: Arc<EventBus>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            artifact_tracker,
            gate_evaluator,
            event_bus,
            project_root,
            history: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            max_history: 100,
        }
    }

    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    pub fn validate_transition(
        &self,
        from: PhaseId,
        to: PhaseId,
    ) -> std::result::Result<(), TransitionError> {
        if from.is_terminal() {
            return Err(TransitionError::TerminalPhase);
        }

        let expected_next = from.next();
        match expected_next {
            Some(next) if next == to => Ok(()),
            Some(_next) => Err(TransitionError::InvalidTransition {
                from: from.0,
                to: to.0,
            }),
            None => Err(TransitionError::TerminalPhase),
        }
    }

    pub fn check_artifacts(
        &self,
        phase: &dyn PhaseState,
    ) -> std::result::Result<(), TransitionError> {
        let required = phase.required_artifacts();
        let mut missing = Vec::new();

        for artifact_type in required {
            let artifacts = self
                .artifact_tracker
                .list_by_type(artifact_type.clone())
                .map_err(|e| TransitionError::ValidationFailed(e.to_string()))?;

            if artifacts.is_empty() {
                missing.push(artifact_type.to_string());
            }
        }

        if missing.is_empty() {
            Ok(())
        } else {
            Err(TransitionError::MissingArtifacts(missing))
        }
    }

    pub fn evaluate_gates(
        &self,
        phase: &dyn PhaseState,
        context: &GateContext,
    ) -> std::result::Result<Vec<GateResult>, TransitionError> {
        let results = self
            .gate_evaluator
            .evaluate_all(phase, context)
            .map_err(|e| TransitionError::GatesFailed(e.to_string()))?;

        let failures: Vec<&GateResult> = results
            .iter()
            .filter(|r| !r.passed && r.severity == super::GateSeverity::Blocking)
            .collect();

        if failures.is_empty() {
            Ok(results)
        } else {
            let failed_gates: Vec<String> = failures.iter().map(|f| f.gate_id.clone()).collect();
            Err(TransitionError::GatesFailed(failed_gates.join(", ")))
        }
    }

    pub async fn execute_transition(
        &self,
        from: PhaseId,
        to: PhaseId,
        metadata: Option<serde_json::Value>,
    ) -> std::result::Result<TransitionRecord, TransitionError> {
        let start_time = std::time::Instant::now();

        self.validate_transition(from, to)?;

        let from_phase = get_phase_by_id(from);
        let _to_phase = get_phase_by_id(to);

        self.check_artifacts(from_phase)?;

        let artifacts = self
            .artifact_tracker
            .list_by_phase(from)
            .map_err(|e| TransitionError::ValidationFailed(e.to_string()))?;

        let mut context = GateContext::new(
            from,
            self.artifact_tracker.clone(),
            self.project_root.clone(),
        );

        for artifact in &artifacts {
            if let serde_json::Value::Object(ref map) = artifact.content {
                for (key, value) in map {
                    context.metadata.insert(key.clone(), value.clone());
                }
            }
        }

        let gate_results = self.evaluate_gates(from_phase, &context)?;

        let passed_gates: Vec<String> = gate_results
            .iter()
            .filter(|r| r.passed)
            .map(|r| r.gate_id.clone())
            .collect();
        let failed_gates: Vec<String> = gate_results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.gate_id.clone())
            .collect();

        let artifact_ids: Vec<String> = artifacts.iter().map(|a| a.id.0.clone()).collect();

        let record = TransitionRecord::new(from, to)
            .with_artifacts(artifact_ids)
            .with_gates(passed_gates, failed_gates)
            .with_metadata(metadata.unwrap_or(serde_json::json!({})))
            .with_duration(start_time.elapsed().as_millis() as u64);

        self.event_bus
            .publish(super::events::NexusEvent::phase_transitioned(from, to))
            .await;

        let mut history = self.history.write().await;
        if history.len() >= self.max_history {
            history.remove(0);
        }
        history.push(record.clone());

        Ok(record)
    }

    pub fn execute_transition_sync(
        &self,
        from: PhaseId,
        to: PhaseId,
        metadata: Option<serde_json::Value>,
    ) -> std::result::Result<TransitionRecord, TransitionError> {
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => handle.block_on(self.execute_transition(from, to, metadata)),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| TransitionError::EventBusError(e.to_string()))?;
                rt.block_on(self.execute_transition(from, to, metadata))
            }
        }
    }

    pub async fn rollback(
        &self,
        record: &TransitionRecord,
    ) -> std::result::Result<(), TransitionError> {
        let _to_phase = get_phase_by_id(record.to_phase);

        let artifacts = self
            .artifact_tracker
            .list_by_phase(record.to_phase)
            .map_err(|e| TransitionError::RollbackFailed(e.to_string()))?;

        for artifact in artifacts {
            if record.artifacts_created.contains(&artifact.id.0) {
                self.artifact_tracker
                    .delete(&artifact.id)
                    .map_err(|e| TransitionError::RollbackFailed(e.to_string()))?;
            }
        }

        self.event_bus
            .publish(super::events::NexusEvent::phase_transitioned(
                record.to_phase,
                record.from_phase,
            ))
            .await;

        Ok(())
    }

    pub fn create_snapshot(&self, phase: PhaseId) -> Result<TransitionSnapshot> {
        let artifacts = self.artifact_tracker.list_by_phase(phase)?;
        let artifact_ids: Vec<String> = artifacts.iter().map(|a| a.id.0.clone()).collect();

        let mut snapshot = TransitionSnapshot::new(phase, artifact_ids);
        snapshot.compute_checksum();

        Ok(snapshot)
    }

    pub fn restore_snapshot(&self, snapshot: &TransitionSnapshot) -> Result<()> {
        let current_artifacts = self.artifact_tracker.list_by_phase(snapshot.phase)?;

        for artifact in current_artifacts {
            if !snapshot.artifacts.contains(&artifact.id.0) {
                self.artifact_tracker.delete(&artifact.id)?;
            }
        }

        Ok(())
    }

    pub async fn history(&self) -> Vec<TransitionRecord> {
        self.history.read().await.clone()
    }

    pub async fn last_transition(&self) -> Option<TransitionRecord> {
        self.history.read().await.last().cloned()
    }

    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }
}

impl std::fmt::Debug for TransitionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransitionEngine")
            .field("project_root", &self.project_root)
            .field("max_history", &self.max_history)
            .finish()
    }
}

pub struct TransitionHistory {
    records: Vec<TransitionRecord>,
    max_records: usize,
}

impl TransitionHistory {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records,
        }
    }

    pub fn record(&mut self, record: TransitionRecord) {
        if self.records.len() >= self.max_records {
            self.records.remove(0);
        }
        self.records.push(record);
    }

    pub fn last(&self) -> Option<&TransitionRecord> {
        self.records.last()
    }

    pub fn history(&self) -> &[TransitionRecord] {
        &self.records
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn for_phase(&self, phase: PhaseId) -> Vec<&TransitionRecord> {
        self.records
            .iter()
            .filter(|r| r.from_phase == phase || r.to_phase == phase)
            .collect()
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }
}

pub struct TransitionTable {
    transitions: HashMap<PhaseId, HashMap<super::events::EventType, PhaseId>>,
}

impl TransitionTable {
    pub fn new() -> Self {
        let mut table = Self {
            transitions: HashMap::new(),
        };
        table.build_transition_table();
        table
    }

    fn build_transition_table(&mut self) {
        for i in 0..23 {
            let from = PhaseId(i);
            let to = PhaseId(i + 1);

            let event = match i {
                0 => super::events::EventType::PhaseCompleted,
                _ => super::events::EventType::PhaseCompleted,
            };

            self.transitions
                .entry(from)
                .or_insert_with(HashMap::new)
                .insert(event, to);
        }
    }

    pub fn get_next(&self, from: PhaseId, event: &super::events::EventType) -> Option<PhaseId> {
        self.transitions
            .get(&from)
            .and_then(|events| events.get(event).copied())
    }

    pub fn valid_transitions(&self, from: PhaseId) -> Vec<(super::events::EventType, PhaseId)> {
        self.transitions
            .get(&from)
            .map(|events| events.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default()
    }
}

impl Default for TransitionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_engine() -> TransitionEngine {
        let tracker = Arc::new(ArtifactTracker::in_memory());
        let evaluator = Arc::new(GateEvaluator::new());
        let bus = Arc::new(EventBus::new());
        TransitionEngine::new(tracker, evaluator, bus, PathBuf::from("/tmp"))
    }

    #[test]
    fn test_transition_history() {
        let mut history = TransitionHistory::new(10);

        let record = TransitionRecord::new(PhaseId(0), PhaseId(1));

        history.record(record);
        assert_eq!(history.count(), 1);
        assert!(history.last().is_some());
    }

    #[test]
    fn test_transition_history_max() {
        let mut history = TransitionHistory::new(5);

        for i in 0..10 {
            let record = TransitionRecord::new(PhaseId(i), PhaseId(i + 1));
            history.record(record);
        }

        assert_eq!(history.count(), 5);
    }

    #[test]
    fn test_transition_record_builder() {
        let record = TransitionRecord::new(PhaseId(0), PhaseId(1))
            .with_artifacts(vec!["artifact1".to_string()])
            .with_gates(vec!["gate1".to_string()], vec!["gate2".to_string()])
            .with_metadata(serde_json::json!({"key": "value"}))
            .with_duration(100);

        assert_eq!(record.from_phase, PhaseId(0));
        assert_eq!(record.to_phase, PhaseId(1));
        assert_eq!(record.artifacts_created.len(), 1);
        assert_eq!(record.gates_passed.len(), 1);
        assert_eq!(record.gates_failed.len(), 1);
        assert_eq!(record.duration_ms, 100);
    }

    #[test]
    fn test_transition_snapshot() {
        let mut snapshot =
            TransitionSnapshot::new(PhaseId(5), vec!["a1".to_string(), "a2".to_string()]);
        assert!(snapshot.checksum.is_empty());

        snapshot.compute_checksum();
        assert!(!snapshot.checksum.is_empty());
    }

    #[test]
    fn test_validate_transition_valid() {
        let engine = create_test_engine();

        assert!(engine.validate_transition(PhaseId(0), PhaseId(1)).is_ok());
        assert!(engine.validate_transition(PhaseId(5), PhaseId(6)).is_ok());
    }

    #[test]
    fn test_validate_transition_invalid() {
        let engine = create_test_engine();

        assert!(engine.validate_transition(PhaseId(0), PhaseId(2)).is_err());
        assert!(engine.validate_transition(PhaseId(5), PhaseId(10)).is_err());
    }

    #[test]
    fn test_validate_transition_terminal() {
        let engine = create_test_engine();

        assert!(engine
            .validate_transition(PhaseId(23), PhaseId(24))
            .is_err());
    }

    #[test]
    fn test_transition_table() {
        let table = TransitionTable::new();

        assert!(table
            .get_next(PhaseId(0), &super::super::events::EventType::PhaseCompleted)
            .is_some());
        assert!(table
            .get_next(
                PhaseId(23),
                &super::super::events::EventType::PhaseCompleted
            )
            .is_none());
    }

    #[test]
    fn test_transition_table_valid_transitions() {
        let table = TransitionTable::new();

        let transitions = table.valid_transitions(PhaseId(0));
        assert!(!transitions.is_empty());

        let terminal_transitions = table.valid_transitions(PhaseId(23));
        assert!(terminal_transitions.is_empty());
    }

    #[test]
    fn test_history_for_phase() {
        let mut history = TransitionHistory::new(10);

        history.record(TransitionRecord::new(PhaseId(0), PhaseId(1)));
        history.record(TransitionRecord::new(PhaseId(1), PhaseId(2)));
        history.record(TransitionRecord::new(PhaseId(2), PhaseId(3)));

        let phase1_records = history.for_phase(PhaseId(1));
        assert_eq!(phase1_records.len(), 2);
    }

    #[tokio::test]
    async fn test_engine_history() {
        let engine = create_test_engine();

        assert_eq!(engine.history().await.len(), 0);

        engine.clear_history().await;
        assert_eq!(engine.history().await.len(), 0);
    }

    #[test]
    fn test_create_snapshot() {
        let engine = create_test_engine();

        let snapshot = engine.create_snapshot(PhaseId(0)).unwrap();
        assert_eq!(snapshot.phase, PhaseId(0));
    }

    #[test]
    fn test_invalid_transition_error() {
        let error = TransitionError::InvalidTransition { from: 5, to: 10 };
        assert!(error.to_string().contains("Invalid transition"));
    }

    #[test]
    fn test_missing_artifacts_error() {
        let error =
            TransitionError::MissingArtifacts(vec!["doc".to_string(), "config".to_string()]);
        assert!(error.to_string().contains("Missing required artifacts"));
    }

    #[test]
    fn test_gates_failed_error() {
        let error = TransitionError::GatesFailed("gate1, gate2".to_string());
        assert!(error.to_string().contains("Quality gates failed"));
    }
}
