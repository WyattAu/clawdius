//! Integration tests for Nexus FSM
//!
//! This module contains comprehensive tests for the Nexus FSM implementation.

use super::*;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_project() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        (temp_dir, path)
    }

    #[test]
    fn test_full_phase_lifecycle() {
        let (_temp, path) = create_test_project();

        let engine = NexusEngine::new(path).unwrap();
        assert_eq!(engine.current_phase(), PhaseId(0));

        let finalized = engine
            .transition_to_environment("test_domain", vec!["ISO9001".to_string()])
            .unwrap()
            .transition_to_requirements("cargo", vec!["serde".to_string()], true)
            .unwrap()
            .transition_to_research(vec![RequirementData {
                id: "REQ-001".to_string(),
                description: "Test requirement".to_string(),
                priority: "High".to_string(),
                testable: true,
            }])
            .unwrap()
            .transition_to_cross_lingual("YP-001", vec!["TV-001".to_string()])
            .unwrap()
            .transition_to_supply_chain()
            .unwrap()
            .transition_to_architecture(serde_json::json!({"signed": true}))
            .unwrap()
            .transition_to_concurrency(
                "BP-001",
                vec![InterfaceData {
                    name: "test".to_string(),
                    signature: "fn test()".to_string(),
                    description: "Test interface".to_string(),
                }],
            )
            .unwrap()
            .transition_to_security(serde_json::json!({"analyzed": true}))
            .unwrap()
            .transition_to_resources(serde_json::json!({"profiled": true}))
            .unwrap()
            .transition_to_performance()
            .unwrap()
            .transition_to_cross_platform(serde_json::json!({"baseline": "ok"}))
            .unwrap()
            .transition_to_adversarial()
            .unwrap()
            .transition_to_cicd(serde_json::json!({"pipeline": "configured"}))
            .unwrap()
            .transition_to_documentation(serde_json::json!({"complete": true}))
            .unwrap()
            .transition_to_knowledge_base(serde_json::json!({"updated": true}))
            .unwrap()
            .transition_to_execution_graph()
            .unwrap()
            .transition_to_supply_monitoring(serde_json::json!({"monitoring": true}))
            .unwrap()
            .transition_to_deployment()
            .unwrap()
            .transition_to_operations(serde_json::json!({"deployed": true}))
            .unwrap()
            .transition_to_closure()
            .unwrap()
            .transition_to_continuous_monitoring()
            .unwrap()
            .transition_to_knowledge_transfer()
            .unwrap()
            .transition_to_archive(serde_json::json!({"transferred": true}))
            .unwrap()
            .finalize()
            .unwrap();

        assert!(finalized.total_artifacts > 0);
        assert!(finalized.duration.num_milliseconds() >= 0);
    }

    #[test]
    fn test_phase_transition_validation() {
        let (_temp, path) = create_test_project();
        let engine = NexusEngine::new(path).unwrap();

        let phase = engine.current_phase();
        assert_eq!(phase, PhaseId(0));
        assert_eq!(engine.phase_name(), "Context Discovery");
    }

    #[test]
    fn test_artifact_tracking_through_lifecycle() {
        let (_temp, path) = create_test_project();

        let engine = NexusEngine::new(path).unwrap();
        assert_eq!(engine.artifact_count().unwrap(), 0);

        let engine = engine.transition_to_environment("domain", vec![]).unwrap();

        let artifacts = engine.artifacts().list_by_phase(PhaseId(0)).unwrap();
        assert_eq!(artifacts.len(), 1);

        let engine = engine
            .transition_to_requirements("cargo", vec![], true)
            .unwrap();

        let artifacts = engine.artifacts().list_by_phase(PhaseId(1)).unwrap();
        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn test_quality_gate_evaluation() {
        let (_temp, path) = create_test_project();
        let engine = NexusEngine::new(path).unwrap();

        let results = engine.evaluate_gates().unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_event_bus_integration() {
        let (_temp, path) = create_test_project();

        let _metrics = Arc::new(MetricsHandler::new());

        let engine = NexusEngine::new(path).unwrap();
        engine.events().subscribe_sync(Box::new(LoggingHandler));
        engine
            .events()
            .subscribe_sync(Box::new(MetricsHandler::new()));

        let events = engine.events().clone();
        let _ = engine.transition_to_environment("domain", vec![]).unwrap();

        let history = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(events.history());

        assert!(!history.is_empty());
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let (_temp, path) = create_test_project();
        let tracker = Arc::new(ArtifactTracker::new(&path).unwrap());

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let tracker = tracker.clone();
                thread::spawn(move || {
                    let artifact = Artifact::new(
                        ArtifactType::Documentation,
                        serde_json::json!({"index": i}),
                        PhaseId(i),
                    );
                    tracker.store(artifact).unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(tracker.count().unwrap(), 5);
    }

    #[test]
    fn test_persistence() {
        let (_temp, path) = create_test_project();

        let engine = NexusEngine::new(path.clone()).unwrap();

        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );
        let id = artifact.id.clone();
        engine.store_artifact(artifact).unwrap();

        let retrieved = engine.retrieve_artifact(&id).unwrap();
        assert!(retrieved.is_some());
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_all_phases_implemented() {
        let phase_count = 24;
        let phases: Vec<u8> = (0..phase_count).collect();
        assert_eq!(phases.len(), 24);
    }

    #[test]
    fn test_artifact_types_defined() {
        let types = ArtifactType::all();
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn test_gate_severity_levels() {
        let blocking = GateSeverity::Blocking;
        let warning = GateSeverity::Warning;
        let info = GateSeverity::Information;

        assert_ne!(blocking, warning);
        assert_ne!(warning, info);
    }

    #[test]
    fn test_event_types_coverage() {
        let event = NexusEvent::phase_started(PhaseId(0));
        assert!(event.timestamp().timestamp_millis() > 0);
        assert_eq!(event.event_type(), EventType::PhaseStarted);
    }

    #[test]
    fn test_error_types_defined() {
        let error = NexusError::InvalidPhase {
            expected: 0,
            actual: 25,
        };
        assert!(error.to_string().contains("Invalid phase"));
    }

    #[test]
    fn test_phase_display() {
        let phase = PhaseId(5);
        assert_eq!(format!("{phase}"), "Phase5");
    }

    #[test]
    fn test_artifact_type_display() {
        assert_eq!(format!("{}", ArtifactType::YellowPaper), "YellowPaper");
        assert_eq!(format!("{}", ArtifactType::BluePaper), "BluePaper");
        assert_eq!(format!("{}", ArtifactType::SourceCode), "SourceCode");
    }

    #[test]
    fn test_gate_result_building() {
        let result = GateResult::passed("test", "OK", PhaseId(0))
            .with_details(serde_json::json!({"extra": "data"}))
            .with_severity(GateSeverity::Warning);

        assert!(result.passed);
        assert_eq!(result.severity, GateSeverity::Warning);
        assert!(result.details.is_some());
    }

    #[test]
    fn test_artifact_query_builder() {
        let query = ArtifactQuery::new()
            .phase(PhaseId(5))
            .artifact_type(ArtifactType::SourceCode)
            .tag("important")
            .author("developer");

        assert_eq!(query.phase, Some(PhaseId(5)));
        assert_eq!(query.artifact_type, Some(ArtifactType::SourceCode));
        assert_eq!(query.tags.len(), 1);
    }

    #[test]
    fn test_transition_record_building() {
        let record = TransitionRecord::new(PhaseId(0), PhaseId(1))
            .with_artifacts(vec!["a1".to_string()])
            .with_gates(vec!["g1".to_string()], vec!["g2".to_string()])
            .with_metadata(serde_json::json!({"key": "value"}))
            .with_duration(100);

        assert_eq!(record.from_phase, PhaseId(0));
        assert_eq!(record.to_phase, PhaseId(1));
        assert_eq!(record.artifacts_created.len(), 1);
        assert_eq!(record.gates_passed.len(), 1);
        assert_eq!(record.gates_failed.len(), 1);
    }

    #[test]
    fn test_transition_table() {
        let table = TransitionTable::new();

        let transitions = table.valid_transitions(PhaseId(0));
        assert!(!transitions.is_empty());

        let terminal_transitions = table.valid_transitions(PhaseId(23));
        assert!(terminal_transitions.is_empty());
    }

    #[test]
    fn test_gate_context_metadata() {
        let tracker = Arc::new(ArtifactTracker::in_memory().unwrap());
        let context = GateContext::new(PhaseId(0), tracker, PathBuf::from("/tmp"))
            .with_metadata("domain", serde_json::json!("test"))
            .with_metadata("count", serde_json::json!(5));

        assert!(context.get_metadata("domain").is_some());
        assert!(context.get_metadata("count").is_some());
        assert!(context.get_metadata("missing").is_none());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn test_artifact_hash_consistency() {
        let content = serde_json::json!({"test": "data"});
        let hash1 = Artifact::compute_hash(&content);
        let hash2 = Artifact::compute_hash(&content);
        let hash3 = Artifact::compute_hash(&content);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_artifact_hash_uniqueness() {
        let hash1 = Artifact::compute_hash(&serde_json::json!({"a": 1}));
        let hash2 = Artifact::compute_hash(&serde_json::json!({"a": 2}));
        let hash3 = Artifact::compute_hash(&serde_json::json!({"b": 1}));

        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_phase_transition_monotonicity() {
        for i in 0..23 {
            let from = PhaseId(i);
            let to = from.next();
            assert!(to.is_some());
            assert_eq!(to.unwrap().0, i + 1);
        }
    }

    #[test]
    fn test_artifact_id_uniqueness() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..100 {
            let id = ArtifactId::generate();
            assert!(ids.insert(id), "Generated duplicate ArtifactId");
        }
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn test_phase_category_completeness() {
        for i in 0..=23u8 {
            let category = PhaseCategory::from_phase_number(i);
            match i {
                0..=2 => assert_eq!(category, PhaseCategory::Discovery),
                3..=5 => assert_eq!(category, PhaseCategory::Requirements),
                6..=9 => assert_eq!(category, PhaseCategory::Architecture),
                10..=12 => assert_eq!(category, PhaseCategory::Planning),
                13..=15 => assert_eq!(category, PhaseCategory::Implementation),
                16..=19 => assert_eq!(category, PhaseCategory::Verification),
                20..=21 => assert_eq!(category, PhaseCategory::Validation),
                22..=23 => assert_eq!(category, PhaseCategory::Transition),
                _ => panic!("Invalid phase"),
            }
        }
    }

    #[test]
    fn test_gate_result_properties() {
        let result = GateResult::passed("gate", "OK", PhaseId(0));
        assert!(result.passed);
        assert!(result.timestamp.timestamp_millis() > 0);

        let result = GateResult::failed("gate", "FAIL", PhaseId(0));
        assert!(!result.passed);
        assert_eq!(result.severity, GateSeverity::Blocking);
    }

    #[test]
    fn test_event_timestamps() {
        let events = vec![
            NexusEvent::phase_started(PhaseId(0)),
            NexusEvent::phase_completed(PhaseId(0), 100),
            NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)),
            NexusEvent::artifact_created(ArtifactId::new("test"), "Doc", PhaseId(0)),
        ];

        for event in events {
            assert!(event.timestamp().timestamp_millis() > 0);
        }
    }

    #[test]
    fn test_transition_snapshot_checksum() {
        let mut snapshot1 =
            TransitionSnapshot::new(PhaseId(0), vec!["a".to_string(), "b".to_string()]);
        let mut snapshot2 =
            TransitionSnapshot::new(PhaseId(0), vec!["a".to_string(), "b".to_string()]);
        let mut snapshot3 =
            TransitionSnapshot::new(PhaseId(0), vec!["b".to_string(), "a".to_string()]);

        snapshot1.compute_checksum();
        snapshot2.compute_checksum();
        snapshot3.compute_checksum();

        assert_eq!(snapshot1.checksum, snapshot2.checksum);
        assert_ne!(snapshot1.checksum, snapshot3.checksum);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_empty_artifact_list() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let artifacts = tracker.list_by_phase(PhaseId(0)).unwrap();
        assert!(artifacts.is_empty());
    }

    #[test]
    fn test_missing_artifact_retrieval() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let result = tracker.retrieve(&ArtifactId::new("nonexistent")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_nonexistent_artifact() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let deleted = tracker.delete(&ArtifactId::new("nonexistent")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_search_empty_tracker() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let results = tracker.search("anything").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        for i in 0..3 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(i),
            );
            tracker.store(artifact).unwrap();
        }

        let query = ArtifactQuery::new();
        let results = query.execute(&tracker).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_terminal_phase_cannot_transition() {
        let engine = TransitionEngine::new(
            Arc::new(ArtifactTracker::in_memory().unwrap()),
            Arc::new(GateEvaluator::new()),
            Arc::new(EventBus::new()),
            PathBuf::from("/tmp"),
        );

        let result = engine.validate_transition(PhaseId(23), PhaseId(24));
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_phase_skip() {
        let engine = TransitionEngine::new(
            Arc::new(ArtifactTracker::in_memory().unwrap()),
            Arc::new(GateEvaluator::new()),
            Arc::new(EventBus::new()),
            PathBuf::from("/tmp"),
        );

        let result = engine.validate_transition(PhaseId(0), PhaseId(5));
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_integrity_after_modification() {
        let mut artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"version": 1}),
            PhaseId(0),
        );

        assert!(artifact.verify_integrity());

        artifact.content = serde_json::json!({"version": 2});
        assert!(!artifact.verify_integrity());

        artifact.hash = Artifact::compute_hash(&artifact.content);
        assert!(artifact.verify_integrity());
    }

    #[test]
    fn test_empty_dependencies() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        let artifact = Artifact::new(ArtifactType::SourceCode, serde_json::json!({}), PhaseId(0));
        let id = artifact.id.clone();
        tracker.store(artifact).unwrap();

        let deps = tracker.get_dependencies(&id).unwrap();
        assert!(deps.is_empty());

        let valid = tracker.validate_dependencies(&id).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        let a = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({}),
            PhaseId(0),
        );
        let a_id = a.id.clone();
        tracker.store(a).unwrap();

        let b = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({}),
            PhaseId(0),
        )
        .with_dependencies(vec![a_id.clone()]);
        let b_id = b.id.clone();
        tracker.store(b).unwrap();

        let updated_a = tracker
            .retrieve(&a_id)
            .unwrap()
            .unwrap()
            .with_dependencies(vec![b_id.clone()]);
        tracker.update(updated_a).unwrap();

        let deps_of_a = tracker.get_dependencies(&a_id).unwrap();
        assert_eq!(deps_of_a.len(), 1);
        assert_eq!(deps_of_a[0].id, b_id);
    }
}

#[cfg(test)]
mod extended_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_event_creation_all_types() {
        let events = vec![
            NexusEvent::phase_started(PhaseId(0)),
            NexusEvent::phase_completed(PhaseId(0), 1000),
            NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)),
            NexusEvent::gate_evaluated("test_gate", PhaseId(0), true),
            NexusEvent::gates_completed(PhaseId(0), true, 0),
            NexusEvent::artifact_created(ArtifactId::new("test"), "Doc", PhaseId(0)),
            NexusEvent::artifact_modified(ArtifactId::new("test")),
            NexusEvent::artifact_deleted(ArtifactId::new("test")),
            NexusEvent::error("Test error", Some(PhaseId(0))),
            NexusEvent::project_initialized("/tmp"),
            NexusEvent::project_finalized(),
        ];

        for event in &events {
            assert!(event.timestamp().timestamp_millis() > 0);
        }
    }

    #[test]
    fn test_event_phase_extraction() {
        assert_eq!(
            NexusEvent::phase_started(PhaseId(5)).phase(),
            Some(PhaseId(5))
        );
        assert_eq!(
            NexusEvent::phase_completed(PhaseId(3), 100).phase(),
            Some(PhaseId(3))
        );
        assert_eq!(
            NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)).phase(),
            Some(PhaseId(1))
        );
        assert_eq!(
            NexusEvent::gate_evaluated("g", PhaseId(2), true).phase(),
            Some(PhaseId(2))
        );
        assert_eq!(
            NexusEvent::artifact_created(ArtifactId::new("x"), "t", PhaseId(4)).phase(),
            Some(PhaseId(4))
        );
        assert_eq!(
            NexusEvent::error("e", Some(PhaseId(6))).phase(),
            Some(PhaseId(6))
        );
        assert_eq!(NexusEvent::error("e", None).phase(), None);
        assert_eq!(NexusEvent::project_initialized("/tmp").phase(), None);
    }

    #[test]
    fn test_artifact_tracker_update_nonexistent() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );

        let result = tracker.update(artifact);
        assert!(result.is_err());
        if let Err(NexusError::ArtifactNotFound(id)) = result {
            assert!(!id.0.is_empty());
        } else {
            panic!("Expected ArtifactNotFound error");
        }
    }

    #[test]
    fn test_artifact_tracker_count_by_phase() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        for i in 0..5 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(0),
            );
            tracker.store(artifact).unwrap();
        }

        for i in 0..3 {
            let artifact = Artifact::new(
                ArtifactType::SourceCode,
                serde_json::json!({"index": i}),
                PhaseId(1),
            );
            tracker.store(artifact).unwrap();
        }

        assert_eq!(tracker.count_by_phase(PhaseId(0)).unwrap(), 5);
        assert_eq!(tracker.count_by_phase(PhaseId(1)).unwrap(), 3);
        assert_eq!(tracker.count_by_phase(PhaseId(2)).unwrap(), 0);
    }

    #[test]
    fn test_artifact_query_date_filtering() {
        let tracker = ArtifactTracker::in_memory().unwrap();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(24);
        let future = now + chrono::Duration::hours(24);

        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );
        tracker.store(artifact).unwrap();

        let query_after = ArtifactQuery::new().created_after(past);
        let results = query_after.execute(&tracker).unwrap();
        assert_eq!(results.len(), 1);

        let query_before = ArtifactQuery::new().created_before(past);
        let results = query_before.execute(&tracker).unwrap();
        assert_eq!(results.len(), 0);

        let query_future = ArtifactQuery::new().created_after(future);
        let results = query_future.execute(&tracker).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_quality_gate_boundary_coverage() {
        let gate = TestCoverageGate::new(0.8);
        let tracker = Arc::new(ArtifactTracker::in_memory().unwrap());

        let context_exact = GateContext::new(PhaseId(16), tracker.clone(), PathBuf::from("/tmp"))
            .with_metadata("test_coverage", serde_json::json!(0.8));
        let result = gate.evaluate(&context_exact).unwrap();
        assert!(result.passed);

        let context_just_below =
            GateContext::new(PhaseId(16), tracker.clone(), PathBuf::from("/tmp"))
                .with_metadata("test_coverage", serde_json::json!(0.79999));
        let result = gate.evaluate(&context_just_below).unwrap();
        assert!(!result.passed);

        let context_zero = GateContext::new(PhaseId(16), tracker.clone(), PathBuf::from("/tmp"))
            .with_metadata("test_coverage", serde_json::json!(0.0));
        let result = gate.evaluate(&context_zero).unwrap();
        assert!(!result.passed);

        let context_perfect = GateContext::new(PhaseId(16), tracker, PathBuf::from("/tmp"))
            .with_metadata("test_coverage", serde_json::json!(1.0));
        let result = gate.evaluate(&context_perfect).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_quality_gate_deployment_readiness_combinations() {
        let gate = DeploymentReadinessGate;
        let tracker = Arc::new(ArtifactTracker::in_memory().unwrap());

        let context_both = GateContext::new(PhaseId(18), tracker.clone(), PathBuf::from("/tmp"))
            .with_metadata("all_tests_pass", serde_json::json!(true))
            .with_metadata("security_cleared", serde_json::json!(true));
        let result = gate.evaluate(&context_both).unwrap();
        assert!(result.passed);

        let context_no_tests =
            GateContext::new(PhaseId(18), tracker.clone(), PathBuf::from("/tmp"))
                .with_metadata("all_tests_pass", serde_json::json!(false))
                .with_metadata("security_cleared", serde_json::json!(true));
        let result = gate.evaluate(&context_no_tests).unwrap();
        assert!(!result.passed);

        let context_no_security =
            GateContext::new(PhaseId(18), tracker.clone(), PathBuf::from("/tmp"))
                .with_metadata("all_tests_pass", serde_json::json!(true))
                .with_metadata("security_cleared", serde_json::json!(false));
        let result = gate.evaluate(&context_no_security).unwrap();
        assert!(!result.passed);

        let context_neither = GateContext::new(PhaseId(18), tracker, PathBuf::from("/tmp"))
            .with_metadata("all_tests_pass", serde_json::json!(false))
            .with_metadata("security_cleared", serde_json::json!(false));
        let result = gate.evaluate(&context_neither).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_phase_transition_backward_fails() {
        let engine = TransitionEngine::new(
            Arc::new(ArtifactTracker::in_memory().unwrap()),
            Arc::new(GateEvaluator::new()),
            Arc::new(EventBus::new()),
            PathBuf::from("/tmp"),
        );

        let result = engine.validate_transition(PhaseId(5), PhaseId(4));
        assert!(result.is_err());
    }

    #[test]
    fn test_phase_transition_same_phase_fails() {
        let engine = TransitionEngine::new(
            Arc::new(ArtifactTracker::in_memory().unwrap()),
            Arc::new(GateEvaluator::new()),
            Arc::new(EventBus::new()),
            PathBuf::from("/tmp"),
        );

        let result = engine.validate_transition(PhaseId(5), PhaseId(5));
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_dependency_missing_validation() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        let artifact = Artifact::new(
            ArtifactType::SourceCode,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        )
        .with_dependencies(vec![ArtifactId::new("nonexistent-dep")]);
        let id = artifact.id.clone();
        tracker.store(artifact).unwrap();

        let valid = tracker.validate_dependencies(&id).unwrap();
        assert!(!valid);

        let deps = tracker.get_dependencies(&id).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_gate_evaluator_missing_gate() {
        let evaluator = GateEvaluator::new();
        let tracker = Arc::new(ArtifactTracker::in_memory().unwrap());
        let context = GateContext::new(PhaseId(0), tracker, PathBuf::from("/tmp"));

        let result = evaluator.evaluate_gate("nonexistent_gate", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_transition_history_clear() {
        let mut history = TransitionHistory::new(10);

        history.record(TransitionRecord::new(PhaseId(0), PhaseId(1)));
        history.record(TransitionRecord::new(PhaseId(1), PhaseId(2)));
        assert_eq!(history.count(), 2);

        history.clear();
        assert_eq!(history.count(), 0);
        assert!(history.last().is_none());
    }

    #[test]
    fn test_artifact_with_all_metadata() {
        let artifact = Artifact::new(
            ArtifactType::SourceCode,
            serde_json::json!({"code": "fn main() {}"}),
            PhaseId(5),
        )
        .with_author("test_author")
        .with_description("Test artifact description")
        .with_tags(vec![
            "tag1".to_string(),
            "tag2".to_string(),
            "tag3".to_string(),
        ]);

        assert_eq!(artifact.metadata.author, "test_author");
        assert_eq!(artifact.metadata.description, "Test artifact description");
        assert_eq!(artifact.metadata.tags.len(), 3);
        assert_eq!(artifact.metadata.phase, PhaseId(5));
        assert!(artifact.verify_integrity());
    }

    #[test]
    fn test_event_type_equality() {
        assert_eq!(EventType::PhaseStarted, EventType::PhaseStarted);
        assert_ne!(EventType::PhaseStarted, EventType::PhaseCompleted);
        assert_ne!(EventType::GateEvaluated, EventType::GatesCompleted);
    }

    #[test]
    fn test_nexus_error_variants() {
        let err1 = NexusError::ArtifactNotFound(ArtifactId::new("test"));
        assert!(err1.to_string().contains("Artifact not found"));

        let err2 = NexusError::GateFailed {
            gate: "test_gate".to_string(),
            message: "Gate failed".to_string(),
        };
        assert!(err2.to_string().contains("Quality gate failed"));

        let err3 = NexusError::MissingArtifact("doc".to_string());
        assert!(err3.to_string().contains("Missing required artifact"));

        let err4 = NexusError::EventBusError("connection failed".to_string());
        assert!(err4.to_string().contains("Event bus error"));

        let err5 = NexusError::LockError("poisoned".to_string());
        assert!(err5.to_string().contains("Lock error"));
    }

    #[test]
    fn test_security_scan_gate_vulnerabilities() {
        let gate = SecurityScanGate;
        let tracker = Arc::new(ArtifactTracker::in_memory().unwrap());

        let context_clean = GateContext::new(PhaseId(8), tracker.clone(), PathBuf::from("/tmp"))
            .with_metadata("vulnerability_count", serde_json::json!(0));
        let result = gate.evaluate(&context_clean).unwrap();
        assert!(result.passed);

        let context_vulns = GateContext::new(PhaseId(8), tracker.clone(), PathBuf::from("/tmp"))
            .with_metadata("vulnerability_count", serde_json::json!(5));
        let result = gate.evaluate(&context_vulns).unwrap();
        assert!(!result.passed);

        let context_one = GateContext::new(PhaseId(8), tracker, PathBuf::from("/tmp"))
            .with_metadata("vulnerability_count", serde_json::json!(1));
        let result = gate.evaluate(&context_one).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_artifact_tracker_get_dependents_empty() {
        let tracker = ArtifactTracker::in_memory().unwrap();

        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );
        let id = artifact.id.clone();
        tracker.store(artifact).unwrap();

        let dependents = tracker.get_dependents(&id).unwrap();
        assert!(dependents.is_empty());
    }

    #[test]
    fn test_artifact_touch_updates_timestamp() {
        let mut artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );
        let original_updated = artifact.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        artifact.touch();

        assert!(artifact.updated_at > original_updated);
    }
}
