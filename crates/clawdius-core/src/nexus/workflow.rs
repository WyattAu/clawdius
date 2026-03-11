//! Workflow Orchestration for Nexus FSM Phase 3
//!
//! This module implements multi-phase workflow composition, parallel task execution,
//! and dependency resolution between workflow stages.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

use super::{ArtifactId, NexusError, PhaseCategory, PhaseId, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new(id: impl Into<String>) -> Self {
        WorkflowId(id.into())
    }

    #[must_use]
    pub fn generate() -> Self {
        WorkflowId(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub phase: PhaseId,
    pub dependencies: Vec<String>,
    pub artifact_requirements: Vec<ArtifactId>,
    pub produces_artifacts: Vec<String>,
    pub parallel_group: Option<String>,
    pub timeout_ms: Option<u64>,
    pub retry_count: u32,
    pub metadata: serde_json::Value,
}

impl TaskDefinition {
    pub fn new(id: impl Into<String>, phase: PhaseId) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            description: String::new(),
            phase,
            dependencies: Vec::new(),
            artifact_requirements: Vec::new(),
            produces_artifacts: Vec::new(),
            parallel_group: None,
            timeout_ms: None,
            retry_count: 0,
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

    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    #[must_use]
    pub fn with_artifact_requirement(mut self, artifact: ArtifactId) -> Self {
        self.artifact_requirements.push(artifact);
        self
    }

    pub fn with_produced_artifact(mut self, artifact: impl Into<String>) -> Self {
        self.produces_artifacts.push(artifact.into());
        self
    }

    pub fn with_parallel_group(mut self, group: impl Into<String>) -> Self {
        self.parallel_group = Some(group.into());
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[must_use]
    pub fn with_retry(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::Ready => write!(f, "Ready"),
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed => write!(f, "Failed"),
            TaskStatus::Skipped => write!(f, "Skipped"),
            TaskStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    pub task_id: String,
    pub status: TaskStatus,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub attempt: u32,
    pub artifacts_produced: Vec<ArtifactId>,
}

impl TaskExecution {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            status: TaskStatus::Pending,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            error: None,
            attempt: 0,
            artifacts_produced: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(chrono::Utc::now());
        self.attempt += 1;
    }

    pub fn complete(&mut self, artifacts: Vec<ArtifactId>) {
        self.status = TaskStatus::Completed;
        self.completed_at = Some(chrono::Utc::now());
        self.artifacts_produced = artifacts;
        if let Some(started) = self.started_at {
            self.duration_ms = Some((chrono::Utc::now() - started).num_milliseconds() as u64);
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed;
        self.completed_at = Some(chrono::Utc::now());
        self.error = Some(error.into());
        if let Some(started) = self.started_at {
            self.duration_ms = Some((chrono::Utc::now() - started).num_milliseconds() as u64);
        }
    }

    pub fn skip(&mut self, reason: impl Into<String>) {
        self.status = TaskStatus::Skipped;
        self.error = Some(reason.into());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Created,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: WorkflowId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub tasks: Vec<TaskDefinition>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl WorkflowDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: WorkflowId::generate(),
            name: name.into(),
            description: String::new(),
            version: "1.0.0".to_string(),
            tasks: Vec::new(),
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    #[must_use]
    pub fn with_task(mut self, task: TaskDefinition) -> Self {
        self.tasks.push(task);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    #[must_use]
    pub fn get_task(&self, id: &str) -> Option<&TaskDefinition> {
        self.tasks.iter().find(|t| t.id == id)
    }

    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub workflow_id: WorkflowId,
    pub execution_id: String,
    pub status: WorkflowStatus,
    pub task_executions: HashMap<String, TaskExecution>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<u64>,
    pub context: serde_json::Value,
}

impl WorkflowExecution {
    #[must_use]
    pub fn new(workflow_id: WorkflowId) -> Self {
        Self {
            execution_id: uuid::Uuid::new_v4().to_string(),
            workflow_id,
            status: WorkflowStatus::Created,
            task_executions: HashMap::new(),
            started_at: None,
            completed_at: None,
            duration_ms: None,
            context: serde_json::json!({}),
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    pub fn initialize_tasks(&mut self, tasks: &[TaskDefinition]) {
        for task in tasks {
            self.task_executions
                .insert(task.id.clone(), TaskExecution::new(&task.id));
        }
    }

    pub fn start(&mut self) {
        self.status = WorkflowStatus::Running;
        self.started_at = Some(chrono::Utc::now());
    }

    pub fn complete(&mut self) {
        self.status = WorkflowStatus::Completed;
        self.completed_at = Some(chrono::Utc::now());
        if let Some(started) = self.started_at {
            self.duration_ms = Some((chrono::Utc::now() - started).num_milliseconds() as u64);
        }
    }

    pub fn fail(&mut self) {
        self.status = WorkflowStatus::Failed;
        self.completed_at = Some(chrono::Utc::now());
        if let Some(started) = self.started_at {
            self.duration_ms = Some((chrono::Utc::now() - started).num_milliseconds() as u64);
        }
    }

    #[must_use]
    pub fn completed_task_count(&self) -> usize {
        self.task_executions
            .values()
            .filter(|t| t.status == TaskStatus::Completed)
            .count()
    }

    #[must_use]
    pub fn failed_task_count(&self) -> usize {
        self.task_executions
            .values()
            .filter(|t| t.status == TaskStatus::Failed)
            .count()
    }

    #[must_use]
    pub fn progress_percent(&self) -> f64 {
        if self.task_executions.is_empty() {
            return 0.0;
        }
        let completed = self.completed_task_count() as f64;
        let total = self.task_executions.len() as f64;
        (completed / total) * 100.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),

    #[error("Task execution failed: {0}")]
    TaskExecutionFailed(String),

    #[error("Workflow already running: {0}")]
    AlreadyRunning(String),

    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    #[error("Invalid workflow state: {0}")]
    InvalidState(String),

    #[error("Timeout exceeded for task: {0}")]
    TimeoutExceeded(String),

    #[error("Max parallelism reached")]
    MaxParallelismReached,
}

#[derive(Debug, Clone)]
pub struct DependencyGraph {
    nodes: HashMap<String, HashSet<String>>,
    reverse_edges: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: &str) {
        self.nodes.entry(id.to_string()).or_default();
        self.reverse_edges.entry(id.to_string()).or_default();
    }

    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.add_node(from);
        self.add_node(to);

        self.nodes.get_mut(from).unwrap().insert(to.to_string());
        self.reverse_edges
            .get_mut(to)
            .unwrap()
            .insert(from.to_string());
    }

    #[must_use]
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in self.nodes.keys() {
            if self.has_cycle_util(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn has_cycle_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        if rec_stack.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }

        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(deps) = self.nodes.get(node) {
            for dep in deps {
                if self.has_cycle_util(dep, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    pub fn topological_sort(&self) -> Result<Vec<String>> {
        if self.has_cycle() {
            return Err(NexusError::LockError(
                "Circular dependency detected".to_string(),
            ));
        }

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp = HashSet::new();

        for node in self.nodes.keys() {
            self.topological_sort_util(node, &mut visited, &mut temp, &mut result)?;
        }

        result.reverse();
        Ok(result)
    }

    fn topological_sort_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        temp: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> Result<()> {
        if temp.contains(node) {
            return Err(NexusError::LockError(
                "Circular dependency detected".to_string(),
            ));
        }
        if visited.contains(node) {
            return Ok(());
        }

        temp.insert(node.to_string());

        if let Some(deps) = self.nodes.get(node) {
            for dep in deps {
                self.topological_sort_util(dep, visited, temp, result)?;
            }
        }

        temp.remove(node);
        visited.insert(node.to_string());
        result.push(node.to_string());
        Ok(())
    }

    #[must_use]
    pub fn get_ready_tasks(&self, completed: &HashSet<String>) -> Vec<String> {
        self.nodes
            .keys()
            .filter(|node| {
                !completed.contains(*node)
                    && self
                        .nodes
                        .get(*node)
                        .is_none_or(|deps| deps.iter().all(|d| completed.contains(d)))
            })
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn get_dependents(&self, task_id: &str) -> Vec<String> {
        self.reverse_edges
            .get(task_id)
            .map(|deps| deps.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParallelConfig {
    pub max_concurrent_tasks: usize,
    pub task_timeout_ms: u64,
    pub retry_failed_tasks: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 4,
            task_timeout_ms: 300_000,
            retry_failed_tasks: true,
        }
    }
}

pub struct WorkflowOrchestrator {
    workflows: Arc<RwLock<HashMap<WorkflowId, WorkflowDefinition>>>,
    executions: Arc<RwLock<HashMap<String, WorkflowExecution>>>,
    config: ParallelConfig,
    semaphore: Arc<Semaphore>,
}

impl WorkflowOrchestrator {
    #[must_use]
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_tasks)),
            config,
        }
    }

    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(ParallelConfig::default())
    }

    pub async fn register_workflow(&self, workflow: WorkflowDefinition) -> Result<WorkflowId> {
        self.validate_workflow(&workflow)?;

        let id = workflow.id.clone();
        self.workflows.write().await.insert(id.clone(), workflow);
        Ok(id)
    }

    fn validate_workflow(&self, workflow: &WorkflowDefinition) -> Result<()> {
        let mut graph = DependencyGraph::new();

        for task in &workflow.tasks {
            graph.add_node(&task.id);
            for dep in &task.dependencies {
                graph.add_dependency(&task.id, dep);
            }
        }

        if graph.has_cycle() {
            return Err(NexusError::LockError(
                "Workflow contains circular dependencies".to_string(),
            ));
        }

        for task in &workflow.tasks {
            for dep in &task.dependencies {
                if !workflow.tasks.iter().any(|t| t.id == *dep) {
                    return Err(NexusError::LockError(format!(
                        "Task '{}' depends on non-existent task '{}'",
                        task.id, dep
                    )));
                }
            }
        }

        Ok(())
    }

    pub async fn start_workflow(&self, workflow_id: &WorkflowId) -> Result<String> {
        let workflows = self.workflows.read().await;
        let workflow = workflows
            .get(workflow_id)
            .ok_or_else(|| NexusError::LockError(format!("Workflow not found: {workflow_id}")))?;

        let mut execution = WorkflowExecution::new(workflow_id.clone());
        execution.initialize_tasks(&workflow.tasks);
        execution.start();

        let execution_id = execution.execution_id.clone();
        self.executions
            .write()
            .await
            .insert(execution_id.clone(), execution);

        Ok(execution_id)
    }

    pub async fn get_execution(&self, execution_id: &str) -> Option<WorkflowExecution> {
        self.executions.read().await.get(execution_id).cloned()
    }

    pub async fn get_ready_tasks(&self, execution_id: &str) -> Result<Vec<String>> {
        let executions = self.executions.read().await;
        let execution = executions
            .get(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        let workflows = self.workflows.read().await;
        let workflow = workflows
            .get(&execution.workflow_id)
            .ok_or_else(|| NexusError::LockError("Workflow not found".to_string()))?;

        let mut graph = DependencyGraph::new();
        for task in &workflow.tasks {
            graph.add_node(&task.id);
            for dep in &task.dependencies {
                graph.add_dependency(&task.id, dep);
            }
        }

        let completed: HashSet<String> = execution
            .task_executions
            .values()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.task_id.clone())
            .collect();

        let pending: HashSet<String> = execution
            .task_executions
            .values()
            .filter(|t| t.status == TaskStatus::Pending || t.status == TaskStatus::Ready)
            .map(|t| t.task_id.clone())
            .collect();

        let ready: Vec<String> = graph
            .get_ready_tasks(&completed)
            .into_iter()
            .filter(|t| pending.contains(t))
            .collect();

        Ok(ready)
    }

    pub async fn update_task_status(
        &self,
        execution_id: &str,
        task_id: &str,
        status: TaskStatus,
        error: Option<String>,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions
            .get_mut(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        if let Some(task_exec) = execution.task_executions.get_mut(task_id) {
            task_exec.status = status;
            task_exec.error = error;
        }

        Ok(())
    }

    pub async fn start_task(&self, execution_id: &str, task_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions
            .get_mut(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        if let Some(task_exec) = execution.task_executions.get_mut(task_id) {
            task_exec.start();
        }

        Ok(())
    }

    pub async fn complete_task(
        &self,
        execution_id: &str,
        task_id: &str,
        artifacts: Vec<ArtifactId>,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions
            .get_mut(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        if let Some(task_exec) = execution.task_executions.get_mut(task_id) {
            task_exec.complete(artifacts);
        }

        let all_completed = execution
            .task_executions
            .values()
            .all(|t| t.status == TaskStatus::Completed);

        if all_completed {
            execution.complete();
        }

        Ok(())
    }

    pub async fn fail_task(
        &self,
        execution_id: &str,
        task_id: &str,
        error: impl Into<String>,
    ) -> Result<()> {
        let mut executions = self.executions.write().await;
        let execution = executions
            .get_mut(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        if let Some(task_exec) = execution.task_executions.get_mut(task_id) {
            task_exec.fail(error);
        }

        Ok(())
    }

    pub async fn get_parallel_groups(
        &self,
        execution_id: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        let executions = self.executions.read().await;
        let execution = executions
            .get(execution_id)
            .ok_or_else(|| NexusError::LockError(format!("Execution not found: {execution_id}")))?;

        let workflows = self.workflows.read().await;
        let workflow = workflows
            .get(&execution.workflow_id)
            .ok_or_else(|| NexusError::LockError("Workflow not found".to_string()))?;

        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for task in &workflow.tasks {
            if let Some(group) = &task.parallel_group {
                groups
                    .entry(group.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }

        Ok(groups)
    }

    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        if let Some(execution) = executions.get_mut(execution_id) {
            execution.status = WorkflowStatus::Cancelled;
            execution.completed_at = Some(chrono::Utc::now());

            for task_exec in execution.task_executions.values_mut() {
                if task_exec.status == TaskStatus::Pending || task_exec.status == TaskStatus::Ready
                {
                    task_exec.status = TaskStatus::Cancelled;
                }
            }
        }
        Ok(())
    }

    pub async fn list_workflows(&self) -> Vec<WorkflowDefinition> {
        self.workflows.read().await.values().cloned().collect()
    }

    pub async fn list_executions(&self) -> Vec<WorkflowExecution> {
        self.executions.read().await.values().cloned().collect()
    }

    pub async fn get_workflow(&self, id: &WorkflowId) -> Option<WorkflowDefinition> {
        self.workflows.read().await.get(id).cloned()
    }

    #[must_use]
    pub fn semaphore(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    #[must_use]
    pub fn config(&self) -> &ParallelConfig {
        &self.config
    }
}

impl std::fmt::Debug for WorkflowOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowOrchestrator")
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseWorkflow {
    pub start_phase: PhaseId,
    pub end_phase: PhaseId,
    pub category: PhaseCategory,
    pub checkpoint_after: bool,
    pub required_gates: Vec<String>,
    pub skip_on_failure: bool,
}

impl PhaseWorkflow {
    #[must_use]
    pub fn new(start: PhaseId, end: PhaseId) -> Self {
        let category = PhaseCategory::from_phase_number(start.0);
        Self {
            start_phase: start,
            end_phase: end,
            category,
            checkpoint_after: true,
            required_gates: Vec::new(),
            skip_on_failure: false,
        }
    }

    #[must_use]
    pub fn with_checkpoint(mut self, checkpoint: bool) -> Self {
        self.checkpoint_after = checkpoint;
        self
    }

    pub fn with_required_gate(mut self, gate: impl Into<String>) -> Self {
        self.required_gates.push(gate.into());
        self
    }

    #[must_use]
    pub fn with_skip_on_failure(mut self, skip: bool) -> Self {
        self.skip_on_failure = skip;
        self
    }

    pub fn phases(&self) -> Vec<PhaseId> {
        (self.start_phase.0..=self.end_phase.0)
            .map(PhaseId)
            .collect()
    }
}

#[must_use]
pub fn create_standard_workflow() -> WorkflowDefinition {
    WorkflowDefinition::new("Standard R&D Lifecycle")
        .with_description("Complete 24-phase R&D lifecycle workflow")
        .with_version("1.0.0")
        .with_task(
            TaskDefinition::new("discovery", PhaseId(0))
                .with_name("Discovery Phase")
                .with_dependency("env_setup")
                .with_parallel_group("initial"),
        )
        .with_task(
            TaskDefinition::new("env_setup", PhaseId(1))
                .with_name("Environment Setup")
                .with_parallel_group("initial"),
        )
        .with_task(
            TaskDefinition::new("requirements", PhaseId(2))
                .with_name("Requirements Engineering")
                .with_dependency("discovery"),
        )
        .with_task(
            TaskDefinition::new("research", PhaseId(3))
                .with_name("Research & Yellow Paper")
                .with_dependency("requirements")
                .with_produced_artifact("yellow_paper"),
        )
        .with_task(
            TaskDefinition::new("architecture", PhaseId(6))
                .with_name("Architecture & Blue Paper")
                .with_dependency("research")
                .with_produced_artifact("blue_paper"),
        )
        .with_task(
            TaskDefinition::new("implementation", PhaseId(13))
                .with_name("Implementation")
                .with_dependency("architecture")
                .with_parallel_group("build"),
        )
        .with_task(
            TaskDefinition::new("testing", PhaseId(16))
                .with_name("Testing & Verification")
                .with_dependency("implementation")
                .with_parallel_group("build"),
        )
        .with_task(
            TaskDefinition::new("deployment", PhaseId(18))
                .with_name("Deployment")
                .with_dependency("testing"),
        )
        .with_task(
            TaskDefinition::new("finalization", PhaseId(23))
                .with_name("Finalization & Archive")
                .with_dependency("deployment"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_id_generation() {
        let id1 = WorkflowId::generate();
        let id2 = WorkflowId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_definition_builder() {
        let task = TaskDefinition::new("task1", PhaseId(0))
            .with_name("Test Task")
            .with_description("A test task")
            .with_dependency("task0")
            .with_timeout(60000)
            .with_retry(3);

        assert_eq!(task.id, "task1");
        assert_eq!(task.phase, PhaseId(0));
        assert_eq!(task.name, "Test Task");
        assert_eq!(task.dependencies.len(), 1);
        assert_eq!(task.timeout_ms, Some(60000));
        assert_eq!(task.retry_count, 3);
    }

    #[test]
    fn test_task_execution_lifecycle() {
        let mut exec = TaskExecution::new("task1");
        assert_eq!(exec.status, TaskStatus::Pending);

        exec.start();
        assert_eq!(exec.status, TaskStatus::Running);
        assert!(exec.started_at.is_some());

        exec.complete(vec![]);
        assert_eq!(exec.status, TaskStatus::Completed);
        assert!(exec.completed_at.is_some());
    }

    #[test]
    fn test_workflow_definition() {
        let workflow = WorkflowDefinition::new("Test Workflow")
            .with_task(TaskDefinition::new("task1", PhaseId(0)))
            .with_task(TaskDefinition::new("task2", PhaseId(1)));

        assert_eq!(workflow.task_count(), 2);
        assert!(workflow.get_task("task1").is_some());
        assert!(workflow.get_task("nonexistent").is_none());
    }

    #[test]
    fn test_dependency_graph_no_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("a", "b");
        graph.add_dependency("b", "c");
        graph.add_dependency("a", "c");

        assert!(!graph.has_cycle());
    }

    #[test]
    fn test_dependency_graph_with_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("a", "b");
        graph.add_dependency("b", "c");
        graph.add_dependency("c", "a");

        assert!(graph.has_cycle());
    }

    #[test]
    fn test_dependency_graph_topological_sort() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("a", "b");
        graph.add_dependency("b", "c");

        let sorted = graph.topological_sort().unwrap();

        assert!(
            sorted.iter().position(|x| x == "c").unwrap()
                < sorted.iter().position(|x| x == "b").unwrap()
        );
        assert!(
            sorted.iter().position(|x| x == "b").unwrap()
                < sorted.iter().position(|x| x == "a").unwrap()
        );
    }

    #[test]
    fn test_dependency_graph_ready_tasks() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("b", "a");
        graph.add_dependency("c", "a");

        let completed = HashSet::new();
        let ready = graph.get_ready_tasks(&completed);
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"a".to_string()));

        let completed: HashSet<String> = ["a".to_string()].into_iter().collect();
        let ready = graph.get_ready_tasks(&completed);
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn test_workflow_execution_progress() {
        let mut exec = WorkflowExecution::new(WorkflowId::new("test"));
        exec.initialize_tasks(&[
            TaskDefinition::new("task1", PhaseId(0)),
            TaskDefinition::new("task2", PhaseId(1)),
            TaskDefinition::new("task3", PhaseId(2)),
        ]);

        assert_eq!(exec.progress_percent(), 0.0);

        exec.task_executions
            .get_mut("task1")
            .unwrap()
            .complete(vec![]);
        assert!((exec.progress_percent() - 33.333).abs() < 0.1);

        exec.task_executions
            .get_mut("task2")
            .unwrap()
            .complete(vec![]);
        exec.task_executions
            .get_mut("task3")
            .unwrap()
            .complete(vec![]);
        assert_eq!(exec.progress_percent(), 100.0);
    }

    #[test]
    fn test_phase_workflow() {
        let workflow = PhaseWorkflow::new(PhaseId(0), PhaseId(2))
            .with_checkpoint(true)
            .with_required_gate("domain_identified");

        assert_eq!(workflow.start_phase, PhaseId(0));
        assert_eq!(workflow.end_phase, PhaseId(2));
        assert!(workflow.checkpoint_after);
        assert_eq!(workflow.required_gates.len(), 1);
        assert_eq!(workflow.phases().len(), 3);
    }

    #[tokio::test]
    async fn test_workflow_orchestrator_register() {
        let orchestrator = WorkflowOrchestrator::with_default_config();

        let workflow =
            WorkflowDefinition::new("Test").with_task(TaskDefinition::new("task1", PhaseId(0)));

        let id = orchestrator.register_workflow(workflow).await.unwrap();
        let retrieved = orchestrator.get_workflow(&id).await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test");
    }

    #[tokio::test]
    async fn test_workflow_orchestrator_circular_dependency() {
        let orchestrator = WorkflowOrchestrator::with_default_config();

        let workflow = WorkflowDefinition::new("Circular")
            .with_task(TaskDefinition::new("a", PhaseId(0)).with_dependency("b"))
            .with_task(TaskDefinition::new("b", PhaseId(1)).with_dependency("a"));

        let result = orchestrator.register_workflow(workflow).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_workflow_orchestrator_start_and_get_ready() {
        let orchestrator = WorkflowOrchestrator::with_default_config();

        let workflow = WorkflowDefinition::new("Test")
            .with_task(TaskDefinition::new("task1", PhaseId(0)))
            .with_task(TaskDefinition::new("task2", PhaseId(1)).with_dependency("task1"));

        let workflow_id = orchestrator.register_workflow(workflow).await.unwrap();
        let exec_id = orchestrator.start_workflow(&workflow_id).await.unwrap();

        let ready = orchestrator.get_ready_tasks(&exec_id).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"task1".to_string()));
    }

    #[tokio::test]
    async fn test_workflow_orchestrator_complete_task() {
        let orchestrator = WorkflowOrchestrator::with_default_config();

        let workflow = WorkflowDefinition::new("Test")
            .with_task(TaskDefinition::new("task1", PhaseId(0)))
            .with_task(TaskDefinition::new("task2", PhaseId(1)).with_dependency("task1"));

        let workflow_id = orchestrator.register_workflow(workflow).await.unwrap();
        let exec_id = orchestrator.start_workflow(&workflow_id).await.unwrap();

        orchestrator.start_task(&exec_id, "task1").await.unwrap();
        orchestrator
            .complete_task(&exec_id, "task1", vec![])
            .await
            .unwrap();

        let ready = orchestrator.get_ready_tasks(&exec_id).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"task2".to_string()));
    }

    #[test]
    fn test_create_standard_workflow() {
        let workflow = create_standard_workflow();
        assert!(!workflow.tasks.is_empty());
        assert_eq!(workflow.name, "Standard R&D Lifecycle");
    }

    #[tokio::test]
    async fn test_workflow_orchestrator_cancel() {
        let orchestrator = WorkflowOrchestrator::with_default_config();

        let workflow =
            WorkflowDefinition::new("Test").with_task(TaskDefinition::new("task1", PhaseId(0)));

        let workflow_id = orchestrator.register_workflow(workflow).await.unwrap();
        let exec_id = orchestrator.start_workflow(&workflow_id).await.unwrap();

        orchestrator.cancel_execution(&exec_id).await.unwrap();

        let exec = orchestrator.get_execution(&exec_id).await.unwrap();
        assert_eq!(exec.status, WorkflowStatus::Cancelled);
    }
}
