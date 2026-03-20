//! Planner Agent
//!
//! Analyzes task requests and creates execution plans.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent that creates execution plans from task requests.
#[derive(Default)]
pub struct PlannerAgent {
    /// Planning strategies registered
    /// (Intended for future extensibility when custom strategies are implemented)
    #[allow(dead_code)]
    strategies: HashMap<String, Box<dyn PlanningStrategy>>,
}

impl PlannerAgent {
    /// Creates a new planner agent.
    #[must_use]
    pub fn new() -> Self {
        let mut agent = Self {
            strategies: HashMap::new(),
        };
        agent.register_default_strategies();
        agent
    }

    /// Creates a simple plan for single-pass generation.
    pub async fn create_simple_plan(&self, request: &super::TaskRequest) -> Result<TaskPlan> {
        let steps = vec![TaskStep {
            id: "step-1".to_string(),
            description: format!("Generate code for: {}", request.description),
            action: StepAction::GenerateCode {
                prompt: request.description.clone(),
                target_files: request.target_files.clone(),
            },
            dependencies: vec![],
            estimated_time_ms: 30_000,
            priority: 1,
        }];

        Ok(TaskPlan {
            id: format!("plan-{}", uuid::Uuid::new_v4()),
            task_id: request.id.clone(),
            steps,
            estimated_total_time_ms: 30_000,
            risk_assessment: RiskAssessment::low(),
        })
    }

    /// Creates an iterative plan with verification loops.
    pub async fn create_iterative_plan(
        &self,
        request: &super::TaskRequest,
        _previous_verification: &super::VerificationResult,
    ) -> Result<TaskPlan> {
        let plan_id = format!("plan-{}", uuid::Uuid::new_v4());

        let steps = vec![
            TaskStep {
                id: format!("{plan_id}-step-1"),
                description: "Initial code generation".to_string(),
                action: StepAction::GenerateCode {
                    prompt: request.description.clone(),
                    target_files: request.target_files.clone(),
                },
                dependencies: vec![],
                estimated_time_ms: 30_000,
                priority: 1,
            },
            TaskStep {
                id: format!("{plan_id}-step-2"),
                description: "Verify generated code".to_string(),
                action: StepAction::Verify {
                    check_tests: true,
                    check_lint: true,
                    check_types: true,
                },
                dependencies: vec![format!("{plan_id}-step-1")],
                estimated_time_ms: 10_000,
                priority: 2,
            },
            TaskStep {
                id: format!("{plan_id}-step-3"),
                description: "Refine based on verification".to_string(),
                action: StepAction::Refine {
                    max_iterations: 3,
                    focus_areas: vec!["tests".to_string(), "types".to_string()],
                },
                dependencies: vec![format!("{plan_id}-step-2")],
                estimated_time_ms: 20_000,
                priority: 3,
            },
        ];

        Ok(TaskPlan {
            id: plan_id,
            task_id: request.id.clone(),
            steps,
            estimated_total_time_ms: 60_000,
            risk_assessment: RiskAssessment::medium(),
        })
    }

    /// Creates a comprehensive plan for agent-based generation.
    pub async fn create_comprehensive_plan(
        &self,
        request: &super::TaskRequest,
    ) -> Result<TaskPlan> {
        let plan_id = format!("plan-{}", uuid::Uuid::new_v4());

        // Analyze the task to understand its complexity
        let analysis = self.analyze_task(request).await?;

        let mut steps = Vec::new();
        let mut step_num = 1;

        // Step 1: Understand and analyze
        steps.push(TaskStep {
            id: format!("{plan_id}-step-{step_num}"),
            description: "Analyze codebase and understand context".to_string(),
            action: StepAction::Analyze {
                scope: AnalysisScope::from_target_files(&request.target_files),
                depth: AnalysisDepth::Comprehensive,
            },
            dependencies: vec![],
            estimated_time_ms: 15_000,
            priority: step_num,
        });
        step_num += 1;

        // Step 2: Design solution
        steps.push(TaskStep {
            id: format!("{plan_id}-step-{step_num}"),
            description: "Design solution architecture".to_string(),
            action: StepAction::Design {
                requirements: vec![request.description.clone()],
                constraints: request.context.constraints.clone(),
            },
            dependencies: vec![format!("{plan_id}-step-1")],
            estimated_time_ms: 20_000,
            priority: step_num,
        });
        step_num += 1;

        // Step 3: Generate code for each file
        for (_idx, file) in request.target_files.iter().enumerate() {
            steps.push(TaskStep {
                id: format!("{plan_id}-step-{step_num}"),
                description: format!("Generate code for {}", file),
                action: StepAction::GenerateCode {
                    prompt: format!("{} for file {}", request.description, file),
                    target_files: vec![file.clone()],
                },
                dependencies: vec![format!("{plan_id}-step-2")],
                estimated_time_ms: 30_000,
                priority: step_num,
            });
            step_num += 1;
        }

        // Step 4: Verify all changes
        let gen_deps: Vec<String> = steps
            .iter()
            .filter(|s| matches!(s.action, StepAction::GenerateCode { .. }))
            .map(|s| s.id.clone())
            .collect();

        steps.push(TaskStep {
            id: format!("{plan_id}-step-{step_num}"),
            description: "Verify all generated code".to_string(),
            action: StepAction::Verify {
                check_tests: true,
                check_lint: true,
                check_types: true,
            },
            dependencies: gen_deps,
            estimated_time_ms: 15_000,
            priority: step_num,
        });
        step_num += 1;

        // Step 5: Run tests
        steps.push(TaskStep {
            id: format!("{plan_id}-step-{step_num}"),
            description: "Run test suite".to_string(),
            action: StepAction::RunTests {
                coverage_threshold: Some(0.8),
            },
            dependencies: vec![format!("{plan_id}-step-{}", step_num - 1)],
            estimated_time_ms: 30_000,
            priority: step_num,
        });
        step_num += 1;

        // Step 6: Final review
        steps.push(TaskStep {
            id: format!("{plan_id}-step-{step_num}"),
            description: "Final review and cleanup".to_string(),
            action: StepAction::Review {
                criteria: vec![
                    ReviewCriterion::CodeQuality,
                    ReviewCriterion::TestCoverage,
                    ReviewCriterion::Documentation,
                ],
            },
            dependencies: vec![format!("{plan_id}-step-{}", step_num - 1)],
            estimated_time_ms: 10_000,
            priority: step_num,
        });

        let total_time: u64 = steps.iter().map(|s| s.estimated_time_ms).sum();

        Ok(TaskPlan {
            id: plan_id,
            task_id: request.id.clone(),
            steps,
            estimated_total_time_ms: total_time,
            risk_assessment: analysis.risk_assessment,
        })
    }

    /// Creates a plan to fix identified issues.
    pub async fn create_fix_plan(
        &self,
        issues: &[super::VerificationIssue],
        request: &super::TaskRequest,
    ) -> Result<TaskPlan> {
        let plan_id = format!("fix-plan-{}", uuid::Uuid::new_v4());

        let mut steps = Vec::new();

        for (idx, issue) in issues.iter().enumerate() {
            if !issue.can_fix {
                continue;
            }

            steps.push(TaskStep {
                id: format!("{plan_id}-fix-{}", idx + 1),
                description: format!("Fix: {}", issue.message),
                action: StepAction::Fix {
                    issue_id: issue.id.clone(),
                    file: issue.file.clone(),
                    line: issue.line,
                },
                dependencies: if idx > 0 {
                    vec![format!("{plan_id}-fix-{}", idx)]
                } else {
                    vec![]
                },
                estimated_time_ms: 15_000,
                priority: issue.severity.priority(),
            });
        }

        // Add verification step
        if !steps.is_empty() {
            let fix_ids: Vec<String> = steps.iter().map(|s| s.id.clone()).collect();
            steps.push(TaskStep {
                id: format!("{plan_id}-verify"),
                description: "Verify fixes".to_string(),
                action: StepAction::Verify {
                    check_tests: true,
                    check_lint: true,
                    check_types: true,
                },
                dependencies: fix_ids,
                estimated_time_ms: 10_000,
                priority: 100,
            });
        }

        let total_time: u64 = steps.iter().map(|s| s.estimated_time_ms).sum();

        Ok(TaskPlan {
            id: plan_id,
            task_id: request.id.clone(),
            steps,
            estimated_total_time_ms: total_time,
            risk_assessment: RiskAssessment::low(),
        })
    }

    async fn analyze_task(&self, request: &super::TaskRequest) -> Result<TaskAnalysis> {
        // Analyze task complexity
        let complexity = self.assess_complexity(request);

        // Assess risk
        let risk = self.assess_risk(request, &complexity);

        Ok(TaskAnalysis {
            complexity,
            risk_assessment: risk,
            recommended_approach: "agent-based".to_string(),
        })
    }

    fn assess_complexity(&self, request: &super::TaskRequest) -> TaskComplexity {
        let mut score = 0;

        // Number of files
        score += request.target_files.len().min(5);

        // Description length (proxy for complexity)
        if request.description.len() > 500 {
            score += 2;
        } else if request.description.len() > 200 {
            score += 1;
        }

        // Context complexity
        score += request.context.constraints.len().min(3);

        match score {
            0..=3 => TaskComplexity::Low,
            4..=6 => TaskComplexity::Medium,
            _ => TaskComplexity::High,
        }
    }

    fn assess_risk(
        &self,
        request: &super::TaskRequest,
        complexity: &TaskComplexity,
    ) -> RiskAssessment {
        let mut risk_factors = Vec::new();

        // Check for risky patterns in description
        let risky_patterns = [
            "delete",
            "remove",
            "drop",
            "truncate",
            "migration",
            "database",
            "production",
        ];
        for pattern in risky_patterns {
            if request.description.to_lowercase().contains(pattern) {
                risk_factors.push(format!("Contains risky keyword: {}", pattern));
            }
        }

        // Factor in complexity
        let complexity_multiplier = match complexity {
            TaskComplexity::Low => 1.0,
            TaskComplexity::Medium => 1.5,
            TaskComplexity::High => 2.0,
        };

        let base_score = risk_factors.len() as f64 * complexity_multiplier;

        match base_score {
            s if s < 1.0 => RiskAssessment::low(),
            s if s < 3.0 => RiskAssessment::medium(),
            _ => RiskAssessment::high(),
        }
        .with_factors(risk_factors)
    }

    fn register_default_strategies(&mut self) {
        // Register built-in strategies
        // In a real implementation, these would be actual strategy implementations
    }
}

/// A plan for executing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique plan identifier
    pub id: String,
    /// ID of the task this plan is for
    pub task_id: String,
    /// Steps in the plan
    pub steps: Vec<TaskStep>,
    /// Estimated total execution time
    pub estimated_total_time_ms: u64,
    /// Risk assessment for this plan
    pub risk_assessment: RiskAssessment,
}

impl TaskPlan {
    /// Returns the number of steps in the plan.
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns the next executable steps (dependencies satisfied).
    #[must_use]
    pub fn next_executable_steps(&self, completed: &[String]) -> Vec<&TaskStep> {
        self.steps
            .iter()
            .filter(|step| {
                !completed.contains(&step.id)
                    && step.dependencies.iter().all(|dep| completed.contains(dep))
            })
            .collect()
    }

    /// Returns true if all steps are complete.
    #[must_use]
    pub fn is_complete(&self, completed: &[String]) -> bool {
        self.steps.iter().all(|step| completed.contains(&step.id))
    }
}

/// A single step in a task plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    /// Unique step identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Action to perform
    pub action: StepAction,
    /// IDs of steps that must complete first
    pub dependencies: Vec<String>,
    /// Estimated time in milliseconds
    pub estimated_time_ms: u64,
    /// Priority (lower = higher priority)
    pub priority: u32,
}

/// Actions that can be performed in a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepAction {
    /// Analyze codebase
    Analyze {
        /// Scope of analysis
        scope: AnalysisScope,
        /// Depth of analysis
        depth: AnalysisDepth,
    },
    /// Design solution
    Design {
        /// Requirements to meet
        requirements: Vec<String>,
        /// Constraints to consider
        constraints: Vec<String>,
    },
    /// Generate code
    GenerateCode {
        /// Prompt for generation
        prompt: String,
        /// Target files
        target_files: Vec<String>,
    },
    /// Write a new file
    WriteFile {
        /// File path
        path: String,
        /// File content
        content: String,
    },
    /// Edit an existing file
    EditFile {
        /// File path
        path: String,
        /// Edits to apply
        edits: Vec<FileEdit>,
    },
    /// Delete a file
    DeleteFile {
        /// File path
        path: String,
    },
    /// Run a command
    RunCommand {
        /// Command to run
        command: String,
        /// Command arguments
        args: Vec<String>,
        /// Timeout in milliseconds
        timeout_ms: u64,
    },
    /// Verify changes
    Verify {
        /// Run tests
        check_tests: bool,
        /// Run linter
        check_lint: bool,
        /// Check types
        check_types: bool,
    },
    /// Run tests
    RunTests {
        /// Minimum coverage threshold
        coverage_threshold: Option<f64>,
    },
    /// Refine based on feedback
    Refine {
        /// Maximum iterations
        max_iterations: u32,
        /// Areas to focus on
        focus_areas: Vec<String>,
    },
    /// Fix an issue
    Fix {
        /// Issue to fix
        issue_id: String,
        /// File to fix
        file: String,
        /// Line number (optional)
        line: Option<u32>,
    },
    /// Review changes
    Review {
        /// Criteria to review
        criteria: Vec<ReviewCriterion>,
    },
    /// Custom action
    Custom {
        /// Action type identifier
        action_type: String,
        /// Action parameters
        params: std::collections::HashMap<String, serde_json::Value>,
    },
}

/// A file edit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    /// Original text to find
    pub old_text: String,
    /// New text to replace with
    pub new_text: String,
    /// Whether to replace all occurrences
    pub replace_all: bool,
}

/// Scope of codebase analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisScope {
    /// Single file
    SingleFile(String),
    /// Multiple files
    Files(Vec<String>),
    /// Directory
    Directory(String),
    /// Entire workspace
    Workspace,
}

impl AnalysisScope {
    /// Creates scope from target files.
    #[must_use]
    pub fn from_target_files(files: &[String]) -> Self {
        match files.len() {
            0 => Self::Workspace,
            1 => Self::SingleFile(files[0].clone()),
            _ => Self::Files(files.to_vec()),
        }
    }
}

/// Depth of analysis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDepth {
    /// Quick surface analysis
    Surface,
    /// Standard analysis
    Standard,
    /// Deep comprehensive analysis
    Comprehensive,
}

/// Review criteria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCriterion {
    /// Code quality
    CodeQuality,
    /// Test coverage
    TestCoverage,
    /// Documentation
    Documentation,
    /// Performance
    Performance,
    /// Security
    Security,
}

/// Risk assessment for a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Risk level (0.0 - 1.0)
    pub level: f64,
    /// Risk factors identified
    pub factors: Vec<String>,
    /// Mitigation suggestions
    pub mitigations: Vec<String>,
}

impl RiskAssessment {
    /// Creates a low risk assessment.
    #[must_use]
    pub fn low() -> Self {
        Self {
            level: 0.1,
            factors: Vec::new(),
            mitigations: Vec::new(),
        }
    }

    /// Creates a medium risk assessment.
    #[must_use]
    pub fn medium() -> Self {
        Self {
            level: 0.4,
            factors: Vec::new(),
            mitigations: vec!["Review changes carefully".to_string()],
        }
    }

    /// Creates a high risk assessment.
    #[must_use]
    pub fn high() -> Self {
        Self {
            level: 0.7,
            factors: Vec::new(),
            mitigations: vec![
                "Review changes carefully".to_string(),
                "Run full test suite".to_string(),
                "Consider rollback plan".to_string(),
            ],
        }
    }

    /// Adds risk factors.
    #[must_use]
    pub fn with_factors(self, factors: Vec<String>) -> Self {
        Self { factors, ..self }
    }

    /// Returns true if risk is low.
    #[must_use]
    pub const fn is_low(&self) -> bool {
        self.level < 0.3
    }

    /// Returns true if risk is high.
    #[must_use]
    pub const fn is_high(&self) -> bool {
        self.level >= 0.6
    }
}

/// Task complexity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskComplexity {
    /// Simple task
    Low,
    /// Moderate complexity
    Medium,
    /// Complex task
    High,
}

/// Analysis result for a task.
#[derive(Debug)]
#[allow(dead_code)]
struct TaskAnalysis {
    complexity: TaskComplexity,
    risk_assessment: RiskAssessment,
    recommended_approach: String,
}

/// Trait for planning strategies.
#[allow(dead_code)]
trait PlanningStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn can_handle(&self, request: &super::TaskRequest) -> bool;
    fn create_plan(&self, request: &super::TaskRequest) -> Result<TaskPlan>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::{
        ApplyWorkflow, GenerationMode, TaskContext, TaskRequest, TestExecutionStrategy, TrustLevel,
        VerificationResult,
    };

    fn make_test_request() -> TaskRequest {
        TaskRequest {
            id: "test-1".to_string(),
            description: "Add a hello function".to_string(),
            target_files: vec!["src/lib.rs".to_string()],
            mode: GenerationMode::single_pass(),
            test_strategy: TestExecutionStrategy::skip(),
            apply_workflow: ApplyWorkflow::preview_only(),
            context: TaskContext::default(),
            trust_level: TrustLevel::medium(),
        }
    }

    #[tokio::test]
    async fn test_create_simple_plan() {
        let planner = PlannerAgent::new();
        let request = make_test_request();
        let plan = planner.create_simple_plan(&request).await.unwrap();

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.task_id, "test-1");
    }

    #[tokio::test]
    async fn test_create_iterative_plan() {
        let planner = PlannerAgent::new();
        let request = make_test_request();
        let verification = VerificationResult::default();
        let plan = planner
            .create_iterative_plan(&request, &verification)
            .await
            .unwrap();

        assert!(!plan.steps.is_empty());
        assert!(plan.steps.len() >= 3);
    }

    #[tokio::test]
    async fn test_create_comprehensive_plan() {
        let planner = PlannerAgent::new();
        let request = make_test_request();
        let plan = planner.create_comprehensive_plan(&request).await.unwrap();

        assert!(!plan.steps.is_empty());
        // Should have analyze, design, generate, verify, test, review
        assert!(plan.steps.len() >= 5);
    }

    #[test]
    fn test_plan_next_executable_steps() {
        let plan = TaskPlan {
            id: "plan-1".to_string(),
            task_id: "task-1".to_string(),
            steps: vec![
                TaskStep {
                    id: "step-1".to_string(),
                    description: "First".to_string(),
                    action: StepAction::Analyze {
                        scope: AnalysisScope::Workspace,
                        depth: AnalysisDepth::Standard,
                    },
                    dependencies: vec![],
                    estimated_time_ms: 1000,
                    priority: 1,
                },
                TaskStep {
                    id: "step-2".to_string(),
                    description: "Second".to_string(),
                    action: StepAction::GenerateCode {
                        prompt: "test".to_string(),
                        target_files: vec![],
                    },
                    dependencies: vec!["step-1".to_string()],
                    estimated_time_ms: 1000,
                    priority: 2,
                },
            ],
            estimated_total_time_ms: 2000,
            risk_assessment: RiskAssessment::low(),
        };

        let next = plan.next_executable_steps(&[]);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].id, "step-1");

        let next = plan.next_executable_steps(&["step-1".to_string()]);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].id, "step-2");
    }
}
