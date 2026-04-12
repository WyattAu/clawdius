//! Orchestrator Mode — Worker Architecture for Multi-Node Task Distribution
//!
//! This module provides a queue-based task distribution system that wraps the
//! existing [`AgenticSystem`](super::AgenticSystem) for horizontal scaling.
//!
//! # Architecture
//!
//! ```text
//!   Next.js API ──→ Redis (pending_tasks) ──→ Worker 1 (Hetzner EX44)
//!                                    │       Worker 2 (Hetzner EX44)
//!                                    │       Worker N (Hetzner EX44)
//!                                    │
//!                                    └──→ Redis (completed_tasks)
//!                                            │
//!                                            └──→ S3 (results) + DB (timeline)
//! ```
//!
//! # Task Lifecycle
//!
//! 1. **Ingress**: Worker polls `pending_tasks` queue
//! 2. **Validation**: Check user credits, parse TaskRequest
//! 3. **Locking**: Atomically claim task (Redis SETNX)
//! 4. **Execution**: Run via [`AgenticSystem::execute()`](super::AgenticSystem::execute)
//! 5. **Egress**: Push result to `completed_tasks`, update heartbeat
//!
//! # Feature Flags
//!
//! - `orchestrator` (default: off) — enables the orchestrator module
//! - `redis-queue` — enables Redis-backed queue implementation

use crate::agentic::TaskRequest;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub mod queue;
pub mod resource_governor;
pub mod worker;

// Re-exports
#[cfg(feature = "redis-queue")]
pub use queue::RedisTaskQueue;
pub use queue::{InMemoryTaskQueue, TaskQueue};
pub use resource_governor::{Quota, ResourceGovernor, ResourceUsage};
pub use worker::{Worker, WorkerConfig, WorkerHandle, WorkerId, WorkerStatus};

/// Configuration for the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Queue backend to use
    pub queue_backend: QueueBackend,
    /// Number of worker threads
    pub worker_count: usize,
    /// Poll interval when no tasks are available (ms)
    pub poll_interval_ms: u64,
    /// Maximum concurrent tasks per worker
    pub max_concurrent_tasks: usize,
    /// Heartbeat interval (ms)
    pub heartbeat_interval_ms: u64,
    /// Task timeout (ms)
    pub task_timeout_ms: u64,
    /// Worker ID prefix
    pub worker_id_prefix: String,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            queue_backend: QueueBackend::InMemory,
            worker_count: 1,
            poll_interval_ms: 1000,
            max_concurrent_tasks: 1,
            heartbeat_interval_ms: 10_000,
            task_timeout_ms: 600_000, // 10 minutes
            worker_id_prefix: "worker".to_string(),
        }
    }
}

/// Queue backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueBackend {
    /// In-memory queue (single node, dev/testing)
    InMemory,
    /// Redis-backed queue (multi-node, production)
    Redis,
}

/// Status of a task in the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is waiting in the queue
    Pending,
    /// Task has been claimed by a worker
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task timed out
    TimedOut,
}

/// A queued task with metadata for orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    /// The actual task request
    pub request: TaskRequest,
    /// Current status
    pub status: TaskStatus,
    /// Worker ID that claimed this task (if any)
    pub claimed_by: Option<String>,
    /// Tenant/user ID for multi-tenancy
    pub tenant_id: String,
    /// Session ID for tracing
    pub session_id: String,
    /// Proxy URL for this task (if configured)
    pub proxy_url: Option<String>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// When the task was enqueued (unix millis)
    pub enqueued_at: u64,
    /// When the task was claimed (unix millis)
    pub claimed_at: Option<u64>,
    /// Last heartbeat timestamp (unix millis)
    pub last_heartbeat: Option<u64>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl QueuedTask {
    /// Creates a new queued task.
    #[must_use]
    pub fn new(
        request: TaskRequest,
        tenant_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        let now = current_timestamp();
        Self {
            request,
            status: TaskStatus::Pending,
            claimed_by: None,
            tenant_id: tenant_id.into(),
            session_id: session_id.into(),
            proxy_url: None,
            priority: 0,
            enqueued_at: now,
            claimed_at: None,
            last_heartbeat: None,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Sets the proxy URL for this task.
    #[must_use]
    pub fn with_proxy(mut self, url: impl Into<String>) -> Self {
        self.proxy_url = Some(url.into());
        self
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the max retries.
    #[must_use]
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Returns the task ID.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.request.id
    }

    /// Returns true if the task can be retried.
    #[must_use]
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Returns the time spent in queue before being claimed (ms).
    #[must_use]
    pub fn queue_wait_time(&self) -> Option<u64> {
        self.claimed_at
            .map(|claimed| claimed.saturating_sub(self.enqueued_at))
    }
}

/// The main orchestrator managing workers and task distribution.
pub struct Orchestrator {
    /// Configuration
    config: OrchestratorConfig,
    /// Task queue
    queue: Arc<dyn TaskQueue>,
    /// Resource governor for multi-tenant quotas
    resource_governor: Arc<ResourceGovernor>,
    /// Active worker handles
    workers: Arc<RwLock<Vec<WorkerHandle>>>,
    /// Running flag
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl Orchestrator {
    /// Creates a new orchestrator with the given config and queue.
    #[must_use]
    pub fn new(
        config: OrchestratorConfig,
        queue: Arc<dyn TaskQueue>,
        resource_governor: Arc<ResourceGovernor>,
    ) -> Self {
        Self {
            config,
            queue,
            resource_governor,
            workers: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Creates an orchestrator with an in-memory queue.
    #[must_use]
    pub fn with_in_memory_queue(config: OrchestratorConfig) -> Self {
        let queue = Arc::new(InMemoryTaskQueue::new());
        let governor = Arc::new(ResourceGovernor::new(HashMap::new()));
        Self::new(config, queue, governor)
    }

    /// Starts the orchestrator and all workers.
    ///
    /// # Errors
    ///
    /// Returns an error if workers fail to start.
    pub async fn start(&self) -> Result<()> {
        if self
            .running
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            warn!("Orchestrator is already running");
            return Ok(());
        }

        info!(
            "Starting orchestrator with {} workers (backend: {:?})",
            self.config.worker_count, self.config.queue_backend
        );

        let mut handles = self.workers.write().await;
        handles.clear();

        for i in 0..self.config.worker_count {
            let worker_id = format!("{}-{}", self.config.worker_id_prefix, i);
            let worker_config = WorkerConfig {
                id: WorkerId::new(&worker_id),
                poll_interval_ms: self.config.poll_interval_ms,
                max_concurrent_tasks: self.config.max_concurrent_tasks,
                heartbeat_interval_ms: self.config.heartbeat_interval_ms,
                task_timeout_ms: self.config.task_timeout_ms,
            };

            let handle = Worker::spawn(
                worker_config,
                Arc::clone(&self.queue),
                Arc::clone(&self.resource_governor),
            )?;

            info!("Started worker: {}", worker_id);
            handles.push(handle);
        }

        Ok(())
    }

    /// Stops the orchestrator gracefully.
    pub async fn stop(&self) {
        if !self
            .running
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        info!("Stopping orchestrator...");

        let mut handles = self.workers.write().await;
        for handle in handles.drain(..) {
            handle.stop().await;
        }

        info!("Orchestrator stopped");
    }

    /// Enqueues a new task.
    ///
    /// # Errors
    ///
    /// Returns an error if the queue rejects the task.
    pub async fn enqueue(&self, task: QueuedTask) -> Result<String> {
        let task_id = task.task_id().to_string();
        info!(
            "Enqueuing task {} for tenant {} (session: {})",
            task_id, task.tenant_id, task.session_id
        );
        self.queue.enqueue(task).await
    }

    /// Gets the status of a task.
    ///
    /// # Errors
    ///
    /// Returns an error if the task is not found.
    pub async fn task_status(&self, task_id: &str) -> Result<TaskStatus> {
        self.queue.task_status(task_id).await
    }

    /// Cancels a pending or running task.
    ///
    /// # Errors
    ///
    /// Returns an error if the task cannot be cancelled.
    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        info!("Cancelling task: {}", task_id);
        self.queue.cancel_task(task_id).await
    }

    /// Returns the number of pending tasks.
    pub async fn pending_count(&self) -> usize {
        self.queue.pending_count().await
    }

    /// Returns worker statuses.
    pub async fn worker_statuses(&self) -> Vec<(WorkerId, WorkerStatus)> {
        let handles = self.workers.read().await;
        handles.iter().map(|h| (h.id(), h.status())).collect()
    }

    /// Returns the resource governor for quota management.
    #[must_use]
    pub fn resource_governor(&self) -> &Arc<ResourceGovernor> {
        &self.resource_governor
    }

    /// Returns true if the orchestrator is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Returns the current timestamp in milliseconds.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queued_task_creation() {
        let request = TaskRequest {
            id: "task-1".to_string(),
            description: "Test task".to_string(),
            target_files: vec![],
            mode: crate::agentic::GenerationMode::SinglePass,
            test_strategy: crate::agentic::TestExecutionStrategy::DirectWithRollback {
                git_stash: true,
                timeout_ms: 30000,
            },
            apply_workflow: crate::agentic::ApplyWorkflow::PreviewOnly,
            context: crate::agentic::TaskContext::default(),
            trust_level: crate::agentic::TrustLevel::Medium,
        };

        let task = QueuedTask::new(request, "tenant-123", "session-456")
            .with_proxy("http://proxy:8080")
            .with_priority(1)
            .with_max_retries(5);

        assert_eq!(task.task_id(), "task-1");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.tenant_id, "tenant-123");
        assert_eq!(task.session_id, "session-456");
        assert_eq!(task.proxy_url.as_deref(), Some("http://proxy:8080"));
        assert_eq!(task.priority, 1);
        assert_eq!(task.max_retries, 5);
        assert!(task.can_retry());
        assert!(task.claimed_by.is_none());
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::Running;
        let json = serde_json::to_string(&status).expect("serialize status");
        let parsed: TaskStatus = serde_json::from_str(&json).expect("deserialize status");
        assert_eq!(parsed, TaskStatus::Running);
    }

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.queue_backend, QueueBackend::InMemory);
        assert_eq!(config.worker_count, 1);
        assert_eq!(config.heartbeat_interval_ms, 10_000);
        assert_eq!(config.task_timeout_ms, 600_000);
    }

    #[tokio::test]
    async fn test_orchestrator_start_stop() {
        let config = OrchestratorConfig {
            worker_count: 2,
            ..Default::default()
        };

        let orch = Orchestrator::with_in_memory_queue(config);
        assert!(!orch.is_running());

        orch.start().await.expect("start orchestrator");
        assert!(orch.is_running());

        let statuses = orch.worker_statuses().await;
        assert_eq!(statuses.len(), 2);

        orch.stop().await;
        assert!(!orch.is_running());
    }

    #[tokio::test]
    async fn test_orchestrator_enqueue() {
        let config = OrchestratorConfig::default();
        let orch = Orchestrator::with_in_memory_queue(config);
        orch.start().await.expect("start");

        let request = TaskRequest {
            id: "enqueue-test".to_string(),
            description: "Test".to_string(),
            target_files: vec![],
            mode: crate::agentic::GenerationMode::SinglePass,
            test_strategy: crate::agentic::TestExecutionStrategy::DirectWithRollback {
                git_stash: true,
                timeout_ms: 30000,
            },
            apply_workflow: crate::agentic::ApplyWorkflow::PreviewOnly,
            context: crate::agentic::TaskContext::default(),
            trust_level: crate::agentic::TrustLevel::Medium,
        };

        let task = QueuedTask::new(request, "t1", "s1");
        let task_id = orch.enqueue(task).await.expect("enqueue");

        assert_eq!(task_id, "enqueue-test");
        assert_eq!(orch.pending_count().await, 1);

        let status = orch.task_status("enqueue-test").await.expect("status");
        assert_eq!(status, TaskStatus::Pending);

        orch.stop().await;
    }
}
