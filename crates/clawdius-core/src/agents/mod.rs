//! Agent Teams / Swarms Coordination
//!
//! This module implements multi-agent coordination for complex tasks.
//! Inspired by NanoClaw and Claude Code's agent teams feature.
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
//! ```no_run
//! use clawdius_core::agents::{AgentTeam, AgentRole, TeamConfig};
//!
//! let mut team = AgentTeam::new("code-review-team");
//! team.add_agent(AgentRole::Coder);
//! team.add_agent(AgentRole::Reviewer);
//! team.add_agent(AgentRole::Tester);
//!
//! let result = team.execute("Implement user authentication").await?;
//! ```

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
                format!("You are a {} agent. {}", name, instructions)
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
        }
    }

    /// Add an agent to the team
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

    /// Add an agent with custom configuration
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

    /// Remove an agent from the team
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

    /// Send a message to the team
    pub fn broadcast(&self, message: AgentMessage) -> Result<()> {
        self.message_bus
            .send(message)
            .map_err(|e| AgentError::CommunicationError(e.to_string()))?;
        Ok(())
    }

    /// Subscribe to team messages
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

    /// Execute a task with the team
    pub async fn execute(&self, task: impl Into<String>) -> Result<TeamResult> {
        let task = task.into();
        let start = std::time::Instant::now();

        // Initialize coordinator if not present
        let agents = self.agents.read().await;
        let has_coordinator = agents.values().any(|a| a.role == AgentRole::Coordinator);
        drop(agents);

        if !has_coordinator {
            self.add_agent(AgentRole::Coordinator).await?;
        }

        // Broadcast task to team
        self.broadcast(AgentMessage {
            id: Uuid::new_v4().to_string(),
            sender: "system".into(),
            recipient: "all".into(),
            content: task.clone(),
            message_type: MessageType::Task,
            timestamp: Utc::now(),
        })?;

        // Simulate team execution (in a real implementation, this would
        // involve actual LLM calls and agent interactions)
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
            task: task.clone(),
            success: true,
            output: format!("Task '{}' completed by the team", task),
            contributions,
            total_turns,
            elapsed_ms: start.elapsed().as_millis() as u64,
            timestamp: Utc::now(),
        };

        // Record in history
        {
            let mut history = self.task_history.write().await;
            history.push(result.clone());
        }

        Ok(result)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
