//! Quality gates for Nexus FSM
//!
//! This module implements the quality gate system that validates phase transitions.
//! Each gate performs specific checks to ensure quality standards are met.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::phases::PhaseState;
use super::{ArtifactTracker, NexusError, PhaseId, Result};

pub trait QualityGate: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn evaluate(&self, context: &GateContext) -> Result<GateResult>;
    fn severity(&self) -> GateSeverity {
        GateSeverity::Blocking
    }
    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateSeverity {
    Blocking,
    Warning,
    Information,
}

impl std::fmt::Display for GateSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateSeverity::Blocking => write!(f, "BLOCKING"),
            GateSeverity::Warning => write!(f, "WARNING"),
            GateSeverity::Information => write!(f, "INFO"),
        }
    }
}

#[derive(Debug)]
pub struct GateContext {
    pub phase: PhaseId,
    pub artifacts: Arc<ArtifactTracker>,
    pub project_root: PathBuf,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl GateContext {
    pub fn new(phase: PhaseId, artifacts: Arc<ArtifactTracker>, project_root: PathBuf) -> Self {
        Self {
            phase,
            artifacts,
            project_root,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_id: String,
    pub passed: bool,
    pub severity: GateSeverity,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub phase: PhaseId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl GateResult {
    pub fn passed(gate_id: impl Into<String>, message: impl Into<String>, phase: PhaseId) -> Self {
        Self {
            gate_id: gate_id.into(),
            passed: true,
            severity: GateSeverity::Blocking,
            message: message.into(),
            details: None,
            phase,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn failed(gate_id: impl Into<String>, message: impl Into<String>, phase: PhaseId) -> Self {
        Self {
            gate_id: gate_id.into(),
            passed: false,
            severity: GateSeverity::Blocking,
            message: message.into(),
            details: None,
            phase,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn warning(gate_id: impl Into<String>, message: impl Into<String>, phase: PhaseId) -> Self {
        Self {
            gate_id: gate_id.into(),
            passed: false,
            severity: GateSeverity::Warning,
            message: message.into(),
            details: None,
            phase,
            timestamp: chrono::Utc::now(),
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    #[must_use]
    pub fn with_severity(mut self, severity: GateSeverity) -> Self {
        self.severity = severity;
        self
    }
}

pub struct GateEvaluator {
    gates: Vec<Box<dyn QualityGate>>,
    exit_gates: HashMap<PhaseId, Vec<String>>,
    entry_gates: HashMap<PhaseId, Vec<String>>,
}

impl GateEvaluator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            gates: Vec::new(),
            exit_gates: HashMap::new(),
            entry_gates: HashMap::new(),
        }
    }

    pub fn register(&mut self, gate: Box<dyn QualityGate>) {
        self.gates.push(gate);
    }

    pub fn register_exit_gate(&mut self, phase: PhaseId, gate_id: impl Into<String>) {
        self.exit_gates
            .entry(phase)
            .or_default()
            .push(gate_id.into());
    }

    pub fn register_entry_gate(&mut self, phase: PhaseId, gate_id: impl Into<String>) {
        self.entry_gates
            .entry(phase)
            .or_default()
            .push(gate_id.into());
    }

    #[must_use]
    pub fn get_gate(&self, gate_id: &str) -> Option<&dyn QualityGate> {
        self.gates
            .iter()
            .find(|g| g.id() == gate_id)
            .map(std::convert::AsRef::as_ref)
    }

    pub fn evaluate_all(
        &self,
        phase: &dyn PhaseState,
        context: &GateContext,
    ) -> Result<Vec<GateResult>> {
        let phase_id = phase.phase_id();
        let applicable_gates: Vec<&dyn QualityGate> = self
            .gates
            .iter()
            .filter(|g| {
                let phases = g.applicable_phases();
                phases.is_empty() || phases.contains(&phase_id)
            })
            .map(std::convert::AsRef::as_ref)
            .collect();

        let mut results = Vec::new();
        for gate in applicable_gates {
            let result = gate.evaluate(context)?;
            results.push(result);
        }

        Ok(results)
    }

    pub fn evaluate_gate(&self, gate_id: &str, context: &GateContext) -> Result<GateResult> {
        let gate = self
            .get_gate(gate_id)
            .ok_or_else(|| NexusError::GateFailed {
                gate: gate_id.to_string(),
                message: "Gate not found".to_string(),
            })?;

        gate.evaluate(context)
    }

    pub fn evaluate_exit_gates(
        &self,
        phase: PhaseId,
        context: &GateContext,
    ) -> Result<Vec<GateResult>> {
        let gate_ids = self.exit_gates.get(&phase).cloned().unwrap_or_default();
        let mut results = Vec::new();

        for gate_id in gate_ids {
            let result = self.evaluate_gate(&gate_id, context)?;
            results.push(result);
        }

        Ok(results)
    }

    pub fn evaluate_entry_gates(
        &self,
        phase: PhaseId,
        context: &GateContext,
    ) -> Result<Vec<GateResult>> {
        let gate_ids = self.entry_gates.get(&phase).cloned().unwrap_or_default();
        let mut results = Vec::new();

        for gate_id in gate_ids {
            let result = self.evaluate_gate(&gate_id, context)?;
            results.push(result);
        }

        Ok(results)
    }

    #[must_use]
    pub fn gates_for_phase(&self, phase: PhaseId) -> Vec<&dyn QualityGate> {
        self.gates
            .iter()
            .filter(|g| {
                let phases = g.applicable_phases();
                phases.is_empty() || phases.contains(&phase)
            })
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    #[must_use]
    pub fn blocking_failures(results: &[GateResult]) -> Vec<&GateResult> {
        results
            .iter()
            .filter(|r| !r.passed && r.severity == GateSeverity::Blocking)
            .collect()
    }

    #[must_use]
    pub fn warnings(results: &[GateResult]) -> Vec<&GateResult> {
        results
            .iter()
            .filter(|r| !r.passed && r.severity == GateSeverity::Warning)
            .collect()
    }

    #[must_use]
    pub fn all_passed(results: &[GateResult]) -> bool {
        results.iter().all(|r| r.passed)
    }

    #[must_use]
    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }
}

impl Default for GateEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GateEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GateEvaluator")
            .field("gate_count", &self.gates.len())
            .field("exit_gates", &self.exit_gates)
            .field("entry_gates", &self.entry_gates)
            .finish()
    }
}

#[derive(Debug)]
pub struct DomainIdentifiedGate;

impl QualityGate for DomainIdentifiedGate {
    fn id(&self) -> &'static str {
        "domain_identified"
    }
    fn description(&self) -> &'static str {
        "Domain must be clearly identified"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(0)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let has_domain = context.metadata.contains_key("domain");

        if has_domain {
            Ok(GateResult::passed(
                self.id(),
                "Domain has been identified",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "Domain has not been identified",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct StandardsMappedGate;

impl QualityGate for StandardsMappedGate {
    fn id(&self) -> &'static str {
        "standards_mapped"
    }
    fn description(&self) -> &'static str {
        "Applicable standards must be mapped"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(0)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let has_standards = context.metadata.contains_key("standards");

        if has_standards {
            Ok(GateResult::passed(
                self.id(),
                "Standards have been mapped",
                context.phase,
            ))
        } else {
            Ok(GateResult::warning(
                self.id(),
                "No standards have been mapped - consider adding applicable standards",
                context.phase,
            )
            .with_severity(GateSeverity::Warning))
        }
    }
}

#[derive(Debug)]
pub struct EnvironmentReproducibleGate;

impl QualityGate for EnvironmentReproducibleGate {
    fn id(&self) -> &'static str {
        "environment_reproducible"
    }
    fn description(&self) -> &'static str {
        "Build environment must be reproducible"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(1)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let is_reproducible = context
            .metadata
            .get("reproducible")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if is_reproducible {
            Ok(GateResult::passed(
                self.id(),
                "Environment is reproducible",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "Environment is not configured for reproducibility",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct RequirementsCompleteGate;

impl QualityGate for RequirementsCompleteGate {
    fn id(&self) -> &'static str {
        "requirements_complete"
    }
    fn description(&self) -> &'static str {
        "All requirements must be documented"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(2)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let req_count = context
            .metadata
            .get("requirement_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if req_count > 0 {
            Ok(GateResult::passed(
                self.id(),
                format!("{req_count} requirements documented"),
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "No requirements have been documented",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct YellowPaperGate;

impl QualityGate for YellowPaperGate {
    fn id(&self) -> &'static str {
        "yellow_paper_complete"
    }
    fn description(&self) -> &'static str {
        "Yellow Paper must be complete with all required sections"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(3)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let has_yellow_paper = context.metadata.contains_key("yellow_paper");

        if has_yellow_paper {
            Ok(GateResult::passed(
                self.id(),
                "Yellow Paper is complete",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "Yellow Paper is not complete",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct BluePaperGate;

impl QualityGate for BluePaperGate {
    fn id(&self) -> &'static str {
        "blue_paper_complete"
    }
    fn description(&self) -> &'static str {
        "Blue Paper must be complete with all required sections"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(6)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let has_blue_paper = context.metadata.contains_key("blue_paper");

        if has_blue_paper {
            Ok(GateResult::passed(
                self.id(),
                "Blue Paper is complete",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "Blue Paper is not complete",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct CompilationGate;

impl QualityGate for CompilationGate {
    fn id(&self) -> &'static str {
        "compilation"
    }
    fn description(&self) -> &'static str {
        "Code must compile without errors"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(13), PhaseId(14)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let compiles = context
            .metadata
            .get("compiles")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if compiles {
            Ok(GateResult::passed(
                self.id(),
                "Code compiles successfully",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                "Code does not compile",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct TestCoverageGate {
    pub minimum_coverage: f64,
}

impl TestCoverageGate {
    #[must_use]
    pub fn new(minimum_coverage: f64) -> Self {
        Self { minimum_coverage }
    }
}

impl QualityGate for TestCoverageGate {
    fn id(&self) -> &'static str {
        "test_coverage"
    }
    fn description(&self) -> &'static str {
        "Test coverage must meet minimum threshold"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(16), PhaseId(17)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let coverage = context
            .metadata
            .get("test_coverage")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);

        if coverage >= self.minimum_coverage {
            Ok(GateResult::passed(
                self.id(),
                format!(
                    "Test coverage {:.1}% meets threshold ({:.1}%)",
                    coverage * 100.0,
                    self.minimum_coverage * 100.0
                ),
                context.phase,
            )
            .with_details(serde_json::json!({ "coverage": coverage })))
        } else {
            Ok(GateResult::failed(
                self.id(),
                format!(
                    "Test coverage {:.1}% below threshold ({:.1}%)",
                    coverage * 100.0,
                    self.minimum_coverage * 100.0
                ),
                context.phase,
            )
            .with_details(
                serde_json::json!({ "coverage": coverage, "required": self.minimum_coverage }),
            ))
        }
    }
}

#[derive(Debug)]
pub struct SecurityScanGate;

impl QualityGate for SecurityScanGate {
    fn id(&self) -> &'static str {
        "security_scan"
    }
    fn description(&self) -> &'static str {
        "Security vulnerabilities must be addressed"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(8), PhaseId(19)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let vulnerabilities = context
            .metadata
            .get("vulnerability_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if vulnerabilities == 0 {
            Ok(GateResult::passed(
                self.id(),
                "No security vulnerabilities found",
                context.phase,
            ))
        } else {
            Ok(GateResult::failed(
                self.id(),
                format!("{vulnerabilities} security vulnerabilities found"),
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct DocumentationGate;

impl QualityGate for DocumentationGate {
    fn id(&self) -> &'static str {
        "documentation"
    }
    fn description(&self) -> &'static str {
        "Documentation must be complete"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(14)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let doc_complete = context
            .metadata
            .get("documentation_complete")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if doc_complete {
            Ok(GateResult::passed(
                self.id(),
                "Documentation is complete",
                context.phase,
            ))
        } else {
            Ok(GateResult::warning(
                self.id(),
                "Documentation may be incomplete",
                context.phase,
            ))
        }
    }
}

#[derive(Debug)]
pub struct DeploymentReadinessGate;

impl QualityGate for DeploymentReadinessGate {
    fn id(&self) -> &'static str {
        "deployment_readiness"
    }
    fn description(&self) -> &'static str {
        "System must be ready for deployment"
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        vec![PhaseId(18)]
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let all_tests_pass = context
            .metadata
            .get("all_tests_pass")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let security_cleared = context
            .metadata
            .get("security_cleared")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if all_tests_pass && security_cleared {
            Ok(GateResult::passed(
                self.id(),
                "System is ready for deployment",
                context.phase,
            ))
        } else {
            let mut issues = Vec::new();
            if !all_tests_pass {
                issues.push("not all tests passing");
            }
            if !security_cleared {
                issues.push("security clearance pending");
            }
            Ok(GateResult::failed(
                self.id(),
                format!("Deployment blocked: {}", issues.join(", ")),
                context.phase,
            ))
        }
    }
}

#[must_use]
pub fn default_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(DomainIdentifiedGate),
        Box::new(StandardsMappedGate),
        Box::new(EnvironmentReproducibleGate),
        Box::new(RequirementsCompleteGate),
        Box::new(YellowPaperGate),
        Box::new(BluePaperGate),
        Box::new(CompilationGate),
        Box::new(TestCoverageGate::new(0.8)),
        Box::new(SecurityScanGate),
        Box::new(DocumentationGate),
        Box::new(DeploymentReadinessGate),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_result_passed() {
        let result = GateResult::passed("test_gate", "All checks passed", PhaseId(0));
        assert!(result.passed);
        assert_eq!(result.gate_id, "test_gate");
        assert_eq!(result.message, "All checks passed");
        assert_eq!(result.severity, GateSeverity::Blocking);
    }

    #[test]
    fn test_gate_result_failed() {
        let result = GateResult::failed("test_gate", "Check failed", PhaseId(1));
        assert!(!result.passed);
        assert_eq!(result.gate_id, "test_gate");
        assert_eq!(result.severity, GateSeverity::Blocking);
    }

    #[test]
    fn test_gate_result_warning() {
        let result = GateResult::warning("test_gate", "Warning", PhaseId(2));
        assert!(!result.passed);
        assert_eq!(result.severity, GateSeverity::Warning);
    }

    #[test]
    fn test_gate_result_with_details() {
        let details = serde_json::json!({ "coverage": 95.5 });
        let result =
            GateResult::passed("coverage", "Good", PhaseId(0)).with_details(details.clone());

        assert_eq!(result.details, Some(details));
    }

    #[test]
    fn test_gate_evaluator_creation() {
        let evaluator = GateEvaluator::new();
        assert_eq!(evaluator.gate_count(), 0);
    }

    #[test]
    fn test_gate_registration() {
        let mut evaluator = GateEvaluator::new();
        evaluator.register(Box::new(DomainIdentifiedGate));
        assert_eq!(evaluator.gate_count(), 1);
    }

    #[test]
    fn test_blocking_failures_filter() {
        let results = vec![
            GateResult::passed("gate1", "OK", PhaseId(0)),
            GateResult::failed("gate2", "Failed", PhaseId(0)).with_severity(GateSeverity::Blocking),
            GateResult::warning("gate3", "Warning", PhaseId(0)),
        ];

        let failures = GateEvaluator::blocking_failures(&results);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].gate_id, "gate2");
    }

    #[test]
    fn test_warnings_filter() {
        let results = vec![
            GateResult::passed("gate1", "OK", PhaseId(0)),
            GateResult::warning("gate2", "Warning", PhaseId(0)),
        ];

        let warnings = GateEvaluator::warnings(&results);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_all_passed() {
        let results = vec![
            GateResult::passed("gate1", "OK", PhaseId(0)),
            GateResult::passed("gate2", "OK", PhaseId(0)),
        ];
        assert!(GateEvaluator::all_passed(&results));

        let results_with_failure = vec![
            GateResult::passed("gate1", "OK", PhaseId(0)),
            GateResult::failed("gate2", "FAIL", PhaseId(0)),
        ];
        assert!(!GateEvaluator::all_passed(&results_with_failure));
    }

    #[test]
    fn test_gate_context() {
        let tracker = Arc::new(ArtifactTracker::new(&std::path::PathBuf::from("/tmp")).unwrap());
        let context = GateContext::new(PhaseId(0), tracker, std::path::PathBuf::from("/tmp"))
            .with_metadata("domain", serde_json::json!("test"));

        assert_eq!(context.phase, PhaseId(0));
        assert!(context.metadata.contains_key("domain"));
    }

    #[test]
    fn test_domain_identified_gate() {
        let gate = DomainIdentifiedGate;
        let tracker = Arc::new(ArtifactTracker::new(&std::path::PathBuf::from("/tmp")).unwrap());

        let context_pass = GateContext::new(
            PhaseId(0),
            tracker.clone(),
            std::path::PathBuf::from("/tmp"),
        )
        .with_metadata("domain", serde_json::json!("test"));
        let result = gate.evaluate(&context_pass).unwrap();
        assert!(result.passed);

        let context_fail = GateContext::new(PhaseId(0), tracker, std::path::PathBuf::from("/tmp"));
        let result = gate.evaluate(&context_fail).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_test_coverage_gate() {
        let gate = TestCoverageGate::new(0.8);
        let tracker = Arc::new(ArtifactTracker::new(&std::path::PathBuf::from("/tmp")).unwrap());

        let context_pass = GateContext::new(
            PhaseId(16),
            tracker.clone(),
            std::path::PathBuf::from("/tmp"),
        )
        .with_metadata("test_coverage", serde_json::json!(0.9));
        let result = gate.evaluate(&context_pass).unwrap();
        assert!(result.passed);

        let context_fail = GateContext::new(PhaseId(16), tracker, std::path::PathBuf::from("/tmp"))
            .with_metadata("test_coverage", serde_json::json!(0.5));
        let result = gate.evaluate(&context_fail).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_exit_entry_gates() {
        let mut evaluator = GateEvaluator::new();
        evaluator.register(Box::new(DomainIdentifiedGate));
        evaluator.register_exit_gate(PhaseId(0), "domain_identified");
        evaluator.register_entry_gate(PhaseId(1), "domain_identified");

        assert!(evaluator.exit_gates.contains_key(&PhaseId(0)));
        assert!(evaluator.entry_gates.contains_key(&PhaseId(1)));
    }

    #[test]
    fn test_default_gates() {
        let gates = default_gates();
        assert_eq!(gates.len(), 11);
    }

    #[test]
    fn test_gate_severity_display() {
        assert_eq!(format!("{}", GateSeverity::Blocking), "BLOCKING");
        assert_eq!(format!("{}", GateSeverity::Warning), "WARNING");
        assert_eq!(format!("{}", GateSeverity::Information), "INFO");
    }
}
