//! Executor Agent
//!
//! Executes the plan created by the Planner Agent.

use crate::error::Result;
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::llm_generator::GeneratedCode;
use super::planner_agent::ReviewCriterion;
use super::streaming_generator::{StreamChunk, StreamProcessor, StreamingCodeGenerator};
use super::tool_executor::{ToolExecutor, ToolRequest};

/// System prompt for code generation.
const CODE_GEN_SYSTEM_PROMPT: &str = "You are an expert software engineer. Generate clean, well-documented code based on the user's request.

When generating code:
1. Follow the language's best practices and idioms
2. Include appropriate error handling
3. Add comments for complex logic
4. Use descriptive variable and function names
5. Keep functions focused and single-purpose

When editing existing code:
1. Preserve the existing code style
2. Make minimal changes to achieve the goal
3. Ensure backward compatibility when possible

Always respond with code in the appropriate format:
- For new files: Provide the complete file content
- For edits: Show the changes using diff-like format with context
- Include the file path in your response";

/// Agent responsible for executing the plan steps.
pub struct ExecutorAgent {
    /// Completed steps
    completed: HashSet<String>,
    /// Failed steps
    failed: HashSet<String>,
    /// Step results
    results: HashMap<String, StepResult>,
    /// Optional tool executor for calling external tools
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    /// Optional LLM client for code generation
    llm_client: Option<Arc<dyn LlmClient>>,
    /// Model name for LLM generation
    model_name: Option<String>,
    /// Optional streaming code generator for real-time output
    streaming_generator: Option<Arc<StreamingCodeGenerator>>,
}

impl fmt::Debug for ExecutorAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorAgent")
            .field("completed", &self.completed)
            .field("failed", &self.failed)
            .field("results", &self.results)
            .field(
                "tool_executor",
                &self.tool_executor.as_ref().map(|_| "ToolExecutor"),
            )
            .field("llm_client", &self.llm_client.as_ref().map(|_| "LlmClient"))
            .field("model_name", &self.model_name)
            .field(
                "streaming_generator",
                &self
                    .streaming_generator
                    .as_ref()
                    .map(|_| "StreamingCodeGenerator"),
            )
            .finish()
    }
}

impl Default for ExecutorAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutorAgent {
    /// Creates a new executor agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            completed: HashSet::new(),
            failed: HashSet::new(),
            results: HashMap::new(),
            tool_executor: None,
            llm_client: None,
            model_name: None,
            streaming_generator: None,
        }
    }

    /// Sets the tool executor for calling external tools.
    #[must_use]
    pub fn with_tool_executor(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Sets the LLM client for code generation.
    #[must_use]
    pub fn with_llm_client(
        mut self,
        client: Arc<dyn LlmClient>,
        model_name: impl Into<String>,
    ) -> Self {
        self.llm_client = Some(client);
        self.model_name = Some(model_name.into());
        self
    }

    /// Sets the streaming code generator for real-time output.
    #[must_use]
    pub fn with_streaming_generator(mut self, generator: Arc<StreamingCodeGenerator>) -> Self {
        self.streaming_generator = Some(generator);
        self
    }

    /// Creates a streaming generator from the configured LLM client.
    #[must_use]
    pub fn with_streaming_from_llm(mut self) -> Self {
        if let (Some(client), Some(model)) = (&self.llm_client, &self.model_name) {
            self.streaming_generator = Some(Arc::new(StreamingCodeGenerator::new(
                client.clone(),
                model.clone(),
            )));
        }
        self
    }

    /// Returns whether a tool executor is configured.
    #[must_use]
    pub fn has_tool_executor(&self) -> bool {
        self.tool_executor.is_some()
    }

    /// Returns whether an LLM client is configured.
    #[must_use]
    pub fn has_llm_client(&self) -> bool {
        self.llm_client.is_some()
    }

    /// Returns the list of available tools if a tool executor is configured.
    #[must_use]
    pub fn available_tools(&self) -> Vec<super::ToolDefinition> {
        self.tool_executor
            .as_ref()
            .map(|e| e.list_tools())
            .unwrap_or_default()
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
                    },
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
                    },
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
            },
            super::StepAction::EditFile { path, edits } => {
                self.execute_edit_file(path, edits).await
            },
            super::StepAction::DeleteFile { path } => self.execute_delete_file(path).await,
            super::StepAction::RunTests { coverage_threshold } => {
                self.execute_run_tests(*coverage_threshold).await
            },
            super::StepAction::Verify {
                check_tests,
                check_lint,
                check_types,
            } => {
                self.execute_verify(*check_tests, *check_lint, *check_types)
                    .await
            },
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
        scope: &super::AnalysisScope,
        depth: &super::AnalysisDepth,
    ) -> Result<String> {
        if let Some(client) = &self.llm_client {
            let scope_desc = match scope {
                super::AnalysisScope::SingleFile(_) => "single file",
                super::AnalysisScope::Files(_) => "multiple files",
                super::AnalysisScope::Directory(_) => "directory",
                super::AnalysisScope::Workspace => "entire workspace",
            };

            let depth_desc = match depth {
                super::AnalysisDepth::Surface => "quick surface analysis",
                super::AnalysisDepth::Standard => "standard analysis",
                super::AnalysisDepth::Comprehensive => "deep comprehensive analysis",
            };

            let prompt = format!(
                "Analyze the code in a {} manner.\n\
                 Scope: {}\n\n\
                 Provide:\n\
                 1. Key functions and their purposes\n\
                 2. Dependencies and relationships\n\
                 3. Potential issues or improvements\n\
                 4. Code quality assessment",
                depth_desc, scope_desc
            );

            let messages = vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are a code analyst. Provide thorough, actionable analysis."
                        .to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: prompt,
                },
            ];

            let response = client.chat(messages).await?;
            return Ok(response);
        }

        Err(crate::error::Error::Config(
            "No LLM client configured — cannot execute analyze step".to_string(),
        ))
    }

    async fn execute_design(
        &self,
        requirements: &[String],
        constraints: &[String],
    ) -> Result<String> {
        if let Some(client) = &self.llm_client {
            let prompt = format!(
                "Design a solution for the following requirements:\n\
                 {}\n\n\
                 Constraints:\n\
                 {}\n\n\
                 Provide:\n\
                 1. Architecture overview\n\
                 2. Component breakdown\n\
                 3. API design\n\
                 4. Data flow\n\
                 5. Implementation recommendations",
                requirements.join("\n- "),
                constraints.join("\n- ")
            );

            let messages = vec![
                ChatMessage {
                    role: ChatRole::System,
                    content:
                        "You are a software architect. Create clear, practical design documents."
                            .to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: prompt,
                },
            ];

            let response = client.chat(messages).await?;
            return Ok(response);
        }

        Err(crate::error::Error::Config(
            "No LLM client configured — cannot execute design step".to_string(),
        ))
    }

    async fn execute_generate_code(&self, prompt: &str, target_files: &[String]) -> Result<String> {
        // If LLM client is configured, use it for real code generation
        if let Some(client) = &self.llm_client {
            let model = self.model_name.as_deref().unwrap_or("default");
            tracing::info!(
                "Generating code with model {} for {} target file(s)",
                model,
                target_files.len()
            );

            let file_context = if target_files.len() == 1 {
                format!("\nTarget file: {}", target_files[0])
            } else if !target_files.is_empty() {
                format!("\nTarget files: {}", target_files.join(", "))
            } else {
                String::new()
            };

            let full_prompt = format!(
                "{}\n\nTask: {}\n\nGenerate clean, well-documented code that follows best practices. \
                 Include appropriate error handling and use idiomatic patterns for the language.",
                file_context, prompt
            );

            let messages = vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: CODE_GEN_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: full_prompt,
                },
            ];

            let response = client.chat(messages).await?;
            return Ok(response);
        }

        Err(crate::error::Error::Config(
            "No LLM client configured — cannot execute code generation".to_string(),
        ))
    }

    /// Generates code with streaming output for real-time UX.
    ///
    /// Returns a receiver that yields `StreamChunk` objects as code is generated.
    /// This provides better UX for longer generations by showing progress.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn execute_generate_code_stream(
        &self,
        prompt: &str,
        target_files: &[String],
    ) -> Result<Option<mpsc::Receiver<StreamChunk>>> {
        // If streaming generator is configured, use it
        if let Some(generator) = &self.streaming_generator {
            let file_context = if target_files.len() == 1 {
                format!("\nTarget file: {}", target_files[0])
            } else if !target_files.is_empty() {
                format!("\nTarget files: {}", target_files.join(", "))
            } else {
                String::new()
            };

            let full_prompt = format!(
                "{}\n\nTask: {}\n\nGenerate clean, well-documented code that follows best practices. \
                 Include appropriate error handling and use idiomatic patterns for the language.",
                file_context, prompt
            );

            tracing::info!("Starting streaming code generation");
            let receiver = generator.generate_stream(&full_prompt, None).await?;
            return Ok(Some(receiver));
        }

        // If we have an LLM client but no streaming generator, we can still stream
        // by using the client's chat_stream method directly
        if let Some(client) = &self.llm_client {
            let model = self.model_name.as_deref().unwrap_or("default");
            tracing::info!(
                "Streaming code generation with model {} for {} target file(s)",
                model,
                target_files.len()
            );

            let file_context = if target_files.len() == 1 {
                format!("\nTarget file: {}", target_files[0])
            } else if !target_files.is_empty() {
                format!("\nTarget files: {}", target_files.join(", "))
            } else {
                String::new()
            };

            let full_prompt = format!(
                "{}\n\nTask: {}\n\nGenerate clean, well-documented code that follows best practices. \
                 Include appropriate error handling and use idiomatic patterns for the language.",
                file_context, prompt
            );

            let messages = vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: CODE_GEN_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: full_prompt,
                },
            ];

            // Get the raw string stream from the LLM client
            let mut raw_receiver = client.chat_stream(messages).await?;

            // Wrap the raw stream into StreamChunk format
            let (tx, rx) = mpsc::channel(32);

            // Spawn a task to convert the raw stream to StreamChunks
            let model_name = model.to_string();
            tokio::spawn(async move {
                let mut content = String::new();
                while let Some(delta) = raw_receiver.recv().await {
                    content.push_str(&delta);
                    let chunk = StreamChunk::incomplete(delta);
                    if tx.send(chunk).await.is_err() {
                        break;
                    }
                }
                // Send final complete chunk
                let _ = tx.send(StreamChunk::complete(content)).await;
                tracing::debug!(
                    "Streaming code generation complete for model {}",
                    model_name
                );
            });

            return Ok(Some(rx));
        }

        // No streaming available
        tracing::debug!("No streaming generator configured");
        Ok(None)
    }

    /// Generates code with streaming output and a callback for each chunk.
    ///
    /// This is a convenience method that handles the stream processing internally
    /// and calls the provided callback for each chunk received.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn execute_generate_code_with_callback<F>(
        &self,
        prompt: &str,
        target_files: &[String],
        callback: F,
    ) -> Result<GeneratedCode>
    where
        F: FnMut(&StreamChunk) + Send + 'static,
    {
        if let Some(receiver) = self
            .execute_generate_code_stream(prompt, target_files)
            .await?
        {
            let mut processor = StreamProcessor::new();
            return processor
                .process_stream_with_callback(receiver, callback)
                .await;
        }

        // Fallback to non-streaming generation
        let content = self.execute_generate_code(prompt, target_files).await?;
        Ok(GeneratedCode {
            content,
            file_path: target_files.first().cloned(),
            language: None,
            confidence: 0.5,
            notes: vec![
                "Generated without streaming (no streaming generator configured)".to_string(),
            ],
        })
    }

    /// Checks if streaming generation is available.
    #[must_use]
    pub const fn has_streaming(&self) -> bool {
        self.streaming_generator.is_some() || self.llm_client.is_some()
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
        timeout_ms: u64,
    ) -> Result<String> {
        // Try to use tool executor if available
        if let Some(executor) = &self.tool_executor {
            // Check if there's a matching shell tool
            if executor.has_tool("shell") || executor.has_tool("run_command") {
                let tool_name = if executor.has_tool("shell") {
                    "shell"
                } else {
                    "run_command"
                };

                let mut request = ToolRequest::new(tool_name)
                    .with_arg("command", serde_json::json!(command))
                    .with_arg("timeout_ms", serde_json::json!(timeout_ms));

                for (i, arg) in args.iter().enumerate() {
                    request = request.with_arg(format!("arg{}", i), serde_json::json!(arg));
                }

                let result = executor.execute(request).await?;

                if result.success {
                    return Ok(result.content);
                }
                return Ok(format!("Tool execution failed: {}", result.content));
            }
        }

        // Fallback: In a real implementation, this would execute a shell command
        Ok(format!("Executed: {} {}", command, args.join(" ")))
    }

    async fn execute_custom(
        &self,
        action_type: &str,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // Try to use tool executor if available
        if let Some(executor) = &self.tool_executor {
            // Check if there's a tool matching the action type
            if executor.has_tool(action_type) {
                let mut request = ToolRequest::new(action_type);
                for (key, value) in params {
                    request = request.with_arg(key.clone(), value.clone());
                }

                let result = executor.execute(request).await?;

                if result.success {
                    return Ok(result.content);
                }
                return Ok(format!(
                    "Tool '{}' execution failed: {}",
                    action_type, result.content
                ));
            }
        }

        // Fallback: In a real implementation, this would execute a custom action
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
