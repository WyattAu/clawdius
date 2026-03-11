//! Advanced Quality Gates for Nexus FSM Phase 3
//!
//! This module implements phase-specific gate configurations and custom gate predicates
//! for more flexible and powerful quality validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::gates::{GateContext, GateResult, GateSeverity, QualityGate};
use super::{NexusError, PhaseId, Result};

pub type GatePredicate = Box<dyn Fn(&GateContext) -> Result<bool> + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
}

impl GateCondition {
    pub fn new(
        field: impl Into<String>,
        operator: ConditionOperator,
        value: serde_json::Value,
    ) -> Self {
        Self {
            field: field.into(),
            operator,
            value,
        }
    }

    #[must_use]
    pub fn evaluate(&self, context: &GateContext) -> bool {
        let actual = context.metadata.get(&self.field);
        match actual {
            Some(actual_value) => self.compare(actual_value),
            None => false,
        }
    }

    fn compare(&self, actual: &serde_json::Value) -> bool {
        match self.operator {
            ConditionOperator::Equals => actual == &self.value,
            ConditionOperator::NotEquals => actual != &self.value,
            ConditionOperator::GreaterThan => self.compare_numeric(actual, |a, b| a > b),
            ConditionOperator::GreaterThanOrEqual => self.compare_numeric(actual, |a, b| a >= b),
            ConditionOperator::LessThan => self.compare_numeric(actual, |a, b| a < b),
            ConditionOperator::LessThanOrEqual => self.compare_numeric(actual, |a, b| a <= b),
            ConditionOperator::Contains => self.check_contains(actual),
            ConditionOperator::NotContains => !self.check_contains(actual),
            ConditionOperator::Matches => self.check_regex(actual),
            ConditionOperator::Exists => true,
            ConditionOperator::NotExists => false,
        }
    }

    fn compare_numeric<F>(&self, actual: &serde_json::Value, cmp: F) -> bool
    where
        F: Fn(f64, f64) -> bool,
    {
        let actual_num = actual.as_f64();
        let expected_num = self.value.as_f64();
        match (actual_num, expected_num) {
            (Some(a), Some(e)) => cmp(a, e),
            _ => false,
        }
    }

    fn check_contains(&self, actual: &serde_json::Value) -> bool {
        match (actual, &self.value) {
            (serde_json::Value::String(s), serde_json::Value::String(pattern)) => {
                s.contains(pattern)
            }
            (serde_json::Value::Array(arr), _) => arr.contains(&self.value),
            _ => false,
        }
    }

    fn check_regex(&self, actual: &serde_json::Value) -> bool {
        match (actual, &self.value) {
            (serde_json::Value::String(s), serde_json::Value::String(pattern)) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(s))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
    NotContains,
    Matches,
    Exists,
    NotExists,
}

impl std::fmt::Display for ConditionOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionOperator::Equals => write!(f, "=="),
            ConditionOperator::NotEquals => write!(f, "!="),
            ConditionOperator::GreaterThan => write!(f, ">"),
            ConditionOperator::GreaterThanOrEqual => write!(f, ">="),
            ConditionOperator::LessThan => write!(f, "<"),
            ConditionOperator::LessThanOrEqual => write!(f, "<="),
            ConditionOperator::Contains => write!(f, "contains"),
            ConditionOperator::NotContains => write!(f, "!contains"),
            ConditionOperator::Matches => write!(f, "=~"),
            ConditionOperator::Exists => write!(f, "exists"),
            ConditionOperator::NotExists => write!(f, "!exists"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOperator {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeCondition {
    pub operator: LogicalOperator,
    pub conditions: Vec<GateConditionSpec>,
}

impl CompositeCondition {
    #[must_use]
    pub fn and(conditions: Vec<GateConditionSpec>) -> Self {
        Self {
            operator: LogicalOperator::And,
            conditions,
        }
    }

    #[must_use]
    pub fn or(conditions: Vec<GateConditionSpec>) -> Self {
        Self {
            operator: LogicalOperator::Or,
            conditions,
        }
    }

    #[must_use]
    pub fn not(condition: GateConditionSpec) -> Self {
        Self {
            operator: LogicalOperator::Not,
            conditions: vec![condition],
        }
    }

    #[must_use]
    pub fn evaluate(&self, context: &GateContext) -> bool {
        match self.operator {
            LogicalOperator::And => self.conditions.iter().all(|c| c.evaluate(context)),
            LogicalOperator::Or => self.conditions.iter().any(|c| c.evaluate(context)),
            LogicalOperator::Not => !self.conditions.first().is_none_or(|c| c.evaluate(context)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConditionSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simple: Option<GateCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite: Option<CompositeCondition>,
}

impl GateConditionSpec {
    #[must_use]
    pub fn simple(condition: GateCondition) -> Self {
        Self {
            simple: Some(condition),
            composite: None,
        }
    }

    #[must_use]
    pub fn composite(composite: CompositeCondition) -> Self {
        Self {
            simple: None,
            composite: Some(composite),
        }
    }

    #[must_use]
    pub fn evaluate(&self, context: &GateContext) -> bool {
        if let Some(ref simple) = self.simple {
            simple.evaluate(context)
        } else if let Some(ref composite) = self.composite {
            composite.evaluate(context)
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateAction {
    pub on_pass: Vec<GateActionType>,
    pub on_fail: Vec<GateActionType>,
}

impl GateAction {
    #[must_use]
    pub fn new() -> Self {
        Self {
            on_pass: Vec::new(),
            on_fail: Vec::new(),
        }
    }

    #[must_use]
    pub fn on_pass(mut self, action: GateActionType) -> Self {
        self.on_pass.push(action);
        self
    }

    #[must_use]
    pub fn on_fail(mut self, action: GateActionType) -> Self {
        self.on_fail.push(action);
        self
    }
}

impl Default for GateAction {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateActionType {
    Log {
        level: String,
        message: String,
    },
    SetMetadata {
        key: String,
        value: serde_json::Value,
    },
    TriggerEvent {
        event_type: String,
        data: serde_json::Value,
    },
    RequireReReview,
    BlockTransition,
    SkipPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtendedGateSeverity {
    Blocking,
    Warning,
    Information,
    Skippable,
}

impl From<ExtendedGateSeverity> for GateSeverity {
    fn from(severity: ExtendedGateSeverity) -> Self {
        match severity {
            ExtendedGateSeverity::Blocking => GateSeverity::Blocking,
            ExtendedGateSeverity::Warning => GateSeverity::Warning,
            ExtendedGateSeverity::Information => GateSeverity::Information,
            ExtendedGateSeverity::Skippable => GateSeverity::Warning,
        }
    }
}

impl std::fmt::Display for ExtendedGateSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtendedGateSeverity::Blocking => write!(f, "BLOCKING"),
            ExtendedGateSeverity::Warning => write!(f, "WARNING"),
            ExtendedGateSeverity::Information => write!(f, "INFO"),
            ExtendedGateSeverity::Skippable => write!(f, "SKIPPABLE"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGateConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub condition: GateConditionSpec,
    pub severity: ExtendedGateSeverity,
    pub actions: GateAction,
    pub applicable_phases: Vec<PhaseId>,
    pub timeout_ms: Option<u64>,
    pub retry_on_fail: bool,
    pub metadata: serde_json::Value,
}

impl CustomGateConfig {
    pub fn new(id: impl Into<String>, condition: GateConditionSpec) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            description: String::new(),
            condition,
            severity: ExtendedGateSeverity::Blocking,
            actions: GateAction::new(),
            applicable_phases: Vec::new(),
            timeout_ms: None,
            retry_on_fail: false,
            metadata: serde_json::json!({}),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    #[must_use]
    pub fn with_severity(mut self, severity: ExtendedGateSeverity) -> Self {
        self.severity = severity;
        self
    }

    #[must_use]
    pub fn for_phases(mut self, phases: Vec<PhaseId>) -> Self {
        self.applicable_phases = phases;
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[must_use]
    pub fn with_retry(mut self, retry: bool) -> Self {
        self.retry_on_fail = retry;
        self
    }

    #[must_use]
    pub fn with_action(mut self, action: GateAction) -> Self {
        self.actions = action;
        self
    }
}

pub struct CustomGate {
    config: CustomGateConfig,
    predicate: Option<GatePredicate>,
}

impl std::fmt::Debug for CustomGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomGate")
            .field("config", &self.config)
            .field("predicate", &self.predicate.as_ref().map(|_| "<predicate>"))
            .finish()
    }
}

impl CustomGate {
    #[must_use]
    pub fn new(config: CustomGateConfig) -> Self {
        Self {
            config,
            predicate: None,
        }
    }

    #[must_use]
    pub fn with_predicate(mut self, predicate: GatePredicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    #[must_use]
    pub fn from_config(config: CustomGateConfig) -> Self {
        Self::new(config)
    }

    fn evaluate_condition(&self, context: &GateContext) -> bool {
        if let Some(ref predicate) = self.predicate {
            match predicate(context) {
                Ok(result) => result,
                Err(_) => self.config.condition.evaluate(context),
            }
        } else {
            self.config.condition.evaluate(context)
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.config.id
    }

    #[must_use]
    pub fn severity(&self) -> ExtendedGateSeverity {
        self.config.severity
    }

    pub fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let passed = self.evaluate_condition(context);

        if passed {
            Ok(GateResult::passed(
                self.id(),
                format!("{} passed", self.config.name),
                context.phase,
            )
            .with_severity(GateSeverity::from(self.config.severity)))
        } else {
            Ok(GateResult::failed(
                self.id(),
                format!("{} failed", self.config.name),
                context.phase,
            )
            .with_severity(GateSeverity::from(self.config.severity)))
        }
    }
}

impl QualityGate for CustomGate {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn severity(&self) -> GateSeverity {
        GateSeverity::from(self.config.severity)
    }

    fn applicable_phases(&self) -> Vec<PhaseId> {
        self.config.applicable_phases.clone()
    }

    fn evaluate(&self, context: &GateContext) -> Result<GateResult> {
        let passed = self.evaluate_condition(context);

        if passed {
            Ok(GateResult::passed(
                self.id(),
                format!("{} passed", self.config.name),
                context.phase,
            )
            .with_severity(self.severity().into()))
        } else {
            Ok(GateResult::failed(
                self.id(),
                format!("{} failed", self.config.name),
                context.phase,
            )
            .with_severity(self.severity().into()))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseGateConfig {
    pub phase: PhaseId,
    pub entry_gates: Vec<String>,
    pub exit_gates: Vec<String>,
    pub required_pass_count: usize,
    pub allow_warnings: bool,
    pub custom_gates: Vec<CustomGateConfig>,
}

impl PhaseGateConfig {
    #[must_use]
    pub fn new(phase: PhaseId) -> Self {
        Self {
            phase,
            entry_gates: Vec::new(),
            exit_gates: Vec::new(),
            required_pass_count: 1,
            allow_warnings: true,
            custom_gates: Vec::new(),
        }
    }

    pub fn with_entry_gate(mut self, gate_id: impl Into<String>) -> Self {
        self.entry_gates.push(gate_id.into());
        self
    }

    pub fn with_exit_gate(mut self, gate_id: impl Into<String>) -> Self {
        self.exit_gates.push(gate_id.into());
        self
    }

    #[must_use]
    pub fn with_required_pass_count(mut self, count: usize) -> Self {
        self.required_pass_count = count;
        self
    }

    #[must_use]
    pub fn with_allow_warnings(mut self, allow: bool) -> Self {
        self.allow_warnings = allow;
        self
    }

    #[must_use]
    pub fn with_custom_gate(mut self, gate: CustomGateConfig) -> Self {
        self.custom_gates.push(gate);
        self
    }
}

#[allow(dead_code)]
pub struct AdvancedGateEvaluator {
    standard_gates: Vec<Box<dyn QualityGate>>,
    custom_gates: HashMap<String, CustomGate>,
    phase_configs: HashMap<PhaseId, PhaseGateConfig>,
    gate_results_cache: HashMap<String, GateResult>,
}

impl AdvancedGateEvaluator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            standard_gates: Vec::new(),
            custom_gates: HashMap::new(),
            phase_configs: HashMap::new(),
            gate_results_cache: HashMap::new(),
        }
    }

    pub fn register_standard(&mut self, gate: Box<dyn QualityGate>) {
        self.standard_gates.push(gate);
    }

    pub fn register_custom(&mut self, gate: CustomGate) {
        self.custom_gates.insert(gate.id().to_string(), gate);
    }

    pub fn register_custom_from_config(&mut self, config: CustomGateConfig) {
        let gate = CustomGate::from_config(config);
        self.register_custom(gate);
    }

    pub fn configure_phase(&mut self, config: PhaseGateConfig) {
        self.phase_configs.insert(config.phase, config);
    }

    pub fn evaluate_all(&self, phase: PhaseId, context: &GateContext) -> Result<Vec<GateResult>> {
        let mut results = Vec::new();

        for gate in &self.standard_gates {
            let applicable = gate.applicable_phases();
            if applicable.is_empty() || applicable.contains(&phase) {
                results.push(gate.evaluate(context)?);
            }
        }

        for gate in self.custom_gates.values() {
            let applicable = gate.config.applicable_phases.clone();
            if applicable.is_empty() || applicable.contains(&phase) {
                results.push(gate.evaluate(context)?);
            }
        }

        Ok(results)
    }

    pub fn evaluate_gate(&self, gate_id: &str, context: &GateContext) -> Result<GateResult> {
        if let Some(gate) = self.custom_gates.get(gate_id) {
            return gate.evaluate(context);
        }

        for gate in &self.standard_gates {
            if gate.id() == gate_id {
                return gate.evaluate(context);
            }
        }

        Err(NexusError::GateFailed {
            gate: gate_id.to_string(),
            message: "Gate not found".to_string(),
        })
    }

    pub fn check_phase_requirements(&self, phase: PhaseId, results: &[GateResult]) -> Result<bool> {
        let config = self.phase_configs.get(&phase);

        if let Some(config) = config {
            let passed_count = results.iter().filter(|r| r.passed).count();
            if passed_count < config.required_pass_count {
                return Ok(false);
            }

            if !config.allow_warnings {
                let has_blocking_failure = results
                    .iter()
                    .any(|r| !r.passed && r.severity == GateSeverity::Blocking);
                if has_blocking_failure {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    #[must_use]
    pub fn gate_count(&self) -> usize {
        self.standard_gates.len() + self.custom_gates.len()
    }

    #[must_use]
    pub fn custom_gate_count(&self) -> usize {
        self.custom_gates.len()
    }
}

impl Default for AdvancedGateEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GateBuilder {
    id: String,
    name: String,
    description: String,
    condition: Option<GateConditionSpec>,
    severity: ExtendedGateSeverity,
    actions: GateAction,
    phases: Vec<PhaseId>,
}

impl GateBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            description: String::new(),
            condition: None,
            severity: ExtendedGateSeverity::Blocking,
            actions: GateAction::new(),
            phases: Vec::new(),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    #[must_use]
    pub fn condition(mut self, condition: GateConditionSpec) -> Self {
        self.condition = Some(condition);
        self
    }

    #[must_use]
    pub fn severity(mut self, severity: ExtendedGateSeverity) -> Self {
        self.severity = severity;
        self
    }

    #[must_use]
    pub fn for_phase(mut self, phase: PhaseId) -> Self {
        self.phases.push(phase);
        self
    }

    #[must_use]
    pub fn on_fail_block(mut self) -> Self {
        self.actions.on_fail.push(GateActionType::BlockTransition);
        self
    }

    pub fn build(self) -> Result<CustomGate> {
        let condition = self
            .condition
            .ok_or_else(|| NexusError::LockError("Gate condition is required".to_string()))?;

        let config = CustomGateConfig {
            id: self.id,
            name: self.name,
            description: self.description,
            condition,
            severity: self.severity,
            actions: self.actions,
            applicable_phases: self.phases,
            timeout_ms: None,
            retry_on_fail: false,
            metadata: serde_json::json!({}),
        };

        Ok(CustomGate::new(config))
    }
}

#[must_use]
pub fn create_default_phase_configs() -> HashMap<PhaseId, PhaseGateConfig> {
    let mut configs = HashMap::new();

    configs.insert(
        PhaseId(0),
        PhaseGateConfig::new(PhaseId(0))
            .with_exit_gate("domain_identified")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(1),
        PhaseGateConfig::new(PhaseId(1))
            .with_exit_gate("environment_reproducible")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(2),
        PhaseGateConfig::new(PhaseId(2))
            .with_exit_gate("requirements_complete")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(3),
        PhaseGateConfig::new(PhaseId(3))
            .with_exit_gate("yellow_paper_complete")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(6),
        PhaseGateConfig::new(PhaseId(6))
            .with_exit_gate("blue_paper_complete")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(13),
        PhaseGateConfig::new(PhaseId(13))
            .with_exit_gate("compilation")
            .with_required_pass_count(1)
            .with_allow_warnings(false),
    );

    configs.insert(
        PhaseId(16),
        PhaseGateConfig::new(PhaseId(16))
            .with_exit_gate("test_coverage")
            .with_required_pass_count(1),
    );

    configs.insert(
        PhaseId(18),
        PhaseGateConfig::new(PhaseId(18))
            .with_exit_gate("deployment_readiness")
            .with_exit_gate("security_scan")
            .with_required_pass_count(2)
            .with_allow_warnings(false),
    );

    configs
}

#[must_use]
pub fn create_sample_custom_gates() -> Vec<CustomGate> {
    vec![
        CustomGate::new(
            CustomGateConfig::new(
                "min_coverage_80",
                GateConditionSpec::simple(GateCondition::new(
                    "test_coverage",
                    ConditionOperator::GreaterThanOrEqual,
                    serde_json::json!(0.8),
                )),
            )
            .with_name("Minimum Test Coverage 80%")
            .with_description("Ensures test coverage meets the 80% threshold")
            .with_severity(ExtendedGateSeverity::Blocking)
            .for_phases(vec![PhaseId(16), PhaseId(17)]),
        ),
        CustomGate::new(
            CustomGateConfig::new(
                "no_critical_vulnerabilities",
                GateConditionSpec::simple(GateCondition::new(
                    "critical_vulnerabilities",
                    ConditionOperator::Equals,
                    serde_json::json!(0),
                )),
            )
            .with_name("No Critical Vulnerabilities")
            .with_description("Ensures no critical security vulnerabilities are present")
            .with_severity(ExtendedGateSeverity::Blocking)
            .for_phases(vec![PhaseId(8), PhaseId(18)]),
        ),
        CustomGate::new(
            CustomGateConfig::new(
                "documentation_complete",
                GateConditionSpec::composite(CompositeCondition::and(vec![
                    GateConditionSpec::simple(GateCondition::new(
                        "readme_exists",
                        ConditionOperator::Equals,
                        serde_json::json!(true),
                    )),
                    GateConditionSpec::simple(GateCondition::new(
                        "api_docs_exist",
                        ConditionOperator::Equals,
                        serde_json::json!(true),
                    )),
                ])),
            )
            .with_name("Documentation Complete")
            .with_description("Ensures README and API documentation exist")
            .with_severity(ExtendedGateSeverity::Warning)
            .for_phases(vec![PhaseId(14)]),
        ),
        CustomGate::new(
            CustomGateConfig::new(
                "performance_baseline_met",
                GateConditionSpec::composite(CompositeCondition::or(vec![
                    GateConditionSpec::simple(GateCondition::new(
                        "latency_p99",
                        ConditionOperator::LessThan,
                        serde_json::json!(100),
                    )),
                    GateConditionSpec::simple(GateCondition::new(
                        "performance_exempt",
                        ConditionOperator::Equals,
                        serde_json::json!(true),
                    )),
                ])),
            )
            .with_name("Performance Baseline Met")
            .with_description("P99 latency must be under 100ms or project is exempt")
            .with_severity(ExtendedGateSeverity::Blocking)
            .for_phases(vec![PhaseId(10)]),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::ArtifactTracker;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_test_context() -> GateContext {
        GateContext::new(
            PhaseId(0),
            Arc::new(ArtifactTracker::in_memory()),
            PathBuf::from("/tmp"),
        )
    }

    #[test]
    fn test_gate_condition_equals() {
        let condition = GateCondition::new(
            "domain",
            ConditionOperator::Equals,
            serde_json::json!("test"),
        );
        let context = create_test_context().with_metadata("domain", serde_json::json!("test"));

        assert!(condition.evaluate(&context));

        let context2 = create_test_context().with_metadata("domain", serde_json::json!("other"));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_gate_condition_numeric() {
        let condition = GateCondition::new(
            "coverage",
            ConditionOperator::GreaterThanOrEqual,
            serde_json::json!(0.8),
        );

        let context = create_test_context().with_metadata("coverage", serde_json::json!(0.9));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context().with_metadata("coverage", serde_json::json!(0.7));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_gate_condition_contains() {
        let condition = GateCondition::new(
            "tags",
            ConditionOperator::Contains,
            serde_json::json!("important"),
        );

        let context = create_test_context()
            .with_metadata("tags", serde_json::json!(["critical", "important"]));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context().with_metadata("tags", serde_json::json!(["minor"]));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_composite_condition_and() {
        let condition = CompositeCondition::and(vec![
            GateConditionSpec::simple(GateCondition::new(
                "field1",
                ConditionOperator::Equals,
                serde_json::json!(true),
            )),
            GateConditionSpec::simple(GateCondition::new(
                "field2",
                ConditionOperator::Equals,
                serde_json::json!(true),
            )),
        ]);

        let context = create_test_context()
            .with_metadata("field1", serde_json::json!(true))
            .with_metadata("field2", serde_json::json!(true));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context()
            .with_metadata("field1", serde_json::json!(true))
            .with_metadata("field2", serde_json::json!(false));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_composite_condition_or() {
        let condition = CompositeCondition::or(vec![
            GateConditionSpec::simple(GateCondition::new(
                "field1",
                ConditionOperator::Equals,
                serde_json::json!(true),
            )),
            GateConditionSpec::simple(GateCondition::new(
                "field2",
                ConditionOperator::Equals,
                serde_json::json!(true),
            )),
        ]);

        let context = create_test_context()
            .with_metadata("field1", serde_json::json!(true))
            .with_metadata("field2", serde_json::json!(false));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context()
            .with_metadata("field1", serde_json::json!(false))
            .with_metadata("field2", serde_json::json!(false));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_composite_condition_not() {
        let condition = CompositeCondition::not(GateConditionSpec::simple(GateCondition::new(
            "field",
            ConditionOperator::Equals,
            serde_json::json!(true),
        )));

        let context = create_test_context().with_metadata("field", serde_json::json!(false));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context().with_metadata("field", serde_json::json!(true));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_custom_gate_config() {
        let config = CustomGateConfig::new(
            "test_gate",
            GateConditionSpec::simple(GateCondition::new(
                "value",
                ConditionOperator::Equals,
                serde_json::json!(42),
            )),
        )
        .with_name("Test Gate")
        .with_severity(ExtendedGateSeverity::Blocking)
        .for_phases(vec![PhaseId(5), PhaseId(6)]);

        assert_eq!(config.id, "test_gate");
        assert_eq!(config.name, "Test Gate");
        assert_eq!(config.severity, ExtendedGateSeverity::Blocking);
        assert_eq!(config.applicable_phases.len(), 2);
    }

    #[test]
    fn test_custom_gate_evaluate() {
        let gate = CustomGate::new(
            CustomGateConfig::new(
                "test_gate",
                GateConditionSpec::simple(GateCondition::new(
                    "domain",
                    ConditionOperator::Equals,
                    serde_json::json!("test"),
                )),
            )
            .with_name("Test Gate"),
        );

        let context_pass = create_test_context().with_metadata("domain", serde_json::json!("test"));
        let result = gate.evaluate(&context_pass).unwrap();
        assert!(result.passed);

        let context_fail =
            create_test_context().with_metadata("domain", serde_json::json!("other"));
        let result = gate.evaluate(&context_fail).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_custom_gate_with_predicate() {
        let gate = CustomGate::new(CustomGateConfig::new(
            "custom_predicate",
            GateConditionSpec::simple(GateCondition::new(
                "field",
                ConditionOperator::Equals,
                serde_json::json!(true),
            )),
        ))
        .with_predicate(Box::new(|ctx| {
            Ok(ctx
                .metadata
                .get("custom")
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
        }));

        let context = create_test_context().with_metadata("custom", serde_json::json!(true));
        let result = gate.evaluate(&context).unwrap();
        assert!(result.passed);

        let context2 = create_test_context().with_metadata("custom", serde_json::json!(false));
        let result = gate.evaluate(&context2).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_phase_gate_config() {
        let config = PhaseGateConfig::new(PhaseId(5))
            .with_entry_gate("entry_check")
            .with_exit_gate("exit_check")
            .with_required_pass_count(2)
            .with_allow_warnings(false);

        assert_eq!(config.phase, PhaseId(5));
        assert_eq!(config.entry_gates.len(), 1);
        assert_eq!(config.exit_gates.len(), 1);
        assert_eq!(config.required_pass_count, 2);
        assert!(!config.allow_warnings);
    }

    #[test]
    fn test_advanced_gate_evaluator() {
        let mut evaluator = AdvancedGateEvaluator::new();

        evaluator.register_custom(CustomGate::new(
            CustomGateConfig::new(
                "coverage_gate",
                GateConditionSpec::simple(GateCondition::new(
                    "test_coverage",
                    ConditionOperator::GreaterThanOrEqual,
                    serde_json::json!(0.8),
                )),
            )
            .for_phases(vec![PhaseId(16)]),
        ));

        let context = create_test_context().with_metadata("test_coverage", serde_json::json!(0.9));

        let results = evaluator.evaluate_all(PhaseId(16), &context).unwrap();
        assert!(!results.is_empty());

        let passed = results.iter().filter(|r| r.passed).count();
        assert!(passed > 0);
    }

    #[test]
    fn test_gate_builder() {
        let gate = GateBuilder::new("test_gate")
            .name("Test Gate")
            .description("A test gate")
            .condition(GateConditionSpec::simple(GateCondition::new(
                "value",
                ConditionOperator::Equals,
                serde_json::json!(42),
            )))
            .severity(ExtendedGateSeverity::Blocking)
            .for_phase(PhaseId(5))
            .on_fail_block()
            .build()
            .unwrap();

        assert_eq!(gate.id(), "test_gate");
        assert_eq!(gate.severity(), ExtendedGateSeverity::Blocking);
    }

    #[test]
    fn test_gate_builder_missing_condition() {
        let result = GateBuilder::new("test").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_create_default_phase_configs() {
        let configs = create_default_phase_configs();

        assert!(configs.contains_key(&PhaseId(0)));
        assert!(configs.contains_key(&PhaseId(13)));
        assert!(configs.contains_key(&PhaseId(18)));
    }

    #[test]
    fn test_create_sample_custom_gates() {
        let gates = create_sample_custom_gates();

        assert!(!gates.is_empty());
        assert!(gates.iter().any(|g| g.id() == "min_coverage_80"));
        assert!(gates
            .iter()
            .any(|g| g.id() == "no_critical_vulnerabilities"));
    }

    #[test]
    fn test_condition_operator_display() {
        assert_eq!(format!("{}", ConditionOperator::Equals), "==");
        assert_eq!(format!("{}", ConditionOperator::NotEquals), "!=");
        assert_eq!(format!("{}", ConditionOperator::GreaterThan), ">");
        assert_eq!(format!("{}", ConditionOperator::Contains), "contains");
    }

    #[test]
    fn test_gate_action() {
        let action = GateAction::new()
            .on_pass(GateActionType::Log {
                level: "info".to_string(),
                message: "Gate passed".to_string(),
            })
            .on_fail(GateActionType::BlockTransition);

        assert_eq!(action.on_pass.len(), 1);
        assert_eq!(action.on_fail.len(), 1);
    }

    #[test]
    fn test_advanced_evaluator_phase_requirements() {
        let mut evaluator = AdvancedGateEvaluator::new();

        evaluator.configure_phase(
            PhaseGateConfig::new(PhaseId(5))
                .with_required_pass_count(2)
                .with_allow_warnings(false),
        );

        let results = vec![
            GateResult::passed("gate1", "OK", PhaseId(5)),
            GateResult::passed("gate2", "OK", PhaseId(5)),
        ];

        assert!(evaluator
            .check_phase_requirements(PhaseId(5), &results)
            .unwrap());

        let results_with_warning = vec![
            GateResult::passed("gate1", "OK", PhaseId(5)),
            GateResult::warning("gate2", "Warning", PhaseId(5)),
        ];

        assert!(!evaluator
            .check_phase_requirements(PhaseId(5), &results_with_warning)
            .unwrap());
    }

    #[test]
    fn test_regex_condition() {
        let condition = GateCondition::new(
            "version",
            ConditionOperator::Matches,
            serde_json::json!(r"^\d+\.\d+\.\d+$"),
        );

        let context = create_test_context().with_metadata("version", serde_json::json!("1.2.3"));
        assert!(condition.evaluate(&context));

        let context2 = create_test_context().with_metadata("version", serde_json::json!("v1.2.3"));
        assert!(!condition.evaluate(&context2));
    }

    #[test]
    fn test_extended_gate_severity() {
        assert_eq!(
            GateSeverity::from(ExtendedGateSeverity::Blocking),
            GateSeverity::Blocking
        );
        assert_eq!(
            GateSeverity::from(ExtendedGateSeverity::Warning),
            GateSeverity::Warning
        );
        assert_eq!(
            GateSeverity::from(ExtendedGateSeverity::Information),
            GateSeverity::Information
        );
    }
}
