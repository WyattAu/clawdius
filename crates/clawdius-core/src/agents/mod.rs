//! Agent Teams / Swarms Coordination
//!
//! This module implements multi-agent coordination for complex tasks.
//! Inspired by `NanoClaw` and Claude Code's agent teams feature.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Team Coordinator                      │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐│
//! │  │ Agent 1  │  │ Agent 2  │  │ Agent 3  │  │ Agent N  ││
//! │  │(Coder)   │  │(Reviewer)│  │(Tester)  │  │(Custom)  ││
//! │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘│
//! │       │             │             │             │      │
//! │       └─────────────┴─────────────┴─────────────┘      │
//! │                          │                              │
//! │                   Shared Context                        │
//! │                   (Message Bus)                         │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use clawdius_core::agents::{AgentTeam, AgentRole, TeamConfig};
//! use std::sync::Arc;
//!
//! let mut team = AgentTeam::new("code-review-team");
//! team.add_agent(AgentRole::Coder).await;
//! team.add_agent(AgentRole::Reviewer).await;
//! team.add_agent(AgentRole::Tester).await;
//!
//! let result = team.execute("Implement user authentication").await?;
//! ```

use crate::llm::{ChatMessage, ChatRole, LlmClient};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// Agent team errors
#[derive(Debug, Error)]
pub enum AgentError {
    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    /// Task failed
    #[error("Task failed: {0}")]
    TaskFailed(String),
    /// Communication error
    #[error("Communication error: {0}")]
    CommunicationError(String),
    /// Timeout
    #[error("Timeout: {0}")]
    Timeout(String),
    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),
    /// LLM error
    #[error("LLM error: {0}")]
    LlmError(String),
}

impl From<crate::Error> for AgentError {
    fn from(e: crate::Error) -> Self {
        AgentError::LlmError(e.to_string())
    }
}

/// Result type for agent operations
pub type Result<T> = std::result::Result<T, AgentError>;

/// Agent role types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Primary coder - implements features
    Coder,
    /// Code reviewer - reviews changes
    Reviewer,
    /// Tester - generates and runs tests
    Tester,
    /// Architect - designs system structure
    Architect,
    /// Researcher - gathers information
    Researcher,
    /// Debugger - diagnoses and fixes issues
    Debugger,
    /// Coordinator - manages team workflow
    Coordinator,
    /// Custom role with specific instructions
    Custom { name: String, instructions: String },
}

impl AgentRole {
    /// Get the system prompt for this role
    #[must_use]
    pub fn system_prompt(&self) -> String {
        match self {
            AgentRole::Coder => {
                "You are an expert coder agent. Your role is to implement features and write clean, efficient code. \
                 Focus on correctness, readability, and following best practices. \
                 When you complete a task, summarize what you did and any important decisions.".to_string()
            }
            AgentRole::Reviewer => {
                "You are an expert code reviewer. Your role is to review code changes for quality, \
                 security, performance, and maintainability. Provide constructive feedback and \
                 suggest improvements. Focus on catching bugs and ensuring code follows best practices.".to_string()
            }
            AgentRole::Tester => {
                "You are an expert testing agent. Your role is to generate comprehensive tests \
                 for code changes. Include unit tests, integration tests, and edge cases. \
                 Focus on high coverage and meaningful assertions.".to_string()
            }
            AgentRole::Architect => {
                "You are an expert software architect. Your role is to design system structure, \
                 define interfaces, and ensure architectural integrity. \
                 Focus on modularity, scalability, and maintainability.".to_string()
            }
            AgentRole::Researcher => {
                "You are an expert researcher agent. Your role is to gather information, \
                 analyze documentation, and provide context for the team. \
                 Focus on finding relevant information and summarizing key findings.".to_string()
            }
            AgentRole::Debugger => {
                "You are an expert debugger agent. Your role is to diagnose issues, \
                 find root causes, and suggest fixes. \
                 Focus on systematic problem-solving and clear explanations.".to_string()
            }
            AgentRole::Coordinator => {
                "You are a team coordinator. Your role is to manage the workflow, \
                 assign tasks to appropriate agents, and ensure the team works efficiently. \
                 Focus on orchestration and keeping the team on track.".to_string()
            }
            AgentRole::Custom { name, instructions } => {
                format!("You are a {name} agent. {instructions}")
            }
        }
    }

    /// Get the name of this role
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            AgentRole::Coder => "coder",
            AgentRole::Reviewer => "reviewer",
            AgentRole::Tester => "tester",
            AgentRole::Architect => "architect",
            AgentRole::Researcher => "researcher",
            AgentRole::Debugger => "debugger",
            AgentRole::Coordinator => "coordinator",
            AgentRole::Custom { name, .. } => name,
        }
    }

    #[allow(dead_code)]
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "coder" => Some(AgentRole::Coder),
            "reviewer" => Some(AgentRole::Reviewer),
            "tester" => Some(AgentRole::Tester),
            "architect" => Some(AgentRole::Architect),
            "researcher" => Some(AgentRole::Researcher),
            "debugger" => Some(AgentRole::Debugger),
            "coordinator" => Some(AgentRole::Coordinator),
            _ => None,
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Unique agent ID
    pub id: String,
    /// Agent role
    pub role: AgentRole,
    /// Maximum turns before stopping
    pub max_turns: usize,
    /// Enable autonomous mode
    pub autonomous: bool,
    /// Custom model override
    pub model: Option<String>,
    /// Temperature for responses
    pub temperature: f32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: AgentRole::Coder,
            max_turns: 10,
            autonomous: false,
            model: None,
            temperature: 0.7,
        }
    }
}

/// Agent state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Agent ID
    pub id: String,
    /// Agent role
    pub role: AgentRole,
    /// Current status
    pub status: AgentStatus,
    /// Current task
    pub current_task: Option<String>,
    /// Turn count
    pub turn_count: usize,
    /// Last activity
    pub last_activity: DateTime<Utc>,
    /// Conversation history
    pub history: Vec<AgentMessage>,
}

/// Agent status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is idle
    Idle,
    /// Agent is working
    Working,
    /// Agent is waiting for input
    Waiting,
    /// Agent has completed its task
    Completed,
    /// Agent encountered an error
    Failed,
}

/// Agent message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Message ID
    pub id: String,
    /// Sender agent ID
    pub sender: String,
    /// Recipient agent ID (or "all" for broadcast)
    pub recipient: String,
    /// Message content
    pub content: String,
    /// Message type
    pub message_type: MessageType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Message type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// Task assignment
    Task,
    /// Status update
    Status,
    /// Question/request
    Query,
    /// Response/answer
    Response,
    /// Code change
    CodeChange,
    /// Review feedback
    Feedback,
    /// Completion notification
    Completion,
    /// Error report
    Error,
}

/// Team configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Team name
    pub name: String,
    /// Maximum concurrent agents
    pub max_agents: usize,
    /// Communication timeout (seconds)
    pub communication_timeout: u64,
    /// Enable shared context
    pub shared_context: bool,
    /// Enable agent voting for decisions
    pub enable_voting: bool,
    /// Maximum iterations for team tasks
    pub max_iterations: usize,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            name: "default-team".into(),
            max_agents: 5,
            communication_timeout: 300,
            shared_context: true,
            enable_voting: false,
            max_iterations: 50,
        }
    }
}

/// Team result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamResult {
    /// Task that was executed
    pub task: String,
    /// Whether the task succeeded
    pub success: bool,
    /// Final output
    pub output: String,
    /// Agent contributions
    pub contributions: HashMap<String, AgentContribution>,
    /// Total turns taken
    pub total_turns: usize,
    /// Total time elapsed (ms)
    pub elapsed_ms: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Agent contribution summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContribution {
    /// Agent role
    pub role: AgentRole,
    /// Number of messages
    pub message_count: usize,
    /// Key contributions
    pub highlights: Vec<String>,
}

/// Shared context for team communication
#[derive(Debug, Default, Clone)]
pub struct SharedContext {
    /// Project files being worked on
    pub files: HashMap<String, String>,
    /// Key decisions made
    pub decisions: Vec<String>,
    /// Open questions
    pub questions: Vec<String>,
    /// Current task state
    pub task_state: HashMap<String, serde_json::Value>,
}

/// A subtask produced by the coordinator for delegation to a worker agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    /// Unique subtask ID
    pub id: String,
    /// Human-readable description of what this subtask should accomplish
    pub description: String,
    /// The agent role best suited to handle this subtask
    pub assigned_role: AgentRole,
    /// Any context from previous subtask results that this subtask depends on
    pub context: String,
}

/// Complexity level of a decomposed subtask.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Trivial,
    Simple,
    Moderate,
    Complex,
}

/// A decomposed subtask from the planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub assigned_role: AgentRole,
    pub depends_on: Vec<String>,
    pub estimated_complexity: TaskComplexity,
    pub acceptance_criteria: Vec<String>,
}

/// Result of LLM-backed task decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDecomposition {
    pub original_task: String,
    pub subtasks: Vec<SubTask>,
    pub total_estimated_steps: usize,
    pub requires_human_review: bool,
}

/// LLM-backed task decomposer that breaks high-level tasks into subtasks
/// with dependencies, complexity estimates, and acceptance criteria.
pub struct TaskDecomposer {
    llm_client: Arc<dyn LlmClient>,
}

impl TaskDecomposer {
    pub fn new(llm_client: Arc<dyn LlmClient>) -> Self {
        Self { llm_client }
    }

    /// Decompose a high-level task into subtasks using the LLM.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM call fails or the response cannot be parsed.
    pub async fn decompose(&self, task: &str, context: Option<&str>) -> Result<TaskDecomposition> {
        let context_section = context
            .map(|c| format!("Additional context:\n{c}"))
            .unwrap_or_default();

        let prompt = format!(
            r#"You are a task decomposition engine. Break this task into concrete subtasks.

Task: {task}

{context_section}

Respond with a JSON object:
{{
  "subtasks": [
    {{
      "id": "1",
      "description": "Concrete description of what to do",
      "assigned_role": "executor|researcher|reviewer|security|coder|tester|architect|debugger",
      "depends_on": [],
      "estimated_complexity": "trivial|simple|moderate|complex",
      "acceptance_criteria": ["criterion 1", "criterion 2"]
    }}
  ],
  "requires_human_review": false
}}

Rules:
- Each subtask must be independently verifiable
- Dependencies must form a DAG (no cycles)
- Security-sensitive tasks must be assigned to the security or reviewer role
- Code changes must be reviewed by the reviewer role
- Respond with ONLY valid JSON (no markdown fences)"#
        );

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: AgentRole::Coordinator.system_prompt(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: prompt,
            },
        ];

        let response = self
            .llm_client
            .chat(messages)
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        parse_task_decomposition(task, &response)
    }
}

fn parse_task_decomposition(task: &str, response: &str) -> Result<TaskDecomposition> {
    let cleaned = response.trim();
    let json_str = if cleaned.starts_with("```") {
        let without_fences = cleaned
            .strip_prefix("```json")
            .or_else(|| cleaned.strip_prefix("```"))
            .unwrap_or(cleaned);
        without_fences
            .strip_suffix("```")
            .unwrap_or(without_fences)
            .trim()
    } else {
        let start = cleaned.find('{').ok_or_else(|| {
            AgentError::TaskFailed("No JSON object found in decomposer response".into())
        })?;
        let end = cleaned.rfind('}').ok_or_else(|| {
            AgentError::TaskFailed("No closing brace in decomposer response".into())
        })?;
        &cleaned[start..=end]
    };

    #[derive(Deserialize)]
    struct SubTaskRaw {
        id: String,
        description: String,
        assigned_role: String,
        #[serde(default)]
        depends_on: Vec<String>,
        #[serde(default)]
        estimated_complexity: Option<String>,
        #[serde(default)]
        acceptance_criteria: Vec<String>,
    }

    #[derive(Deserialize)]
    struct DecompositionRaw {
        subtasks: Vec<SubTaskRaw>,
        #[serde(default)]
        requires_human_review: bool,
    }

    let raw: DecompositionRaw = serde_json::from_str(json_str)
        .map_err(|e| AgentError::TaskFailed(format!("Failed to parse task decomposition: {e}")))?;

    if raw.subtasks.is_empty() {
        return Err(AgentError::TaskFailed(
            "Decomposer produced zero subtasks".into(),
        ));
    }

    let subtasks: Vec<SubTask> = raw
        .subtasks
        .into_iter()
        .map(|s| {
            let role = parse_decompose_role(&s.assigned_role);
            let complexity = match s.estimated_complexity.as_deref() {
                Some("trivial") => TaskComplexity::Trivial,
                Some("simple") => TaskComplexity::Simple,
                Some("moderate") => TaskComplexity::Moderate,
                Some("complex") => TaskComplexity::Complex,
                _ => TaskComplexity::Moderate,
            };
            SubTask {
                id: s.id,
                description: s.description,
                assigned_role: role,
                depends_on: s.depends_on,
                estimated_complexity: complexity,
                acceptance_criteria: s.acceptance_criteria,
            }
        })
        .collect();

    validate_subtask_dag(&subtasks)?;

    let total_estimated_steps = subtasks.len();

    Ok(TaskDecomposition {
        original_task: task.to_string(),
        subtasks,
        total_estimated_steps,
        requires_human_review: raw.requires_human_review,
    })
}

fn parse_decompose_role(name: &str) -> AgentRole {
    match name.trim().to_lowercase().as_str() {
        "coder" | "executor" => AgentRole::Coder,
        "reviewer" => AgentRole::Reviewer,
        "tester" => AgentRole::Tester,
        "architect" => AgentRole::Architect,
        "researcher" => AgentRole::Researcher,
        "debugger" => AgentRole::Debugger,
        "security" => AgentRole::Reviewer,
        _ => AgentRole::Coder,
    }
}

fn validate_subtask_dag(subtasks: &[SubTask]) -> Result<()> {
    let id_set: std::collections::HashSet<&str> = subtasks.iter().map(|s| s.id.as_str()).collect();

    for subtask in subtasks {
        for dep in &subtask.depends_on {
            if !id_set.contains(dep.as_str()) {
                return Err(AgentError::TaskFailed(format!(
                    "Subtask '{}' depends on unknown subtask '{}'",
                    subtask.id, dep
                )));
            }
        }
    }

    let mut visited = std::collections::HashSet::new();
    let mut stack = std::collections::HashSet::new();

    for subtask in subtasks {
        if dfs_cycle_check(subtask.id.as_str(), subtasks, &mut visited, &mut stack)? {
            return Err(AgentError::TaskFailed(
                "Subtask dependencies contain a cycle".into(),
            ));
        }
    }

    Ok(())
}

fn dfs_cycle_check(
    id: &str,
    subtasks: &[SubTask],
    visited: &mut std::collections::HashSet<String>,
    stack: &mut std::collections::HashSet<String>,
) -> Result<bool> {
    if stack.contains(id) {
        return Ok(true);
    }
    if visited.contains(id) {
        return Ok(false);
    }

    visited.insert(id.to_string());
    stack.insert(id.to_string());

    if let Some(subtask) = subtasks.iter().find(|s| s.id == id) {
        for dep in &subtask.depends_on {
            if dfs_cycle_check(dep, subtasks, visited, stack)? {
                return Ok(true);
            }
        }
    }

    stack.remove(id);
    Ok(false)
}

fn topological_sort(subtasks: &[SubTask]) -> Vec<usize> {
    let id_to_idx: std::collections::HashMap<&str, usize> = subtasks
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    let mut in_degree: std::collections::HashMap<&str, usize> =
        subtasks.iter().map(|s| (s.id.as_str(), 0usize)).collect();

    for subtask in subtasks {
        for dep in &subtask.depends_on {
            if let Some(deg) = in_degree.get_mut(subtask.id.as_str()) {
                *deg += 1;
            }
        }
    }

    let mut queue: std::collections::VecDeque<usize> = subtasks
        .iter()
        .enumerate()
        .filter(|(_, s)| *in_degree.get(s.id.as_str()).unwrap_or(&0) == 0)
        .map(|(i, _)| i)
        .collect();

    let mut order = Vec::new();

    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for (other_i, other) in subtasks.iter().enumerate() {
            if other.depends_on.contains(&subtasks[idx].id) {
                if let Some(deg) = in_degree.get_mut(other.id.as_str()) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(other_i);
                    }
                }
            }
        }
    }

    order
}

/// Agent Team
pub struct AgentTeam {
    /// Team configuration
    config: TeamConfig,
    /// Agents in the team
    agents: Arc<RwLock<HashMap<String, AgentState>>>,
    /// Message bus for communication
    message_bus: broadcast::Sender<AgentMessage>,
    /// Shared context
    shared_context: Arc<RwLock<SharedContext>>,
    /// Task history
    task_history: Arc<RwLock<Vec<TeamResult>>>,
    /// Optional LLM client for real agent processing
    llm_client: Arc<RwLock<Option<Arc<dyn LlmClient>>>>,
}

impl AgentTeam {
    /// Create a new agent team
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(TeamConfig {
            name: name.into(),
            ..Default::default()
        })
    }

    /// Create a team with custom configuration
    #[must_use]
    pub fn with_config(config: TeamConfig) -> Self {
        let (tx, _) = broadcast::channel(100);

        Self {
            config,
            agents: Arc::new(RwLock::new(HashMap::new())),
            message_bus: tx,
            shared_context: Arc::new(RwLock::new(SharedContext::default())),
            task_history: Arc::new(RwLock::new(Vec::new())),
            llm_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the LLM client for this team.
    pub async fn set_llm_client(&self, client: Arc<dyn LlmClient>) {
        let mut llm = self.llm_client.write().await;
        *llm = Some(client);
    }

    /// Add an agent to the team.
    ///
    /// # Errors
    ///
    /// Returns an error if the maximum number of agents has been reached.
    pub async fn add_agent(&self, role: AgentRole) -> Result<String> {
        let mut agents = self.agents.write().await;

        if agents.len() >= self.config.max_agents {
            return Err(AgentError::InvalidState("Maximum agents reached".into()));
        }

        let id = Uuid::new_v4().to_string();
        let state = AgentState {
            id: id.clone(),
            role,
            status: AgentStatus::Idle,
            current_task: None,
            turn_count: 0,
            last_activity: Utc::now(),
            history: Vec::new(),
        };

        agents.insert(id.clone(), state);
        Ok(id)
    }

    /// Add an agent with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the maximum number of agents has been reached.
    pub async fn add_agent_with_config(&self, config: AgentConfig) -> Result<String> {
        let mut agents = self.agents.write().await;

        if agents.len() >= self.config.max_agents {
            return Err(AgentError::InvalidState("Maximum agents reached".into()));
        }

        let state = AgentState {
            id: config.id.clone(),
            role: config.role,
            status: AgentStatus::Idle,
            current_task: None,
            turn_count: 0,
            last_activity: Utc::now(),
            history: Vec::new(),
        };

        agents.insert(config.id.clone(), state);
        Ok(config.id)
    }

    /// Remove an agent from the team.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent with the given ID is not found.
    pub async fn remove_agent(&self, id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;
        agents
            .remove(id)
            .ok_or_else(|| AgentError::AgentNotFound(id.into()))?;
        Ok(())
    }

    /// Get all agents
    pub async fn list_agents(&self) -> Vec<AgentState> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Send a message to the team.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent (e.g., no receivers).
    pub fn broadcast(&self, message: AgentMessage) -> Result<()> {
        let _ = self.message_bus.send(message);
        Ok(())
    }

    /// Subscribe to team messages
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.message_bus.subscribe()
    }

    /// Update shared context
    pub async fn update_context<F>(&self, f: F)
    where
        F: FnOnce(&mut SharedContext),
    {
        let mut ctx = self.shared_context.write().await;
        f(&mut ctx);
    }

    /// Get shared context
    pub async fn get_context(&self) -> SharedContext {
        let ctx = self.shared_context.read().await;
        ctx.clone()
    }

    /// Execute a task with the team.
    ///
    /// When an LLM client is configured, the coordinator breaks the task into
    /// subtasks, dispatches each to the appropriate agent (which calls the LLM
    /// with its role-specific prompt), collects the results, and produces a
    /// final fused output.
    ///
    /// When no LLM client is set, the method falls back to a lightweight
    /// simulation that still broadcasts messages and records contributions.
    ///
    /// # Errors
    ///
    /// Returns an error if the team cannot be initialized or the task execution fails.
    pub async fn execute(&self, task: impl Into<String>) -> Result<TeamResult> {
        let task = task.into();
        let start = std::time::Instant::now();

        let llm = self.llm_client.read().await;
        if llm.is_some() {
            drop(llm);
            return self.execute_with_llm(&task, start).await;
        }
        drop(llm);

        self.execute_simulated(&task, start).await
    }

    async fn execute_simulated(&self, task: &str, start: std::time::Instant) -> Result<TeamResult> {
        let agents = self.agents.read().await;
        let has_coordinator = agents.values().any(|a| a.role == AgentRole::Coordinator);
        drop(agents);

        if !has_coordinator {
            self.add_agent(AgentRole::Coordinator).await?;
        }

        self.broadcast(AgentMessage {
            id: Uuid::new_v4().to_string(),
            sender: "system".into(),
            recipient: "all".into(),
            content: task.to_string(),
            message_type: MessageType::Task,
            timestamp: Utc::now(),
        })?;

        let mut total_turns = 0;
        let mut contributions = HashMap::new();

        {
            let agents = self.agents.read().await;
            for (id, state) in agents.iter() {
                contributions.insert(
                    id.clone(),
                    AgentContribution {
                        role: state.role.clone(),
                        message_count: state.turn_count,
                        highlights: Vec::new(),
                    },
                );
                total_turns += state.turn_count;
            }
        }

        let result = TeamResult {
            task: task.to_string(),
            success: true,
            output: format!("Task '{task}' completed by the team (simulated, no LLM client set)"),
            contributions,
            total_turns,
            elapsed_ms: start.elapsed().as_millis() as u64,
            timestamp: Utc::now(),
        };

        {
            let mut history = self.task_history.write().await;
            history.push(result.clone());
        }

        Ok(result)
    }

    async fn execute_with_llm(&self, task: &str, start: std::time::Instant) -> Result<TeamResult> {
        let agents = self.agents.read().await;
        let has_coordinator = agents.values().any(|a| a.role == AgentRole::Coordinator);
        let worker_roles: Vec<AgentRole> = agents
            .values()
            .filter(|a| a.role != AgentRole::Coordinator)
            .map(|a| a.role.clone())
            .collect();
        drop(agents);

        if !has_coordinator {
            self.add_agent(AgentRole::Coordinator).await?;
        }

        if worker_roles.is_empty() {
            return Err(AgentError::InvalidState(
                "Team needs at least one non-coordinator agent".into(),
            ));
        }

        self.broadcast(AgentMessage {
            id: Uuid::new_v4().to_string(),
            sender: "system".into(),
            recipient: "all".into(),
            content: task.to_string(),
            message_type: MessageType::Task,
            timestamp: Utc::now(),
        })?;

        let decomposition = self
            .coordinator_decompose_with_llm(task, &worker_roles)
            .await?;
        let sorted_indices = topological_sort(&decomposition.subtasks);

        let mut completed_results: HashMap<String, (SubTask, String)> = HashMap::new();
        let mut contributions: HashMap<String, AgentContribution> = HashMap::new();
        let mut total_turns = 0;

        for idx in &sorted_indices {
            let subtask = &decomposition.subtasks[*idx];
            let role = &subtask.assigned_role;

            let prior_context = subtask
                .depends_on
                .iter()
                .filter_map(|dep_id| {
                    completed_results
                        .get(dep_id)
                        .map(|(s, r)| format!("[{}]: {}", s.assigned_role.name(), r))
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            let user_prompt = build_worker_prompt(&subtask.description, &prior_context);

            let response = self.call_agent(role, &user_prompt).await.map_err(|e| {
                AgentError::TaskFailed(format!(
                    "Agent {} failed on subtask '{}': {e}",
                    role.name(),
                    subtask.id
                ))
            })?;

            total_turns += 1;

            let role_name = role.name().to_string();
            let entry =
                contributions
                    .entry(role_name.clone())
                    .or_insert_with(|| AgentContribution {
                        role: role.clone(),
                        message_count: 0,
                        highlights: Vec::new(),
                    });
            entry.message_count += 1;
            let highlight = response
                .lines()
                .next()
                .unwrap_or(&response)
                .chars()
                .take(120)
                .collect::<String>();
            entry.highlights.push(highlight);

            self.broadcast(AgentMessage {
                id: Uuid::new_v4().to_string(),
                sender: role_name.clone(),
                recipient: "all".into(),
                content: response.clone(),
                message_type: MessageType::Response,
                timestamp: Utc::now(),
            })?;

            completed_results.insert(subtask.id.clone(), (subtask.clone(), response));
        }

        let subtask_results: Vec<(SubTask, String)> = sorted_indices
            .iter()
            .filter_map(|i| completed_results.remove(&decomposition.subtasks[*i].id))
            .collect();

        let fused = self
            .coordinator_fuse_decomposed(task, &subtask_results)
            .await?;

        total_turns += 1;

        {
            let coord_entry = contributions
                .entry("coordinator".to_string())
                .or_insert_with(|| AgentContribution {
                    role: AgentRole::Coordinator,
                    message_count: 0,
                    highlights: Vec::new(),
                });
            coord_entry.message_count += 2;
            coord_entry
                .highlights
                .push("Decomposed task into subtasks with dependencies".into());
            coord_entry
                .highlights
                .push("Fused subtask results into final output".into());
        }

        let success = !fused.is_empty();

        let result = TeamResult {
            task: task.to_string(),
            success,
            output: fused,
            contributions,
            total_turns,
            elapsed_ms: start.elapsed().as_millis() as u64,
            timestamp: Utc::now(),
        };

        {
            let mut history = self.task_history.write().await;
            history.push(result.clone());
        }

        Ok(result)
    }

    async fn coordinator_decompose_with_llm(
        &self,
        task: &str,
        _worker_roles: &[AgentRole],
    ) -> Result<TaskDecomposition> {
        let llm = self.llm_client.read().await;
        let client = llm
            .as_ref()
            .ok_or_else(|| AgentError::InvalidState("No LLM client configured".into()))?;
        let client = Arc::clone(client);
        drop(llm);

        let decomposer = TaskDecomposer::new(client);
        decomposer.decompose(task, None).await
    }

    async fn coordinator_fuse_decomposed(
        &self,
        task: &str,
        subtask_results: &[(SubTask, String)],
    ) -> Result<String> {
        let results_text = subtask_results
            .iter()
            .map(|(s, r)| {
                let criteria = if s.acceptance_criteria.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\nAcceptance criteria: {}",
                        s.acceptance_criteria.join(", ")
                    )
                };
                format!(
                    "--- Subtask {} (assigned to {}, complexity: {:?}) ---\n{}\n{criteria}\n\nResult:\n{}",
                    s.id,
                    s.assigned_role.name(),
                    s.estimated_complexity,
                    s.description,
                    r
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            "You are a team coordinator. The team has completed subtasks for the original task.\n\n\
             Original task:\n{task}\n\n\
             Subtask results:\n{results_text}\n\n\
             Synthesize these results into a single coherent final output. Address the original task completely. \
             If there are conflicts between subtask results, resolve them. Provide the final answer directly."
        );

        self.call_agent(&AgentRole::Coordinator, &prompt).await
    }

    async fn call_agent(&self, role: &AgentRole, user_prompt: &str) -> Result<String> {
        let llm = self.llm_client.read().await;
        let client = llm
            .as_ref()
            .ok_or_else(|| AgentError::InvalidState("No LLM client configured".into()))?;

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: role.system_prompt(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: user_prompt.to_string(),
            },
        ];

        let response = client
            .chat(messages)
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;
        Ok(response)
    }

    /// Get task history
    pub async fn get_history(&self) -> Vec<TeamResult> {
        let history = self.task_history.read().await;
        history.clone()
    }

    /// Get team name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

fn build_worker_prompt(description: &str, prior_context: &str) -> String {
    let mut prompt = String::new();
    if !prior_context.is_empty() {
        prompt.push_str("Context from other agents:\n");
        prompt.push_str(prior_context);
        prompt.push_str("\n\n---\n\n");
    }
    prompt.push_str("Your assigned subtask:\n");
    prompt.push_str(description);
    prompt
}

#[cfg(test)]
fn parse_subtasks(response: &str) -> Result<Vec<Subtask>> {
    let cleaned = response.trim();
    let json_str = if cleaned.starts_with("```") {
        let without_fences = cleaned
            .strip_prefix("```json")
            .or_else(|| cleaned.strip_prefix("```"))
            .unwrap_or(cleaned);
        without_fences
            .strip_suffix("```")
            .unwrap_or(without_fences)
            .trim()
    } else {
        let start = cleaned.find('{').ok_or_else(|| {
            AgentError::TaskFailed("No JSON object found in coordinator response".into())
        })?;
        let end = cleaned.rfind('}').ok_or_else(|| {
            AgentError::TaskFailed("No closing brace in coordinator response".into())
        })?;
        &cleaned[start..=end]
    };

    #[derive(Deserialize)]
    struct SubtaskRaw {
        description: String,
        assigned_role: String,
        context: Option<String>,
    }

    #[derive(Deserialize)]
    struct CoordinatorOutput {
        subtasks: Vec<SubtaskRaw>,
    }

    let output: CoordinatorOutput = serde_json::from_str(json_str).map_err(|e| {
        AgentError::TaskFailed(format!("Failed to parse coordinator subtasks: {e}"))
    })?;

    let subtasks: Vec<Subtask> = output
        .subtasks
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let role = AgentRole::from_name(&s.assigned_role).unwrap_or(AgentRole::Coder);
            Subtask {
                id: format!("subtask-{i}"),
                description: s.description,
                assigned_role: role,
                context: s.context.unwrap_or_default(),
            }
        })
        .collect();

    if subtasks.is_empty() {
        return Err(AgentError::TaskFailed(
            "Coordinator produced zero subtasks".into(),
        ));
    }

    Ok(subtasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockLlmClient {
        responses: HashMap<String, String>,
    }

    impl MockLlmClient {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        fn with_coder_reviewer_responses() -> Self {
            let mut m = Self::new();
            let coordinator_decompose = r#"{
                "subtasks": [
                    {
                        "id": "1",
                        "description": "Implement the solution for the given task",
                        "assigned_role": "coder",
                        "depends_on": [],
                        "estimated_complexity": "simple",
                        "acceptance_criteria": ["Code compiles without errors"]
                    },
                    {
                        "id": "2",
                        "description": "Review the implemented solution for quality and correctness",
                        "assigned_role": "reviewer",
                        "depends_on": ["1"],
                        "estimated_complexity": "simple",
                        "acceptance_criteria": ["No bugs found"]
                    }
                ],
                "requires_human_review": false
            }"#.to_string();
            m.responses
                .insert("coordinator_decompose".to_string(), coordinator_decompose);
            m.responses.insert(
                "coder_response".to_string(),
                "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            );
            m.responses.insert(
                "reviewer_response".to_string(),
                "LGTM: The function is correct and concise.".to_string(),
            );
            m.responses.insert(
                "coordinator_fuse".to_string(),
                "The task is complete. The coder implemented a simple addition function, \
                 and the reviewer confirmed it is correct."
                    .to_string(),
            );
            m
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, messages: Vec<ChatMessage>) -> crate::Result<String> {
            let user_msg = messages
                .iter()
                .find(|m| m.role == ChatRole::User)
                .map(|m| m.content.as_str())
                .unwrap_or("");

            if user_msg.contains("Break the following task")
                || user_msg.contains("task decomposition engine")
            {
                if let Some(r) = self.responses.get("coordinator_decompose") {
                    return Ok(r.clone());
                }
            }
            if user_msg.contains("Synthesize these results") {
                if let Some(r) = self.responses.get("coordinator_fuse") {
                    return Ok(r.clone());
                }
            }

            let system_msg = messages.first().map(|m| m.content.as_str()).unwrap_or("");

            if system_msg.contains("coder") {
                if let Some(r) = self.responses.get("coder_response") {
                    return Ok(r.clone());
                }
            }
            if system_msg.contains("reviewer") {
                if let Some(r) = self.responses.get("reviewer_response") {
                    return Ok(r.clone());
                }
            }
            if system_msg.contains("architect") {
                if let Some(r) = self.responses.get("architect_response") {
                    return Ok(r.clone());
                }
            }

            Ok("Mock response".to_string())
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> crate::Result<tokio::sync::mpsc::Receiver<String>> {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let _ = tx.send("mock stream".to_string()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    #[test]
    fn test_agent_role_prompts() {
        let coder = AgentRole::Coder;
        assert!(coder.system_prompt().contains("coder"));

        let custom = AgentRole::Custom {
            name: "security".into(),
            instructions: "Focus on security vulnerabilities".into(),
        };
        assert!(custom.system_prompt().contains("security"));
    }

    #[tokio::test]
    async fn test_team_creation() {
        let team = AgentTeam::new("test-team");
        assert_eq!(team.name(), "test-team");
    }

    #[tokio::test]
    async fn test_add_agent() {
        let team = AgentTeam::new("test-team");
        let id = team.add_agent(AgentRole::Coder).await.unwrap();
        assert!(!id.is_empty());

        let agents = team.list_agents().await;
        assert_eq!(agents.len(), 1);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let team = AgentTeam::new("test-team");
        let mut rx = team.subscribe();

        let msg = AgentMessage {
            id: "msg-1".into(),
            sender: "agent-1".into(),
            recipient: "all".into(),
            content: "Hello team".into(),
            message_type: MessageType::Status,
            timestamp: Utc::now(),
        };

        team.broadcast(msg.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "Hello team");
    }

    #[tokio::test]
    async fn test_shared_context() {
        let team = AgentTeam::new("test-team");

        team.update_context(|ctx| {
            ctx.decisions.push("Use Rust".into());
        })
        .await;

        let ctx = team.get_context().await;
        assert_eq!(ctx.decisions.len(), 1);
    }

    #[test]
    fn test_parse_subtasks_valid_json() {
        let json = r#"{"subtasks": [
            {"description": "Implement feature X", "assigned_role": "coder", "context": ""},
            {"description": "Review feature X", "assigned_role": "reviewer", "context": ""}
        ]}"#;

        let result = parse_subtasks(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].assigned_role, AgentRole::Coder);
        assert_eq!(result[1].assigned_role, AgentRole::Reviewer);
    }

    #[test]
    fn test_parse_subtasks_with_markdown_fences() {
        let json = "```json\n{\"subtasks\": [\n  {\"description\": \"Do the thing\", \"assigned_role\": \"coder\"}\n]}\n```";

        let result = parse_subtasks(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Do the thing");
    }

    #[test]
    fn test_parse_subtasks_embedded_in_text() {
        let json = "Sure, here is the plan:\n\n{\"subtasks\": [{\"description\": \"Fix bug\", \"assigned_role\": \"debugger\"}]}\n\nLet me know if you need more.";

        let result = parse_subtasks(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].assigned_role, AgentRole::Debugger);
    }

    #[test]
    fn test_parse_subtasks_unknown_role_falls_back_to_coder() {
        let json =
            r#"{"subtasks": [{"description": "Something", "assigned_role": "nonexistent_role"}]}"#;

        let result = parse_subtasks(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].assigned_role, AgentRole::Coder);
    }

    #[test]
    fn test_parse_subtasks_empty_subtasks_errors() {
        let json = r#"{"subtasks": []}"#;
        assert!(parse_subtasks(json).is_err());
    }

    #[test]
    fn test_parse_subtasks_invalid_json_errors() {
        assert!(parse_subtasks("not json at all").is_err());
    }

    #[test]
    fn test_build_worker_prompt_no_context() {
        let prompt = build_worker_prompt("Write tests", "");
        assert!(!prompt.contains("Context from other agents"));
        assert!(prompt.contains("Write tests"));
    }

    #[test]
    fn test_build_worker_prompt_with_context() {
        let prompt = build_worker_prompt("Review code", "[coder]: fn add(a,b){a+b}");
        assert!(prompt.contains("Context from other agents"));
        assert!(prompt.contains("Review code"));
        assert!(prompt.contains("coder"));
    }

    #[tokio::test]
    async fn test_execute_with_mock_llm_coder_and_reviewer() {
        let team = AgentTeam::new("llm-test-team");
        team.add_agent(AgentRole::Coder).await.unwrap();
        team.add_agent(AgentRole::Reviewer).await.unwrap();
        team.set_llm_client(Arc::new(MockLlmClient::with_coder_reviewer_responses()))
            .await;

        let result = team
            .execute("Implement an addition function")
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("addition function"));
        assert!(result.total_turns >= 3);
        assert!(result.contributions.contains_key("coder"));
        assert!(result.contributions.contains_key("reviewer"));
        assert!(result.contributions.contains_key("coordinator"));

        let coder_contrib = &result.contributions["coder"];
        assert_eq!(coder_contrib.message_count, 1);
        assert!(!coder_contrib.highlights.is_empty());

        let reviewer_contrib = &result.contributions["reviewer"];
        assert_eq!(reviewer_contrib.message_count, 1);
    }

    #[tokio::test]
    async fn test_execute_without_llm_falls_back_to_simulation() {
        let team = AgentTeam::new("sim-team");
        team.add_agent(AgentRole::Coder).await.unwrap();

        let result = team.execute("Some task").await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("simulated"));
    }

    #[tokio::test]
    async fn test_execute_with_llm_but_no_workers_errors() {
        let team = AgentTeam::new("empty-team");
        team.set_llm_client(Arc::new(MockLlmClient::new())).await;

        let result = team.execute("Some task").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-coordinator agent"));
    }

    #[tokio::test]
    async fn test_execute_records_history() {
        let team = AgentTeam::new("history-team");
        team.add_agent(AgentRole::Coder).await.unwrap();

        team.execute("Task A").await.unwrap();
        team.execute("Task B").await.unwrap();

        let history = team.get_history().await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].task, "Task A");
        assert_eq!(history[1].task, "Task B");
    }

    #[tokio::test]
    async fn test_single_worker_role_creates_single_subtask() {
        let team = AgentTeam::new("solo-team");
        team.add_agent(AgentRole::Architect).await.unwrap();

        let mut mock = MockLlmClient::new();
        mock.responses.insert(
            "coordinator_decompose".to_string(),
            r#"{"subtasks": [{"id": "1", "description": "Design the REST API architecture", "assigned_role": "architect", "depends_on": [], "estimated_complexity": "moderate", "acceptance_criteria": ["API endpoints defined"]}], "requires_human_review": false}"#.to_string(),
        );
        mock.responses.insert(
            "architect_response".to_string(),
            "Designed a REST API with /users and /orders endpoints.".to_string(),
        );
        mock.responses.insert(
            "coordinator_fuse".to_string(),
            "REST API design is complete.".to_string(),
        );
        team.set_llm_client(Arc::new(mock)).await;

        let result = team.execute("Design a REST API").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_task_decomposition_valid() {
        let json = r#"{
            "subtasks": [
                {
                    "id": "1",
                    "description": "Research existing auth solutions",
                    "assigned_role": "researcher",
                    "depends_on": [],
                    "estimated_complexity": "simple",
                    "acceptance_criteria": ["Survey completed"]
                },
                {
                    "id": "2",
                    "description": "Implement user authentication",
                    "assigned_role": "coder",
                    "depends_on": ["1"],
                    "estimated_complexity": "complex",
                    "acceptance_criteria": ["Login works", "Signup works"]
                },
                {
                    "id": "3",
                    "description": "Review auth implementation",
                    "assigned_role": "reviewer",
                    "depends_on": ["2"],
                    "estimated_complexity": "moderate",
                    "acceptance_criteria": ["No security issues"]
                }
            ],
            "requires_human_review": true
        }"#;

        let result = parse_task_decomposition("Add user authentication", json).unwrap();
        assert_eq!(result.original_task, "Add user authentication");
        assert_eq!(result.subtasks.len(), 3);
        assert_eq!(result.total_estimated_steps, 3);
        assert!(result.requires_human_review);

        assert_eq!(result.subtasks[0].assigned_role, AgentRole::Researcher);
        assert_eq!(result.subtasks[0].depends_on.len(), 0);
        assert_eq!(
            result.subtasks[0].estimated_complexity,
            TaskComplexity::Simple
        );

        assert_eq!(result.subtasks[1].assigned_role, AgentRole::Coder);
        assert_eq!(result.subtasks[1].depends_on, vec!["1"]);
        assert_eq!(
            result.subtasks[1].estimated_complexity,
            TaskComplexity::Complex
        );
        assert_eq!(result.subtasks[1].acceptance_criteria.len(), 2);

        assert_eq!(result.subtasks[2].depends_on, vec!["2"]);
    }

    #[test]
    fn test_parse_task_decomposition_with_markdown_fences() {
        let json = "```json\n{\"subtasks\": [{\"id\": \"1\", \"description\": \"Do the thing\", \"assigned_role\": \"coder\", \"depends_on\": [], \"estimated_complexity\": \"trivial\", \"acceptance_criteria\": []}], \"requires_human_review\": false}\n```";

        let result = parse_task_decomposition("Some task", json).unwrap();
        assert_eq!(result.subtasks.len(), 1);
        assert_eq!(result.subtasks[0].description, "Do the thing");
        assert_eq!(
            result.subtasks[0].estimated_complexity,
            TaskComplexity::Trivial
        );
    }

    #[test]
    fn test_parse_task_decomposition_empty_subtasks_errors() {
        let json = r#"{"subtasks": [], "requires_human_review": false}"#;
        let result = parse_task_decomposition("task", json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("zero subtasks"));
    }

    #[test]
    fn test_parse_task_decomposition_invalid_json_errors() {
        let result = parse_task_decomposition("task", "not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_task_decomposition_empty_response_errors() {
        let result = parse_task_decomposition("task", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_subtask_dag_valid() {
        let subtasks = vec![
            SubTask {
                id: "1".into(),
                description: "Research".into(),
                assigned_role: AgentRole::Researcher,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "2".into(),
                description: "Implement".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["1".into()],
                estimated_complexity: TaskComplexity::Moderate,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "3".into(),
                description: "Review".into(),
                assigned_role: AgentRole::Reviewer,
                depends_on: vec!["2".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
        ];
        assert!(validate_subtask_dag(&subtasks).is_ok());
    }

    #[test]
    fn test_validate_subtask_dag_cycle_detected() {
        let subtasks = vec![
            SubTask {
                id: "1".into(),
                description: "A".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["2".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "2".into(),
                description: "B".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["1".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
        ];
        let result = validate_subtask_dag(&subtasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_validate_subtask_dag_unknown_dependency_errors() {
        let subtasks = vec![SubTask {
            id: "1".into(),
            description: "A".into(),
            assigned_role: AgentRole::Coder,
            depends_on: vec!["999".into()],
            estimated_complexity: TaskComplexity::Simple,
            acceptance_criteria: vec![],
        }];
        let result = validate_subtask_dag(&subtasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown subtask"));
    }

    #[test]
    fn test_topological_sort_respects_dependencies() {
        let subtasks = vec![
            SubTask {
                id: "1".into(),
                description: "Research".into(),
                assigned_role: AgentRole::Researcher,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "2".into(),
                description: "Implement".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["1".into()],
                estimated_complexity: TaskComplexity::Moderate,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "3".into(),
                description: "Review".into(),
                assigned_role: AgentRole::Reviewer,
                depends_on: vec!["2".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
        ];

        let order = topological_sort(&subtasks);
        assert_eq!(order.len(), 3);

        let pos: std::collections::HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(pos, idx)| (subtasks[*idx].id.as_str(), pos))
            .collect();

        assert!(pos["1"] < pos["2"], "Research must come before Implement");
        assert!(pos["2"] < pos["3"], "Implement must come before Review");
    }

    #[test]
    fn test_topological_sort_independent_subtasks_all_included() {
        let subtasks = vec![
            SubTask {
                id: "a".into(),
                description: "Task A".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Trivial,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "b".into(),
                description: "Task B".into(),
                assigned_role: AgentRole::Tester,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Trivial,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "c".into(),
                description: "Task C".into(),
                assigned_role: AgentRole::Reviewer,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Trivial,
                acceptance_criteria: vec![],
            },
        ];

        let order = topological_sort(&subtasks);
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_topological_sort_diamond_dependency() {
        let subtasks = vec![
            SubTask {
                id: "root".into(),
                description: "Root".into(),
                assigned_role: AgentRole::Architect,
                depends_on: vec![],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "left".into(),
                description: "Left".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["root".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "right".into(),
                description: "Right".into(),
                assigned_role: AgentRole::Coder,
                depends_on: vec!["root".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
            SubTask {
                id: "join".into(),
                description: "Join".into(),
                assigned_role: AgentRole::Reviewer,
                depends_on: vec!["left".into(), "right".into()],
                estimated_complexity: TaskComplexity::Simple,
                acceptance_criteria: vec![],
            },
        ];

        let order = topological_sort(&subtasks);
        assert_eq!(order.len(), 4);

        let pos: std::collections::HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(pos, idx)| (subtasks[*idx].id.as_str(), pos))
            .collect();

        assert!(pos["root"] < pos["left"]);
        assert!(pos["root"] < pos["right"]);
        assert!(pos["left"] < pos["join"]);
        assert!(pos["right"] < pos["join"]);
    }

    #[test]
    fn test_parse_decompose_role_maps_security_to_reviewer() {
        assert_eq!(parse_decompose_role("security"), AgentRole::Reviewer);
        assert_eq!(parse_decompose_role("executor"), AgentRole::Coder);
        assert_eq!(parse_decompose_role("coder"), AgentRole::Coder);
        assert_eq!(parse_decompose_role("reviewer"), AgentRole::Reviewer);
        assert_eq!(parse_decompose_role("tester"), AgentRole::Tester);
        assert_eq!(parse_decompose_role("architect"), AgentRole::Architect);
        assert_eq!(parse_decompose_role("researcher"), AgentRole::Researcher);
        assert_eq!(parse_decompose_role("debugger"), AgentRole::Debugger);
        assert_eq!(parse_decompose_role("unknown"), AgentRole::Coder);
    }

    #[tokio::test]
    async fn test_task_decomposer_with_mock_llm() {
        let mut mock = MockLlmClient::new();
        mock.responses.insert(
            "coordinator_decompose".to_string(),
            r#"{
                "subtasks": [
                    {
                        "id": "1",
                        "description": "Analyze the codebase structure",
                        "assigned_role": "researcher",
                        "depends_on": [],
                        "estimated_complexity": "simple",
                        "acceptance_criteria": ["Structure documented"]
                    },
                    {
                        "id": "2",
                        "description": "Implement the feature",
                        "assigned_role": "coder",
                        "depends_on": ["1"],
                        "estimated_complexity": "moderate",
                        "acceptance_criteria": ["Tests pass"]
                    }
                ],
                "requires_human_review": false
            }"#
            .to_string(),
        );

        let decomposer = TaskDecomposer::new(Arc::new(mock));
        let result = decomposer
            .decompose("Add caching layer", Some("Using Redis"))
            .await
            .unwrap();

        assert_eq!(result.original_task, "Add caching layer");
        assert_eq!(result.subtasks.len(), 2);
        assert!(!result.requires_human_review);
        assert_eq!(result.subtasks[0].assigned_role, AgentRole::Researcher);
        assert_eq!(result.subtasks[1].depends_on, vec!["1"]);
    }
}
