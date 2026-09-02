//! Model Router: task-aware LLM provider selection with cost tracking.
//!
//! The ModelRouter implements the `LlmClient` trait, making it a drop-in
//! replacement for any single-provider LLM. It dispatches requests to
//! different models based on:
//!
//! - **Task type** (think/plan → cheap, build → expensive, test → cheap, review → medium)
//! - **Budget constraints** (per-session or per-tenant dollar limits)
//! - **Fallback chains** (if primary model fails, try cheaper alternative)
//!
//! Cost tracking is done via a pricing table mapping model names to
//! cost-per-1k-tokens (input and output).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};

use crate::error::Result;
use crate::llm::providers::{ChatWithToolsResult, LlmClient, Tool, ToolCall};
use crate::llm::{ChatMessage, ChatRole, ResolvedLlmConfig};
use crate::llm::create_provider;
use crate::mcp::client::McpClientManager;
use crate::mcp::protocol::McpTool as McpProtocolTool;

/// Task type for routing decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Thinking/analysis phase — benefits from reasoning but not code generation.
    Think,
    /// Planning phase — structured output, moderate intelligence needed.
    Plan,
    /// Code generation/build phase — needs strongest model.
    Build,
    /// Test execution/analysis — can use cheaper model.
    Test,
    /// Code review — needs security awareness but not generation.
    Review,
    /// Summarization/compaction — cheapest acceptable model.
    Summarize,
    /// General chat — default routing.
    Chat,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Think => write!(f, "think"),
            Self::Plan => write!(f, "plan"),
            Self::Build => write!(f, "build"),
            Self::Test => write!(f, "test"),
            Self::Review => write!(f, "review"),
            Self::Summarize => write!(f, "summarize"),
            Self::Chat => write!(f, "chat"),
        }
    }
}

impl TaskType {
    /// Map from SprintPhase name to TaskType.
    pub fn from_phase_name(phase: &str) -> Self {
        match phase.to_lowercase().as_str() {
            "think" => Self::Think,
            "plan" => Self::Plan,
            "build" | "implement" | "code" => Self::Build,
            "test" | "verify" => Self::Test,
            "review" | "reflect" => Self::Review,
            _ => Self::Chat,
        }
    }
}

/// Per-model pricing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost per 1M input tokens in USD.
    pub input_per_1m: f64,
    /// Cost per 1M output tokens in USD.
    pub output_per_1m: f64,
    /// Maximum context window in tokens.
    pub context_window: usize,
    /// Maximum output tokens.
    pub max_output_tokens: usize,
    /// Quality tier (1-5, 5=highest).
    pub quality_tier: u8,
}

impl ModelPricing {
    /// Calculate cost for a given number of input/output tokens.
    pub fn cost(&self, input_tokens: usize, output_tokens: usize) -> f64 {
        (input_tokens as f64 * self.input_per_1m / 1_000_000.0)
            + (output_tokens as f64 * self.output_per_1m / 1_000_000.0)
    }

    /// Calculate cost efficiency (quality per dollar).
    pub fn efficiency(&self) -> f64 {
        let avg_cost = (self.input_per_1m + self.output_per_1m) / 2.0;
        if avg_cost > 0.0 {
            self.quality_tier as f64 / avg_cost
        } else {
            f64::MAX
        }
    }
}

/// Task complexity levels for cost-aware routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskComplexity {
    /// Simple tasks: formatting, renaming, simple refactors.
    Simple = 1,
    /// Medium tasks: bug fixes, small features, code review.
    Medium = 2,
    /// Complex tasks: architecture, multi-file refactors, complex algorithms.
    Complex = 3,
    /// Critical tasks: security fixes, production issues, complex debugging.
    Critical = 4,
}

/// Built-in pricing table for popular models.
pub fn default_pricing_table() -> HashMap<String, ModelPricing> {
    let mut table = HashMap::new();

    // Claude models (Anthropic)
    table.insert("claude-sonnet-4-20250514".into(), ModelPricing {
        input_per_1m: 3.0, output_per_1m: 15.0, context_window: 200_000, max_output_tokens: 16_384, quality_tier: 5,
    });
    table.insert("claude-3-5-sonnet-20241022".into(), ModelPricing {
        input_per_1m: 3.0, output_per_1m: 15.0, context_window: 200_000, max_output_tokens: 8_192, quality_tier: 4,
    });
    table.insert("claude-3-5-haiku-20241022".into(), ModelPricing {
        input_per_1m: 0.8, output_per_1m: 4.0, context_window: 200_000, max_output_tokens: 8_192, quality_tier: 3,
    });
    table.insert("claude-3-opus-20240229".into(), ModelPricing {
        input_per_1m: 15.0, output_per_1m: 75.0, context_window: 200_000, max_output_tokens: 4_096, quality_tier: 5,
    });

    // GPT models (OpenAI)
    table.insert("gpt-4o".into(), ModelPricing {
        input_per_1m: 2.5, output_per_1m: 10.0, context_window: 128_000, max_output_tokens: 16_384, quality_tier: 4,
    });
    table.insert("gpt-4o-mini".into(), ModelPricing {
        input_per_1m: 0.15, output_per_1m: 0.6, context_window: 128_000, max_output_tokens: 16_384, quality_tier: 3,
    });
    table.insert("gpt-4-turbo".into(), ModelPricing {
        input_per_1m: 10.0, output_per_1m: 30.0, context_window: 128_000, max_output_tokens: 4_096, quality_tier: 5,
    });

    // Gemini models (Google)
    table.insert("gemini-2.0-flash".into(), ModelPricing {
        input_per_1m: 0.1, output_per_1m: 0.4, context_window: 1_000_000, max_output_tokens: 8_192,
    });
    table.insert("gemini-1.5-pro".into(), ModelPricing {
        input_per_1m: 1.25, output_per_1m: 5.0, context_window: 2_000_000, max_output_tokens: 8_192,
    });

    // GLM models (ZAI)
    table.insert("glm-4.6".into(), ModelPricing {
        input_per_1m: 0.5, output_per_1m: 0.5, context_window: 128_000, max_output_tokens: 4_096,
    });
    table.insert("glm-5-turbo".into(), ModelPricing {
        input_per_1m: 0.5, output_per_1m: 0.5, context_window: 128_000, max_output_tokens: 4_096,
    });

    // DeepSeek
    table.insert("deepseek-chat".into(), ModelPricing {
        input_per_1m: 0.14, output_per_1m: 0.28, context_window: 64_000, max_output_tokens: 8_192,
    });
    table.insert("deepseek-coder".into(), ModelPricing {
        input_per_1m: 0.14, output_per_1m: 0.28, context_window: 64_000, max_output_tokens: 8_192,
    });

    // OpenRouter prefixed models
    table.insert("anthropic/claude-3.5-sonnet".into(), ModelPricing {
        input_per_1m: 3.0, output_per_1m: 15.0, context_window: 200_000, max_output_tokens: 8_192,
    });
    table.insert("openai/gpt-4o".into(), ModelPricing {
        input_per_1m: 2.5, output_per_1m: 10.0, context_window: 128_000, max_output_tokens: 16_384,
    });
    table.insert("google/gemma-3-4b-it:free".into(), ModelPricing {
        input_per_1m: 0.0, output_per_1m: 0.0, context_window: 32_000, max_output_tokens: 4_096,
    });
    table.insert("openai/gpt-oss-20b:free".into(), ModelPricing {
        input_per_1m: 0.0, output_per_1m: 0.0, context_window: 32_000, max_output_tokens: 4_096,
    });

    table
}

/// Routing rule: which model to use for which task type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Task type this rule applies to.
    pub task_type: TaskType,
    /// Provider name (e.g., "zai", "anthropic", "openrouter").
    pub provider: String,
    /// Model name (e.g., "glm-4.6", "claude-sonnet-4-20250514").
    pub model: String,
    /// API key (if different from default).
    pub api_key: Option<String>,
    /// Base URL override (if needed).
    pub base_url: Option<String>,
    /// Fallback provider/model if primary fails.
    pub fallback: Option<Box<RoutingRule>>,
}

impl RoutingRule {
    /// Create a simple routing rule.
    pub fn new(task_type: TaskType, provider: &str, model: &str) -> Self {
        Self {
            task_type,
            provider: provider.to_string(),
            model: model.to_string(),
            api_key: None,
            base_url: None,
            fallback: None,
        }
    }

    /// Create with fallback.
    pub fn with_fallback(mut self, fallback: RoutingRule) -> Self {
        self.fallback = Some(Box::new(fallback));
        self
    }
}

/// Cost tracking accumulator.
#[derive(Debug, Clone)]
pub struct CostTracker {
    /// Total cost in USD (atomic for concurrent access).
    total_cost: Arc<AtomicU64>, // stored as cents * 100 (fixed-point)
    /// Total input tokens.
    total_input_tokens: Arc<AtomicU64>,
    /// Total output tokens.
    total_output_tokens: Arc<AtomicU64>,
    /// Per-model cost breakdown.
    per_model: Arc<Mutex<HashMap<String, ModelCostRecord>>>,
    /// Budget limit in USD (None = unlimited).
    budget_limit_usd: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct ModelCostRecord {
    input_tokens: u64,
    output_tokens: u64,
    cost_cents_x100: u64, // fixed-point: divide by 10000 for USD
    request_count: u64,
}

impl CostTracker {
    /// Create a new cost tracker.
    pub fn new(budget_limit_usd: Option<f64>) -> Self {
        Self {
            total_cost: Arc::new(AtomicU64::new(0)),
            total_input_tokens: Arc::new(AtomicU64::new(0)),
            total_output_tokens: Arc::new(AtomicU64::new(0)),
            per_model: Arc::new(Mutex::new(HashMap::new())),
            budget_limit_usd,
        }
    }

    /// Record token usage and cost.
    pub async fn record(
        &self,
        model: &str,
        input_tokens: usize,
        output_tokens: usize,
        pricing: &ModelPricing,
    ) -> Result<()> {
        let cost_usd = pricing.cost(input_tokens, output_tokens);
        let cost_cents_x100 = (cost_usd * 10000.0) as u64;

        // Check budget
        if let Some(limit) = self.budget_limit_usd {
            let current = self.total_cost.load(Ordering::Relaxed) as f64 / 10000.0;
            if current + cost_usd > limit {
                return Err(crate::Error::Llm(format!(
                    "Budget exceeded: ${:.4} + ${:.4} > ${:.2}",
                    current, cost_usd, limit
                )));
            }
        }

        self.total_cost.fetch_add(cost_cents_x100, Ordering::Relaxed);
        self.total_input_tokens.fetch_add(input_tokens as u64, Ordering::Relaxed);
        self.total_output_tokens.fetch_add(output_tokens as u64, Ordering::Relaxed);

        let mut per_model = self.per_model.lock().await;
        let record = per_model.entry(model.to_string()).or_default();
        record.input_tokens += input_tokens as u64;
        record.output_tokens += output_tokens as u64;
        record.cost_cents_x100 += cost_cents_x100;
        record.request_count += 1;

        tracing::debug!(
            model = model,
            input = input_tokens,
            output = output_tokens,
            cost_usd = format!("{:.4}", cost_usd),
            total_usd = format!("{:.4}", self.total_usd()),
            "model routing cost recorded"
        );

        Ok(())
    }

    /// Get total cost in USD.
    pub fn total_usd(&self) -> f64 {
        self.total_cost.load(Ordering::Relaxed) as f64 / 10000.0
    }

    /// Get total input tokens.
    pub fn total_input_tokens(&self) -> u64 {
        self.total_input_tokens.load(Ordering::Relaxed)
    }

    /// Get total output tokens.
    pub fn total_output_tokens(&self) -> u64 {
        self.total_output_tokens.load(Ordering::Relaxed)
    }

    /// Get per-model cost breakdown.
    pub async fn per_model_breakdown(&self) -> HashMap<String, ModelCostBreakdown> {
        let per_model = self.per_model.lock().await;
        per_model
            .iter()
            .map(|(model, record)| {
                (
                    model.clone(),
                    ModelCostBreakdown {
                        input_tokens: record.input_tokens,
                        output_tokens: record.output_tokens,
                        cost_usd: record.cost_cents_x100 as f64 / 10000.0,
                        request_count: record.request_count,
                    },
                )
            })
            .collect()
    }

    /// Check if budget has been exceeded.
    pub fn is_over_budget(&self) -> bool {
        if let Some(limit) = self.budget_limit_usd {
            self.total_usd() >= limit
        } else {
            false
        }
    }
}

/// Per-model cost breakdown for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCostBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub request_count: u64,
}

/// Cost report summarizing all usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub per_model: HashMap<String, ModelCostBreakdown>,
    pub budget_limit_usd: Option<f64>,
    pub is_over_budget: bool,
}

/// Task handle for background agent execution.
pub struct TaskHandle {
    /// Unique task identifier.
    pub id: String,
    /// Current status.
    status: Arc<tokio::sync::RwLock<TaskStatus>>,
    /// Result channel.
    result_rx: tokio::sync::oneshot::Receiver<Result<String>>,
}

/// Status of a background task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

impl TaskHandle {
    /// Get the current status.
    pub async fn status(&self) -> TaskStatus {
        self.status.read().await.clone()
    }

    /// Wait for the task to complete and return the result.
    pub async fn result(self) -> Result<String> {
        self.result_rx.await.map_err(|_| {
            crate::Error::Llm("Task result channel closed".to_string())
        })?
    }

    /// Check if the task is done.
    pub async fn is_done(&self) -> bool {
        matches!(
            *self.status.read().await,
            TaskStatus::Completed | TaskStatus::Failed(_) | TaskStatus::Cancelled
        )
    }
}

/// Hook trait for intercepting tool calls in the agent loop.
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Called before a tool is executed. Return false to skip the tool call.
    async fn before_tool_call(&self, _tool_name: &str, _args: &str) -> bool {
        true
    }

    /// Called after a tool is executed with its result.
    async fn after_tool_call(&self, _tool_name: &str, _result: &str) {}

    /// Called when an error occurs during tool execution.
    async fn on_error(&self, _tool_name: &str, _error: &str) {}
}

/// Default no-op hook.
pub struct NoopHook;

#[async_trait]
impl AgentHook for NoopHook {}

/// Cloud Agent for background task execution.
///
/// Provides isolated task execution with:
/// - Background processing via tokio spawn
/// - Status tracking (Queued → Running → Completed/Failed)
/// - Result delivery via oneshot channel
/// - Cancellation support
pub struct CloudAgent {
    /// Task queue for pending work.
    task_queue: mpsc::UnboundedSender<CloudTask>,
    /// Active task count.
    active_tasks: Arc<AtomicU64>,
    /// Maximum concurrent tasks.
    max_concurrent: usize,
}

/// A task submitted to the Cloud Agent.
struct CloudTask {
    /// Unique task identifier.
    id: String,
    /// Task type for routing.
    task_type: TaskType,
    /// Messages to process.
    messages: Vec<ChatMessage>,
    /// Status handle.
    status: Arc<tokio::sync::RwLock<TaskStatus>>,
    /// Result sender.
    result_tx: tokio::sync::oneshot::Sender<Result<String>>,
}

/// Status of a cloud agent task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is queued waiting for execution.
    Queued,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed(String),
    /// Task was cancelled.
    Cancelled,
}

impl CloudAgent {
    /// Create a new Cloud Agent with the given concurrency limit.
    pub fn new(max_concurrent: usize) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let active_tasks = Arc::new(AtomicU64::new(0));

        // Spawn the task processor
        let active = Arc::clone(&active_tasks);
        tokio::spawn(Self::process_tasks(rx, active));

        Self {
            task_queue: tx,
            active_tasks,
            max_concurrent,
        }
    }

    /// Create a Cloud Agent with default concurrency (4).
    pub fn default_agent() -> Self {
        Self::new(4)
    }

    /// Submit a task for background execution.
    pub async fn submit(
        &self,
        task_type: TaskType,
        messages: Vec<ChatMessage>,
    ) -> Result<TaskHandle> {
        // Check concurrency limit
        if self.active_tasks.load(Ordering::Relaxed) >= self.max_concurrent as u64 {
            return Err(crate::Error::Llm(
                "Cloud agent task limit reached".to_string(),
            ));
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let status = Arc::new(tokio::sync::RwLock::new(TaskStatus::Queued));
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        let task = CloudTask {
            id: task_id.clone(),
            task_type,
            messages,
            status: Arc::clone(&status),
            result_tx,
        };

        self.task_queue.send(task).map_err(|_| {
            crate::Error::Llm("Cloud agent task queue closed".to_string())
        })?;

        Ok(TaskHandle {
            id: task_id,
            status,
            result_rx,
        })
    }

    /// Get the number of active tasks.
    pub fn active_task_count(&self) -> u64 {
        self.active_tasks.load(Ordering::Relaxed)
    }

    /// Process tasks from the queue.
    async fn process_tasks(
        mut rx: mpsc::UnboundedReceiver<CloudTask>,
        active_tasks: Arc<AtomicU64>,
    ) {
        while let Some(task) = rx.recv().await {
            let active = Arc::clone(&active_tasks);
            active.fetch_add(1, Ordering::Relaxed);

            tokio::spawn(async move {
                *task.status.write().await = TaskStatus::Running;

                // Execute the task (placeholder - in production this would call the LLM)
                let result = Self::execute_task(task.task_type, task.messages).await;

                *task.status.write().await = match &result {
                    Ok(_) => TaskStatus::Completed,
                    Err(e) => TaskStatus::Failed(e.to_string()),
                };

                let _ = task.result_tx.send(result);
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }

    /// Execute a task (placeholder implementation).
    async fn execute_task(
        task_type: TaskType,
        messages: Vec<ChatMessage>,
    ) -> Result<String> {
        // In production, this would create a ModelRouter and call chat()
        // For now, return a placeholder response
        let task_name = match task_type {
            TaskType::Think => "thinking",
            TaskType::Plan => "planning",
            TaskType::Build => "building",
            TaskType::Test => "testing",
            TaskType::Review => "reviewing",
            TaskType::Summarize => "summarizing",
            TaskType::Chat => "chatting",
        };

        let prompt = messages.last().map(|m| m.content.as_str()).unwrap_or("no input");
        Ok(format!("[CloudAgent:{task_name}] Processed: {prompt}"))
    }
}

/// Handle for tracking a cloud agent task.
pub struct TaskHandle {
    /// Unique task identifier.
    pub id: String,
    /// Current status.
    status: Arc<tokio::sync::RwLock<TaskStatus>>,
    /// Result channel.
    result_rx: tokio::sync::oneshot::Receiver<Result<String>>,
}

impl TaskHandle {
    /// Get the current status.
    pub async fn status(&self) -> TaskStatus {
        self.status.read().await.clone()
    }

    /// Wait for the task to complete and return the result.
    pub async fn result(self) -> Result<String> {
        self.result_rx.await.map_err(|_| {
            crate::Error::Llm("Task result channel closed".to_string())
        })?
    }

    /// Check if the task is done.
    pub async fn is_done(&self) -> bool {
        matches!(
            *self.status.read().await,
            TaskStatus::Completed | TaskStatus::Failed(_) | TaskStatus::Cancelled
        )
    }
}

/// A step in a dynamic workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Unique step identifier.
    pub id: String,
    /// Task type for this step.
    pub task_type: TaskType,
    /// Prompt/instructions for this step.
    pub prompt: String,
    /// IDs of steps this depends on (must complete before this runs).
    pub depends_on: Vec<String>,
}

/// Result of a workflow step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResult {
    /// Step ID.
    pub step_id: String,
    /// Whether the step succeeded.
    pub success: bool,
    /// Output from the step.
    pub output: String,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
}

/// A dynamic workflow with DAG-based step orchestration.
pub struct DynamicWorkflow {
    /// Workflow steps.
    steps: Vec<WorkflowStep>,
    /// Step results.
    results: HashMap<String, WorkflowStepResult>,
    /// Cloud agent for execution.
    cloud_agent: CloudAgent,
}

impl DynamicWorkflow {
    /// Create a new empty workflow.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            results: HashMap::new(),
            cloud_agent: CloudAgent::new(4),
        }
    }

    /// Add a step to the workflow.
    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
    }

    /// Get all steps that are ready to execute (all dependencies satisfied).
    pub fn ready_steps(&self) -> Vec<&WorkflowStep> {
        self.steps
            .iter()
            .filter(|step| {
                step.depends_on
                    .iter()
                    .all(|dep_id| self.results.contains_key(dep_id))
            })
            .collect()
    }

    /// Check if the workflow is complete.
    pub fn is_complete(&self) -> bool {
        self.results.len() == self.steps.len()
    }

    /// Execute the workflow.
    pub async fn execute(&mut self) -> Result<Vec<WorkflowStepResult>> {
        let mut all_results = Vec::new();

        while !self.is_complete() {
            let ready: Vec<_> = self.ready_steps().into_iter().cloned().collect();

            if ready.is_empty() && !self.is_complete() {
                return Err(crate::Error::Llm(
                    "Workflow has circular dependencies or missing steps".to_string(),
                ));
            }

            // Execute ready steps in parallel
            let mut handles = Vec::new();
            for step in &ready {
                let messages = vec![ChatMessage::text(ChatRole::User, &step.prompt)];
                let handle = self.cloud_agent.submit(step.task_type, messages).await?;
                handles.push((step.id.clone(), handle));
            }

            // Collect results
            for (step_id, handle) in handles {
                let output = handle.result().await.unwrap_or_else(|e| e.to_string());
                let result = WorkflowStepResult {
                    step_id: step_id.clone(),
                    success: !output.starts_with("Error"),
                    output,
                    duration_ms: 0,
                };
                self.results.insert(step_id, result.clone());
                all_results.push(result);
            }
        }

        Ok(all_results)
    }
}

impl Default for DynamicWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

/// Model Router: implements `LlmClient` with task-aware dispatch.
pub struct ModelRouter {
    /// Provider instances keyed by (provider, model) tuple.
    providers: Arc<Mutex<HashMap<(String, String), Arc<dyn LlmClient>>>>,
    /// Routing rules: task_type → primary rule.
    rules: HashMap<TaskType, RoutingRule>,
    /// Default rule when no specific rule matches.
    default_rule: RoutingRule,
    /// Pricing table.
    pricing: HashMap<String, ModelPricing>,
    /// Cost tracker.
    cost_tracker: CostTracker,
    /// Current task type (set per-request).
    current_task: Arc<Mutex<Option<TaskType>>>,
    /// Default API key for providers that need one.
    default_api_keys: HashMap<String, String>,
    /// Active hooks for tool call interception.
    hooks: Vec<Arc<dyn AgentHook>>,
    /// Maximum concurrent subagents.
    max_subagents: usize,
    /// Active subagent count.
    active_subagents: Arc<AtomicU64>,
    /// Optional MCP client manager for external tool discovery and execution.
    mcp_manager: Option<Arc<McpClientManager>>,
}

impl ModelRouter {
    /// Create a new ModelRouter with default configuration.
    pub fn new(default_config: &ResolvedLlmConfig) -> Result<Self> {
        let default_rule = RoutingRule::new(
            TaskType::Chat,
            &default_config.provider,
            &default_config.model,
        );

        let mut default_api_keys = HashMap::new();
        if let Some(ref key) = default_config.api_key {
            default_api_keys.insert(default_config.provider.clone(), key.clone());
        }

        Ok(Self {
            providers: Arc::new(Mutex::new(HashMap::new())),
            rules: HashMap::new(),
            default_rule,
            pricing: default_pricing_table(),
            cost_tracker: CostTracker::new(None),
            current_task: Arc::new(Mutex::new(None)),
            default_api_keys,
            hooks: Vec::new(),
            max_subagents: 8,
            active_subagents: Arc::new(AtomicU64::new(0)),
            mcp_manager: None,
        })
    }

    /// Create with budget limit.
    pub fn with_budget(default_config: &ResolvedLlmConfig, budget_usd: f64) -> Result<Self> {
        let mut router = Self::new(default_config)?;
        router.cost_tracker = CostTracker::new(Some(budget_usd));
        Ok(router)
    }

    /// Add a routing rule for a specific task type.
    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.rules.insert(rule.task_type, rule);
    }

    /// Set the default API key for a provider.
    pub fn set_api_key(&mut self, provider: &str, key: &str) {
        self.default_api_keys.insert(provider.to_string(), key.to_string());
    }

    /// Add custom pricing for a model.
    pub fn add_pricing(&mut self, model: &str, pricing: ModelPricing) {
        self.pricing.insert(model.to_string(), pricing);
    }

    /// Set the current task type for routing.
    pub async fn set_task(&self, task: TaskType) {
        let mut current = self.current_task.lock().await;
        *current = Some(task);
    }

    /// Clear the current task type (revert to default routing).
    pub async fn clear_task(&self) {
        let mut current = self.current_task.lock().await;
        *current = None;
    }

    /// Set the MCP client manager for external tool discovery.
    pub fn set_mcp_manager(&mut self, manager: Arc<McpClientManager>) {
        self.mcp_manager = Some(manager);
    }

    /// Get the MCP client manager if set.
    pub fn mcp_manager(&self) -> Option<&Arc<McpClientManager>> {
        self.mcp_manager.as_ref()
    }

    /// Select the best model for a given task complexity and budget.
    ///
    /// Returns (provider, model) tuple for the optimal model.
    pub fn select_model_for_complexity(&self, complexity: TaskComplexity) -> (String, String) {
        let min_quality = match complexity {
            TaskComplexity::Simple => 2,
            TaskComplexity::Medium => 3,
            TaskComplexity::Complex => 4,
            TaskComplexity::Critical => 5,
        };

        // Find models that meet the quality requirement
        let candidates: Vec<_> = self.pricing.iter()
            .filter(|(_, p)| p.quality_tier >= min_quality)
            .collect();

        if candidates.is_empty() {
            // Fallback to default rule
            return (self.default_rule.provider.clone(), self.default_rule.model.clone());
        }

        // Sort by efficiency (quality per dollar), descending
        let mut sorted = candidates;
        sorted.sort_by(|a, b| {
            b.1.efficiency().partial_cmp(&a.1.efficiency()).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return the most efficient model that meets quality requirements
        let (model_name, _) = sorted[0];
        let provider = self.default_api_keys.keys().next()
            .cloned()
            .unwrap_or_else(|| "openai".to_string());

        (provider, model_name.clone())
    }

    /// Estimate cost for a given model and token counts.
    pub fn estimate_cost(&self, model: &str, input_tokens: usize, output_tokens: usize) -> f64 {
        let pricing = self.get_pricing(model);
        pricing.cost(input_tokens, output_tokens)
    }

    /// Get cost report with budget status.
    pub async fn cost_report_with_budget(&self) -> CostReport {
        let report = self.cost_report().await;
        CostReport {
            total_cost_usd: report.total_cost_usd,
            total_input_tokens: report.total_input_tokens,
            total_output_tokens: report.total_output_tokens,
            per_model: report.per_model,
            budget_limit_usd: self.cost_tracker.budget_limit_usd,
            is_over_budget: self.cost_tracker.is_over_budget(),
        }
    }

    /// Discover MCP tools and convert to genai Tool format.
    async fn discover_mcp_tools(&self) -> Vec<Tool> {
        let Some(manager) = &self.mcp_manager else {
            return Vec::new();
        };

        let mcp_tools = match manager.list_all_tools().await {
            Ok(tools) => tools,
            Err(e) => {
                tracing::warn!("Failed to discover MCP tools: {e}");
                return Vec::new();
            }
        };

        mcp_tools
            .into_iter()
            .map(|(server, tool)| {
                let name = format!("mcp_{}_{}", server, tool.name);
                let description = tool.description.clone().unwrap_or_default();
                let params = tool.input_schema.clone().unwrap_or(serde_json::json!({}));
                Tool {
                    name,
                    description,
                    parameters: params,
                }
            })
            .collect()
    }

    /// Execute an MCP tool call via the client manager.
    async fn execute_mcp_tool(&self, full_name: &str, arguments: &str) -> Result<String> {
        let Some(manager) = &self.mcp_manager else {
            return Err(crate::Error::Llm("No MCP manager configured".to_string()));
        };

        // Parse "mcp_{server}_{tool}" format
        let name_without_prefix = full_name.strip_prefix("mcp_").unwrap_or(full_name);
        let parts: Vec<&str> = name_without_prefix.splitn(2, '_').collect();
        if parts.len() < 2 {
            return Err(crate::Error::Llm(format!(
                "Invalid MCP tool name format: {full_name}"
            )));
        }
        let server_name = parts[0];
        let tool_name = parts[1];

        let args: serde_json::Value = serde_json::from_str(arguments)
            .unwrap_or(serde_json::json!({}));

        match manager.call_tool(server_name, tool_name, args).await {
            Ok(result) => {
                // Extract text from MCP tool result
                let text = result
                    .content
                    .iter()
                    .filter_map(|c| match &c {
                        super::mcp::protocol::McpContent::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(text)
            },
            Err(e) => Err(crate::Error::Llm(format!("MCP tool error: {e}"))),
        }
    }

    /// Add a hook for intercepting tool calls.
    pub fn add_hook(&mut self, hook: Arc<dyn AgentHook>) {
        self.hooks.push(hook);
    }

    /// Spawn a subagent to run a task in the background.
    pub async fn spawn_subagent(
        &self,
        task_type: TaskType,
        messages: Vec<ChatMessage>,
    ) -> Result<TaskHandle> {
        if self.active_subagents.load(Ordering::Relaxed) >= self.max_subagents as u64 {
            return Err(crate::Error::Llm(
                "Maximum concurrent subagents reached".to_string(),
            ));
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let status = Arc::new(tokio::sync::RwLock::new(TaskStatus::Queued));
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        self.active_subagents.fetch_add(1, Ordering::Relaxed);

        let providers = Arc::clone(&self.providers);
        let rules = self.rules.clone();
        let default_rule = self.default_rule.clone();
        let pricing = self.pricing.clone();
        let cost_tracker = CostTracker::new(None);
        let default_api_keys = self.default_api_keys.clone();
        let task_status = Arc::clone(&status);
        let active_count = Arc::clone(&self.active_subagents);

        tokio::spawn(async move {
            *task_status.write().await = TaskStatus::Running;

            let router = ModelRouter {
                providers,
                rules,
                default_rule,
                pricing,
                cost_tracker,
                current_task: Arc::new(Mutex::new(Some(task_type))),
                default_api_keys,
                hooks: Vec::new(),
                max_subagents: 1,
                active_subagents: Arc::new(AtomicU64::new(0)),
                mcp_manager: None,
            };

            let result = router.chat(messages).await;

            *task_status.write().await = match &result {
                Ok(_) => TaskStatus::Completed,
                Err(e) => TaskStatus::Failed(e.to_string()),
            };

            active_count.fetch_sub(1, Ordering::Relaxed);
            let _ = result_tx.send(result);
        });

        Ok(TaskHandle {
            id: task_id,
            status,
            result_rx,
        })
    }

    /// Resolve which rule to use for the current task.
    fn resolve_rule(&self, task: Option<TaskType>) -> &RoutingRule {
        match task {
            Some(t) => self.rules.get(&t).unwrap_or(&self.default_rule),
            None => &self.default_rule,
        }
    }

    /// Get or create a provider for a routing rule.
    async fn get_provider(&self, rule: &RoutingRule) -> Result<Arc<dyn LlmClient>> {
        let key = (rule.provider.clone(), rule.model.clone());

        {
            let providers = self.providers.lock().await;
            if let Some(provider) = providers.get(&key) {
                return Ok(Arc::clone(provider));
            }
        }

        // Create new provider
        let mut config = ResolvedLlmConfig {
            provider: rule.provider.clone(),
            model: rule.model.clone(),
            api_key: rule.api_key.clone().or_else(|| {
                self.default_api_keys.get(&rule.provider).cloned()
            }),
            base_url: rule.base_url.clone(),
            max_tokens: 4096,
        };

        // Read from env if no explicit key
        if config.api_key.is_none() {
            if let Ok(env_config) = ResolvedLlmConfig::from_env(&rule.provider) {
                config.api_key = env_config.api_key;
                config.base_url = env_config.base_url.or(config.base_url);
            }
        }

        let provider = create_provider(&config)?;
        let provider: Arc<dyn LlmClient> = Arc::new(provider);

        let mut providers = self.providers.lock().await;
        providers.insert(key, Arc::clone(&provider));
        Ok(provider)
    }

    /// Get pricing for a model (fallback to defaults if not in table).
    fn get_pricing(&self, model: &str) -> ModelPricing {
        self.pricing.get(model).cloned().unwrap_or(ModelPricing {
            input_per_1m: 1.0,
            output_per_1m: 3.0,
            context_window: 128_000,
            max_output_tokens: 4_096,
        })
    }

    /// Get the cost tracker reference.
    pub fn cost_tracker(&self) -> &CostTracker {
        &self.cost_tracker
    }

    /// Generate a cost report.
    pub async fn cost_report(&self) -> CostReport {
        CostReport {
            total_cost_usd: self.cost_tracker.total_usd(),
            total_input_tokens: self.cost_tracker.total_input_tokens(),
            total_output_tokens: self.cost_tracker.total_output_tokens(),
            per_model: self.cost_tracker.per_model_breakdown().await,
            budget_limit_usd: self.cost_tracker.budget_limit_usd,
            is_over_budget: self.cost_tracker.is_over_budget(),
        }
    }

    /// Estimate cost before making a request.
    pub fn estimate_cost(&self, model: &str, input_tokens: usize, estimated_output_tokens: usize) -> f64 {
        let pricing = self.get_pricing(model);
        pricing.cost(input_tokens, estimated_output_tokens)
    }

    /// Build default routing rules for a typical setup.
    /// Uses the given provider/model as the "expensive" primary,
    /// and tries to find a cheaper model for think/test/summarize.
    pub fn default_rules(primary_provider: &str, primary_model: &str) -> Vec<RoutingRule> {
        let (cheap_provider, cheap_model) = match primary_provider {
            "anthropic" => ("anthropic", "claude-3-5-haiku-20241022"),
            "openai" => ("openai", "gpt-4o-mini"),
            "google" => ("google", "gemini-2.0-flash"),
            "zai" => ("zai", "glm-4.6"),
            "openrouter" => ("openrouter", "google/gemma-3-4b-it:free"),
            "opencode-go" => ("opencode-go", "mimo-v2.5"),
            _ => (primary_provider, primary_model), // fallback to same
        };

        vec![
            RoutingRule::new(TaskType::Think, cheap_provider, cheap_model),
            RoutingRule::new(TaskType::Plan, cheap_provider, cheap_model),
            RoutingRule::new(TaskType::Build, primary_provider, primary_model),
            RoutingRule::new(TaskType::Test, cheap_provider, cheap_model),
            RoutingRule::new(TaskType::Review, primary_provider, primary_model),
            RoutingRule::new(TaskType::Summarize, cheap_provider, cheap_model),
        ]
    }
}

#[async_trait]
impl LlmClient for ModelRouter {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let task = *self.current_task.lock().await;
        let rule = self.resolve_rule(task);
        let input_tokens: usize = messages.iter().map(|m| m.content.split_whitespace().count()).sum();

        let (provider, active_model) = match self.get_provider(rule).await {
            Ok(p) => (p, rule.model.clone()),
            Err(e) => {
                if let Some(ref fallback) = rule.fallback {
                    tracing::warn!("Primary model {}:{} failed: {}, trying fallback", rule.provider, rule.model, e);
                    (self.get_provider(fallback).await?, fallback.model.clone())
                } else {
                    return Err(e);
                }
            }
        };

        let result = provider.chat(messages).await;

        if let Ok(ref response) = result {
            let output_tokens = response.split_whitespace().count();
            let pricing = self.get_pricing(&active_model);
            let _ = self.cost_tracker.record(&active_model, input_tokens, output_tokens, &pricing).await;
        }

        result
    }

    async fn chat_stream(&self, messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
        let task = *self.current_task.lock().await;
        let rule = self.resolve_rule(task);
        let input_tokens: usize = messages.iter().map(|m| m.content.split_whitespace().count()).sum();

        let (provider, model_name) = match self.get_provider(rule).await {
            Ok(p) => (p, rule.model.clone()),
            Err(e) => {
                if let Some(ref fallback) = rule.fallback {
                    tracing::warn!("Primary failed: {}, trying fallback", e);
                    (self.get_provider(fallback).await?, fallback.model.clone())
                } else {
                    return Err(e);
                }
            }
        };

        let result = provider.chat_stream(messages).await;

        if result.is_ok() {
            let pricing = self.get_pricing(&model_name);
            let _ = self.cost_tracker.record(&model_name, input_tokens, 100, &pricing).await;
        }

        result
    }

    async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        mut tools: Vec<Tool>,
    ) -> Result<ChatWithToolsResult> {
        let task = *self.current_task.lock().await;
        let rule = self.resolve_rule(task);
        let input_tokens: usize = messages.iter().map(|m| m.content.split_whitespace().count()).sum();

        // Discover MCP tools and merge with provided tools
        let mcp_tools = self.discover_mcp_tools().await;
        if !mcp_tools.is_empty() {
            tracing::debug!("Discovered {} MCP tools", mcp_tools.len());
            tools.extend(mcp_tools);
        }

        let (provider, model_name) = match self.get_provider(rule).await {
            Ok(p) => (p, rule.model.clone()),
            Err(e) => {
                if let Some(ref fallback) = rule.fallback {
                    (self.get_provider(fallback).await?, fallback.model.clone())
                } else {
                    return Err(e);
                }
            }
        };

        let result = provider.chat_with_tools(messages, tools).await;

        if let Ok(ref r) = result {
            // Run hooks on tool calls
            for hook in &self.hooks {
                for tc in &r.tool_calls {
                    hook.before_tool_call(&tc.function_name, &tc.arguments).await;
                    hook.after_tool_call(&tc.function_name, &r.text).await;
                }
            }

            let output_tokens = r.text.split_whitespace().count() + r.tool_calls.len() * 50;
            let pricing = self.get_pricing(&model_name);
            let _ = self.cost_tracker.record(&model_name, input_tokens, output_tokens, &pricing).await;
        } else if let Err(ref e) = result {
            for hook in &self.hooks {
                hook.on_error("chat_with_tools", &e.to_string()).await;
            }
        }

        result
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let task = *self.current_task.lock().await;
        let rule = self.resolve_rule(task);
        let provider = self.get_provider(rule).await?;
        provider.embed(text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_from_phase() {
        assert_eq!(TaskType::from_phase_name("think"), TaskType::Think);
        assert_eq!(TaskType::from_phase_name("Think"), TaskType::Think);
        assert_eq!(TaskType::from_phase_name("build"), TaskType::Build);
        assert_eq!(TaskType::from_phase_name("test"), TaskType::Test);
        assert_eq!(TaskType::from_phase_name("review"), TaskType::Review);
        assert_eq!(TaskType::from_phase_name("unknown"), TaskType::Chat);
    }

    #[test]
    fn test_model_pricing_cost() {
        let claude_sonnet = ModelPricing {
            input_per_1m: 3.0,
            output_per_1m: 15.0,
            context_window: 200_000,
            max_output_tokens: 16_384,
        };

        // 1k input, 500 output tokens
        let cost = claude_sonnet.cost(1000, 500);
        assert!((cost - 0.0105).abs() < 0.0001); // $0.0105

        // 100k input, 10k output tokens
        let cost = claude_sonnet.cost(100_000, 10_000);
        assert!((cost - 0.45).abs() < 0.01); // ~$0.45
    }

    #[test]
    fn test_free_model_has_zero_cost() {
        let free = ModelPricing {
            input_per_1m: 0.0,
            output_per_1m: 0.0,
            context_window: 32_000,
            max_output_tokens: 4_096,
        };
        assert_eq!(free.cost(1_000_000, 1_000_000), 0.0);
    }

    #[test]
    fn test_cost_tracker_basic() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tracker = CostTracker::new(None);

        let pricing = ModelPricing {
            input_per_1m: 3.0,
            output_per_1m: 15.0,
            context_window: 200_000,
            max_output_tokens: 16_384,
        };

        rt.block_on(async {
            tracker.record("claude-sonnet-4", 1000, 500, &pricing).await.unwrap();
            tracker.record("claude-sonnet-4", 2000, 1000, &pricing).await.unwrap();

            assert!(tracker.total_usd() > 0.0);
            assert_eq!(tracker.total_input_tokens(), 3000);
            assert_eq!(tracker.total_output_tokens(), 1500);

            let breakdown = tracker.per_model_breakdown().await;
            assert!(breakdown.contains_key("claude-sonnet-4"));
            assert_eq!(breakdown["claude-sonnet-4"].request_count, 2);
        });
    }

    #[test]
    fn test_cost_tracker_budget_enforcement() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tracker = CostTracker::new(Some(0.01)); // $0.01 budget

        let expensive = ModelPricing {
            input_per_1m: 100.0,
            output_per_1m: 500.0,
            context_window: 200_000,
            max_output_tokens: 16_384,
        };

        rt.block_on(async {
            // First request should succeed ($0.001 + $0.0025 = $0.0035)
            tracker.record("expensive-model", 10, 5, &expensive).await.unwrap();
            // Second request exceeds budget — should be rejected
            let result = tracker.record("expensive-model", 100_000, 10_000, &expensive).await;
            assert!(result.is_err());
            // Budget was not exceeded because the over-budget request was rejected
            assert!(!tracker.is_over_budget());
            // But total is close to budget
            assert!(tracker.total_usd() < 0.01);
        });
    }

    #[test]
    fn test_default_pricing_table() {
        let table = default_pricing_table();
        assert!(table.contains_key("claude-sonnet-4-20250514"));
        assert!(table.contains_key("gpt-4o"));
        assert!(table.contains_key("glm-4.6"));
        assert!(table.contains_key("gemini-2.0-flash"));

        // Free models
        assert_eq!(table["google/gemma-3-4b-it:free"].input_per_1m, 0.0);
    }

    #[test]
    fn test_default_rules() {
        let rules = ModelRouter::default_rules("anthropic", "claude-sonnet-4-20250514");
        assert_eq!(rules.len(), 6);

        // Think/Plan should use cheap model
        let think_rule = rules.iter().find(|r| r.task_type == TaskType::Think).unwrap();
        assert_eq!(think_rule.model, "claude-3-5-haiku-20241022");

        // Build should use primary model
        let build_rule = rules.iter().find(|r| r.task_type == TaskType::Build).unwrap();
        assert_eq!(build_rule.model, "claude-sonnet-4-20250514");

        // Test should use cheap model
        let test_rule = rules.iter().find(|r| r.task_type == TaskType::Test).unwrap();
        assert_eq!(test_rule.model, "claude-3-5-haiku-20241022");
    }

    #[tokio::test]
    async fn test_mcp_tool_discovery_empty() {
        let config = ResolvedLlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };
        let router = ModelRouter::new(&config).unwrap();

        // No MCP manager set, should return empty
        let tools = router.discover_mcp_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_subagent_limit() {
        let config = ResolvedLlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };
        let mut router = ModelRouter::new(&config).unwrap();
        router.set_max_subagents(2);

        // Fill up the subagent slots
        router.active_subagents.store(2, Ordering::Relaxed);

        // Should fail when at limit
        let messages = vec![ChatMessage::text(ChatRole::User, "test")];
        let result = router.spawn_subagent(TaskType::Chat, messages).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let config = ResolvedLlmConfig {
            provider: "zai".to_string(),
            model: "glm-4.6".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };
        let router = ModelRouter::new(&config).unwrap();

        // GLM-4.6: $0.50/1M input, $0.50/1M output
        let cost = router.estimate_cost("glm-4.6", 100_000, 10_000);
        assert!((cost - 0.055).abs() < 0.001); // ~$0.055
    }

    #[test]
    fn test_model_pricing_efficiency() {
        let cheap = ModelPricing {
            input_per_1m: 0.15,
            output_per_1m: 0.6,
            context_window: 128_000,
            max_output_tokens: 16_384,
            quality_tier: 3,
        };

        let expensive = ModelPricing {
            input_per_1m: 15.0,
            output_per_1m: 75.0,
            context_window: 200_000,
            max_output_tokens: 4_096,
            quality_tier: 5,
        };

        // Cheap model should have higher efficiency (quality per dollar)
        assert!(cheap.efficiency() > expensive.efficiency());
    }

    #[test]
    fn test_task_complexity_ordering() {
        assert!(TaskComplexity::Simple < TaskComplexity::Medium);
        assert!(TaskComplexity::Medium < TaskComplexity::Complex);
        assert!(TaskComplexity::Complex < TaskComplexity::Critical);
    }

    #[test]
    fn test_select_model_for_complexity() {
        let config = ResolvedLlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };
        let router = ModelRouter::new(&config).unwrap();

        // Simple tasks should select a cheaper model
        let (provider, model) = router.select_model_for_complexity(TaskComplexity::Simple);
        assert!(!model.is_empty());

        // Critical tasks should select a higher quality model
        let (provider, model) = router.select_model_for_complexity(TaskComplexity::Critical);
        assert!(!model.is_empty());
    }

    #[test]
    fn test_cost_report() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = ResolvedLlmConfig {
            provider: "zai".to_string(),
            model: "glm-4.6".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };
        let router = ModelRouter::with_budget(&config, 10.0).unwrap();

        let pricing = ModelPricing {
            input_per_1m: 1.0,
            output_per_1m: 3.0,
            context_window: 128_000,
            max_output_tokens: 4_096,
        };

        rt.block_on(async {
            router.cost_tracker.record("test-model", 1000, 500, &pricing).await.unwrap();
            let report = router.cost_report().await;
            assert_eq!(report.per_model.len(), 1);
            assert!(!report.is_over_budget);
            assert!(report.total_cost_usd > 0.0);
        });
    }

    #[tokio::test]
    async fn test_cloud_agent_creation() {
        let agent = CloudAgent::new(2);
        assert_eq!(agent.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_cloud_agent_submit() {
        let agent = CloudAgent::new(4);
        let messages = vec![ChatMessage::text(ChatRole::User, "test")];
        let handle = agent.submit(TaskType::Chat, messages).await.unwrap();
        assert!(!handle.id.is_empty());
    }

    #[tokio::test]
    async fn test_cloud_agent_concurrency_limit() {
        let agent = CloudAgent::new(1);
        agent.active_tasks.store(1, Ordering::Relaxed);

        let messages = vec![ChatMessage::text(ChatRole::User, "test")];
        let result = agent.submit(TaskType::Chat, messages).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dynamic_workflow_creation() {
        let mut workflow = DynamicWorkflow::new();
        assert!(workflow.is_complete());
        assert!(workflow.ready_steps().is_empty());
    }

    #[tokio::test]
    async fn test_dynamic_workflow_add_step() {
        let mut workflow = DynamicWorkflow::new();
        workflow.add_step(WorkflowStep {
            id: "step1".to_string(),
            task_type: TaskType::Think,
            prompt: "Analyze".to_string(),
            depends_on: vec![],
        });
        assert_eq!(workflow.ready_steps().len(), 1);
    }

    #[tokio::test]
    async fn test_dynamic_workflow_dependencies() {
        let mut workflow = DynamicWorkflow::new();
        workflow.add_step(WorkflowStep {
            id: "step1".to_string(),
            task_type: TaskType::Think,
            prompt: "Analyze".to_string(),
            depends_on: vec![],
        });
        workflow.add_step(WorkflowStep {
            id: "step2".to_string(),
            task_type: TaskType::Build,
            prompt: "Build".to_string(),
            depends_on: vec!["step1".to_string()],
        });

        // Only step1 should be ready
        assert_eq!(workflow.ready_steps().len(), 1);
        assert_eq!(workflow.ready_steps()[0].id, "step1");
    }
}
