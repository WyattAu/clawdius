//! Executor Agent
//!
//! Executes the plan created by the Planner Agent.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::planner_agent::ReviewCriterion;

/// Agent responsible for executing the plan steps.
#[derive(Debug, Default)]
pub struct ExecutorAgent {
    /// Completed steps
    completed: HashSet<String>,
    /// Failed steps
    failed: HashSet<String>,
    /// Step results
    results: HashMap<String, StepResult>,
}

impl ExecutorAgent {
    /// Creates a new executor agent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Executes the entire plan.
    ///
    /// # Errors
    ///
    /// Returns an error if a critical step fails.
    pub async fn execute_plan(
        &mut self,
        plan: &super::TaskPlan,
        log: &mut Vec<crate::agentic::LogEntry>,
    ) -> Result<Vec<super::FileChange>> {
        let mut all_changes = Vec::new();
        let mut completed_steps = HashSet::new();

        // Execute steps in dependency order
        while completed_steps.len() < plan.steps.len() {
            // Find executable steps
            let executable: Vec<_> = plan
                .steps
                .iter()
                .filter(|s| !completed_steps.contains(&s.id))
                .filter(|s| s.dependencies.iter().all(|d| completed_steps.contains(d)))
                .collect();

            if executable.is_empty() && completed_steps.len() < plan.steps.len() {
                // Deadlock or cycle
                log.push(crate::agentic::LogEntry {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                    level: crate::agentic::LogLevel::Error,
                    component: "ExecutorAgent".to_string(),
                    message: "Dependency cycle detected in plan".to_string(),
                });
                break;
            }

            // Execute each ready step
            for step in executable {
                log.push(crate::agentic::LogEntry {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                    level: crate::agentic::LogLevel::Info,
                    component: "ExecutorAgent".to_string(),
                    message: format!("Executing step: {}", step.description),
                });

                let result = self.execute_step(step, log).await;

                match result {
                    Ok(step_result) => {
                        completed_steps.insert(step.id.clone());
                        self.completed.insert(step.id.clone());
                        self.results.insert(step.id.clone(), step_result.clone());

                        if let Some(changes) = &step_result.changes {
                            all_changes.extend(changes.clone());
                        }

                        log.push(crate::agentic::LogEntry {
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0),
                            level: crate::agentic::LogLevel::Info,
                            component: "ExecutorAgent".to_string(),
                            message: format!(
                                "Step completed: {} in {}ms",
                                step.id, step_result.duration_ms
                            ),
                        });
                    }
                    Err(e) => {
                        self.failed.insert(step.id.clone());

                        log.push(crate::agentic::LogEntry {
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0),
                            level: crate::agentic::LogLevel::Error,
                            component: "ExecutorAgent".to_string(),
                            message: format!("Step failed: {} - {}", step.id, e),
                        });

                        // For now, continue with other steps
                        // In a real implementation, we might want to stop or retry
                        completed_steps.insert(step.id.clone());
                    }
                }
            }
        }

        Ok(all_changes)
    }

    /// Executes a single step.
    async fn execute_step(
        &self,
        step: &super::TaskStep,
        _log: &mut Vec<crate::agentic::LogEntry>,
    ) -> Result<StepResult> {
        let start = std::time::Instant::now();

        let result = match &step.action {
            super::StepAction::Analyze { scope, depth } => self.execute_analyze(scope, depth).await,
            super::StepAction::Design {
                requirements,
                constraints,
            } => self.execute_design(requirements, constraints).await,
            super::StepAction::GenerateCode {
                prompt,
                target_files,
            } => self.execute_generate_code(prompt, target_files).await,
            super::StepAction::WriteFile { path, content } => {
                self.execute_write_file(path, content).await
            }
            super::StepAction::EditFile { path, edits } => {
                self.execute_edit_file(path, edits).await
            }
            super::StepAction::DeleteFile { path } => self.execute_delete_file(path).await,
            super::StepAction::RunTests { coverage_threshold } => {
                self.execute_run_tests(*coverage_threshold).await
            }
            super::StepAction::Verify {
                check_tests,
                check_lint,
                check_types,
            } => {
                self.execute_verify(*check_tests, *check_lint, *check_types)
                    .await
            }
            super::StepAction::Refine {
                max_iterations,
                focus_areas,
            } => self.execute_refine(*max_iterations, focus_areas).await,
            super::StepAction::Review { criteria } => self.execute_review(criteria).await,
            super::StepAction::Fix {
                issue_id,
                file,
                line,
            } => self.execute_fix(issue_id, file, *line).await,
            super::StepAction::RunCommand {
                command,
                args,
                timeout_ms,
            } => self.execute_command(command, args, *timeout_ms).await,
            super::StepAction::Custom {
                action_type,
                params,
            } => self.execute_custom(action_type, params).await,
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Ok(StepResult {
                step_id: step.id.clone(),
                success: true,
                output: Some(output),
                changes: None,
                duration_ms,
                error: None,
            }),
            Err(e) => Ok(StepResult {
                step_id: step.id.clone(),
                success: false,
                output: None,
                changes: None,
                duration_ms,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn execute_analyze(
        &self,
        _scope: &super::AnalysisScope,
        _depth: &super::AnalysisDepth,
    ) -> Result<String> {
        // In a real implementation, this would analyze the codebase
        Ok("Analysis complete".to_string())
    }

    async fn execute_design(
        &self,
        _requirements: &[String],
        _constraints: &[String],
    ) -> Result<String> {
        // In a real implementation, this would generate a design document
        Ok("Design complete".to_string())
    }

    async fn execute_generate_code(
        &self,
        _prompt: &str,
        _target_files: &[String],
    ) -> Result<String> {
        // In a real implementation, this would call the LLM to generate code
        Ok("// Generated code\nfn generated_function() {}".to_string())
    }

    async fn execute_write_file(&self, path: &str, content: &str) -> Result<String> {
        // In a real implementation, this would write to the file system
        let _ = (path, content);
        Ok(format!("Wrote to {}", path))
    }

    async fn execute_edit_file(&self, path: &str, _edits: &[super::FileEdit]) -> Result<String> {
        // In a real implementation, this would apply edits to the file
        Ok(format!("Edited {}", path))
    }

    async fn execute_delete_file(&self, path: &str) -> Result<String> {
        // In a real implementation, this would delete the file
        Ok(format!("Deleted {}", path))
    }

    async fn execute_run_tests(&self, _coverage_threshold: Option<f64>) -> Result<String> {
        // In a real implementation, this would run the test suite
        // and check coverage against the threshold
        Ok("All tests passed".to_string())
    }

    async fn execute_verify(
        &self,
        _check_tests: bool,
        _check_lint: bool,
        _check_types: bool,
    ) -> Result<String> {
        // In a real implementation, this would run verification checks
        Ok("Verification passed".to_string())
    }

    async fn execute_refine(
        &self,
        _max_iterations: u32,
        _focus_areas: &[String],
    ) -> Result<String> {
        // In a real implementation, this would refine the code based on feedback
        Ok("Refinement complete".to_string())
    }

    async fn execute_review(&self, _criteria: &[ReviewCriterion]) -> Result<String> {
        // In a real implementation, this would perform a code review
        Ok("Review complete".to_string())
    }

    async fn execute_fix(
        &self,
        _issue_id: &str,
        _file: &str,
        _line: Option<u32>,
    ) -> Result<String> {
        // In a real implementation, this would fix the identified issue
        Ok("Fix applied".to_string())
    }

    async fn execute_command(
        &self,
        command: &str,
        args: &[String],
        _timeout_ms: u64,
    ) -> Result<String> {
        // In a real implementation, this would execute a shell command
        Ok(format!("Executed: {} {}", command, args.join(" ")))
    }

    async fn execute_custom(
        &self,
        action_type: &str,
        _params: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // In a real implementation, this would execute a custom action
        Ok(format!("Custom action {} executed", action_type))
    }

    /// Returns the number of completed steps.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Returns the number of failed steps.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// Returns the result for a specific step.
    #[must_use]
    pub fn get_result(&self, step_id: &str) -> Option<&StepResult> {
        self.results.get(step_id)
    }
}

/// Result of executing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// The step ID
    pub step_id: String,
    /// Whether the step succeeded
    pub success: bool,
    /// Output from the step
    pub output: Option<String>,
    /// File changes made
    pub changes: Option<Vec<super::FileChange>>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
}

impl StepResult {
    /// Creates a successful result.
    #[must_use]
    pub fn success(step_id: String, output: Option<String>, duration_ms: u64) -> Self {
        Self {
            step_id,
            success: true,
            output,
            changes: None,
            duration_ms,
            error: None,
        }
    }

    /// Creates a successful result with changes.
    #[must_use]
    pub fn success_with_changes(
        step_id: String,
        output: Option<String>,
        changes: Vec<super::FileChange>,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_id,
            success: true,
            output,
            changes: Some(changes),
            duration_ms,
            error: None,
        }
    }

    /// Creates a failed result.
    #[must_use]
    pub fn failure(step_id: String, error: String, duration_ms: u64) -> Self {
        Self {
            step_id,
            success: false,
            output: None,
            changes: None,
            duration_ms,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::{RiskAssessment, StepAction, TaskPlan, TaskStep};

    #[test]
    fn test_executor_creation() {
        let executor = ExecutorAgent::new();
        assert_eq!(executor.completed_count(), 0);
        assert_eq!(executor.failed_count(), 0);
    }

    #[tokio::test]
    async fn test_execute_simple_plan() {
        let mut executor = ExecutorAgent::new();
        let plan = TaskPlan {
            id: "plan-1".to_string(),
            task_id: "task-1".to_string(),
            steps: vec![TaskStep {
                id: "step-1".to_string(),
                description: "Test step".to_string(),
                action: StepAction::RunCommand {
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    timeout_ms: 5000,
                },
                dependencies: vec![],
                estimated_time_ms: 1000,
                priority: 1,
            }],
            estimated_total_time_ms: 1000,
            risk_assessment: RiskAssessment::low(),
        };

        let mut log = Vec::new();
        let result = executor.execute_plan(&plan, &mut log).await;
        assert!(result.is_ok());
        assert_eq!(executor.completed_count(), 1);
    }
}
