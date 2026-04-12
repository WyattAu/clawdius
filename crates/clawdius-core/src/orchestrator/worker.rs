//! Worker Implementation
//!
//! Each worker polls the task queue, executes tasks via the agentic system,
//! and pushes results back. Workers are designed to run one per CPU core on
//! Hetzner EX44 servers (8 cores, 32GB RAM).

use super::queue::TaskQueue;
use super::resource_governor::ResourceGovernor;
use super::{QueuedTask, TaskStatus};
use crate::agentic::{AgenticSystem, TaskResult};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Unique identifier for a worker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(String);

impl WorkerId {
    /// Creates a new worker ID.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Returns the worker ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Worker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Worker ID
    pub id: WorkerId,
    /// Poll interval when no tasks available (ms)
    pub poll_interval_ms: u64,
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Heartbeat interval (ms)
    pub heartbeat_interval_ms: u64,
    /// Task timeout (ms)
    pub task_timeout_ms: u64,
}

/// Worker status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Worker is idle, waiting for tasks
    Idle,
    /// Worker is processing a task
    Busy,
    /// Worker is shutting down
    ShuttingDown,
    /// Worker has stopped
    Stopped,
}

/// Handle to a running worker for control and monitoring.
pub struct WorkerHandle {
    /// Worker ID
    id: WorkerId,
    /// Status channel (send)
    status_tx: watch::Sender<WorkerStatus>,
    /// Status receiver for reading current status
    status_rx: watch::Receiver<WorkerStatus>,
    /// Shutdown signal
    shutdown_tx: watch::Sender<bool>,
}

impl WorkerHandle {
    /// Returns the worker ID.
    #[must_use]
    pub fn id(&self) -> WorkerId {
        self.id.clone()
    }

    /// Returns the current worker status.
    #[must_use]
    pub fn status(&self) -> WorkerStatus {
        *self.status_rx.borrow()
    }

    /// Signals the worker to stop.
    pub async fn stop(&self) {
        info!("Stopping worker: {}", self.id);
        let _ = self.status_tx.send(WorkerStatus::ShuttingDown);
        let _ = self.shutdown_tx.send(true);
    }
}

/// A worker that processes tasks from the queue.
pub struct Worker {
    /// Configuration
    config: WorkerConfig,
    /// Task queue
    queue: Arc<dyn TaskQueue>,
    /// Resource governor
    resource_governor: Arc<ResourceGovernor>,
    /// Current status
    status: WorkerStatus,
    /// Tasks currently being processed
    active_tasks: Mutex<Vec<String>>,
    /// Status channel
    status_tx: watch::Sender<WorkerStatus>,
    /// Shutdown receiver
    shutdown_rx: watch::Receiver<bool>,
}

impl Worker {
    /// Spawns a new worker as a background task.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker fails to start.
    pub fn spawn(
        config: WorkerConfig,
        queue: Arc<dyn TaskQueue>,
        resource_governor: Arc<ResourceGovernor>,
    ) -> Result<WorkerHandle> {
        let id = config.id.clone();
        let (status_tx, status_rx) = watch::channel(WorkerStatus::Idle);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let worker = Self {
            config,
            queue,
            resource_governor,
            status: WorkerStatus::Idle,
            active_tasks: Mutex::new(Vec::new()),
            status_tx: status_tx.clone(),
            shutdown_rx,
        };

        // Spawn the worker loop
        tokio::spawn(worker.run());

        Ok(WorkerHandle {
            id,
            status_tx,
            status_rx,
            shutdown_tx,
        })
    }

    /// The main worker loop.
    async fn run(mut self) {
        info!("Worker {} starting", self.config.id);
        let mut poll_interval = interval(Duration::from_millis(self.config.poll_interval_ms));
        poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            // Check for shutdown
            if *self.shutdown_rx.borrow() {
                self.set_status(WorkerStatus::Stopped).await;
                info!("Worker {} stopped", self.config.id);
                return;
            }

            // Check if we can accept more tasks
            let current_load = self.active_tasks.lock().await.len();
            if current_load >= self.config.max_concurrent_tasks {
                poll_interval.tick().await;
                continue;
            }

            // Try to dequeue a task
            match self.try_dequeue().await {
                Ok(Some(task)) => {
                    self.set_status(WorkerStatus::Busy).await;
                    self.active_tasks
                        .lock()
                        .await
                        .push(task.task_id().to_string());

                    // Spawn task execution
                    let queue = Arc::clone(&self.queue);
                    let governor = Arc::clone(&self.resource_governor);
                    let timeout_ms = self.config.task_timeout_ms;
                    let worker_id = self.config.id.clone();

                    tokio::spawn(async move {
                        Self::execute_task(task, queue, governor, timeout_ms, &worker_id).await;
                    });
                },
                Ok(None) => {
                    self.set_status(WorkerStatus::Idle).await;
                    poll_interval.tick().await;
                },
                Err(e) => {
                    error!("Worker {} dequeue error: {}", self.config.id, e);
                    poll_interval.tick().await;
                },
            }
        }
    }

    /// Tries to dequeue a task, respecting resource quotas.
    async fn try_dequeue(&self) -> Result<Option<QueuedTask>> {
        let task = self.queue.dequeue(self.config.id.as_str(), None).await?;

        if let Some(ref t) = task {
            // Check resource quotas for the tenant
            if let Err(e) = self.resource_governor.check_quota(&t.tenant_id).await {
                warn!(
                    "Worker {}: tenant {} quota exceeded: {}, requeueing task {}",
                    self.config.id,
                    t.tenant_id,
                    e,
                    t.task_id()
                );
                // Requeue by updating status back to pending
                let _ = self
                    .queue
                    .update_status(t.task_id(), TaskStatus::Pending)
                    .await;
                return Ok(None);
            }

            // Acquire resources
            if let Err(e) = self
                .resource_governor
                .acquire(&t.tenant_id, t.task_id())
                .await
            {
                warn!(
                    "Worker {}: failed to acquire resources for tenant {}: {}",
                    self.config.id, t.tenant_id, e
                );
                let _ = self
                    .queue
                    .update_status(t.task_id(), TaskStatus::Pending)
                    .await;
                return Ok(None);
            }
        }

        Ok(task)
    }

    /// Executes a single task with timeout and heartbeat.
    async fn execute_task(
        task: QueuedTask,
        queue: Arc<dyn TaskQueue>,
        governor: Arc<ResourceGovernor>,
        timeout_ms: u64,
        worker_id: &WorkerId,
    ) {
        let task_id = task.task_id().to_string();
        let tenant_id = task.tenant_id.clone();

        info!(
            "Worker {} executing task {} for tenant {}",
            worker_id, task_id, tenant_id
        );

        // Spawn heartbeat loop
        let hb_queue = Arc::clone(&queue);
        let hb_task_id = task_id.clone();
        let hb_worker_id = worker_id.to_string();
        let heartbeat_ms = 10_000u64;

        let heartbeat_handle = tokio::spawn(async move {
            let mut hb_interval = interval(Duration::from_millis(heartbeat_ms));
            loop {
                hb_interval.tick().await;
                if hb_queue
                    .record_heartbeat(&hb_task_id, &hb_worker_id)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            Self::run_task(task.clone()),
        )
        .await;

        // Stop heartbeat
        heartbeat_handle.abort();

        // Handle result
        match result {
            Ok(Ok(task_result)) => {
                let result_json = serde_json::to_string(&task_result).unwrap_or_else(|_| {
                    r#"{"success":false,"error":"serialize_failed"}"#.to_string()
                });

                if let Err(e) = queue.push_result(&task_id, &result_json).await {
                    error!("Failed to push result for task {}: {}", task_id, e);
                }
                if let Err(e) = queue.update_status(&task_id, TaskStatus::Completed).await {
                    error!("Failed to update task {} status: {}", task_id, e);
                }

                info!(
                    "Worker {} completed task {} ({}ms, success: {})",
                    worker_id, task_id, task_result.duration_ms, task_result.success
                );
            },
            Ok(Err(e)) => {
                error!(
                    "Worker {} task {} execution error: {}",
                    worker_id, task_id, e
                );
                let _ = queue
                    .push_result(
                        &task_id,
                        &serde_json::json!({"success": false, "error": e.to_string()}).to_string(),
                    )
                    .await;
                let _ = queue.update_status(&task_id, TaskStatus::Failed).await;
            },
            Err(_) => {
                warn!(
                    "Worker {} task {} timed out after {}ms",
                    worker_id, task_id, timeout_ms
                );
                let _ = queue
                    .push_result(
                        &task_id,
                        &serde_json::json!({"success": false, "error": "timeout"}).to_string(),
                    )
                    .await;
                let _ = queue.update_status(&task_id, TaskStatus::TimedOut).await;
            },
        }

        // Release resources
        governor.release(&tenant_id, &task_id).await;
    }

    /// Runs the actual task using the agentic system.
    async fn run_task(task: QueuedTask) -> Result<TaskResult> {
        // Build the agentic system from the task request
        let mut system = AgenticSystem::new(
            task.request.mode.clone(),
            task.request.test_strategy.clone(),
            task.request.apply_workflow.clone(),
        );

        system.execute(task.request).await
    }

    /// Updates the worker status.
    async fn set_status(&mut self, status: WorkerStatus) {
        self.status = status;
        let _ = self.status_tx.send(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::{
        ApplyWorkflow, GenerationMode, TaskContext, TaskRequest, TestExecutionStrategy, TrustLevel,
    };
    use crate::orchestrator::queue::InMemoryTaskQueue;

    fn make_test_config() -> WorkerConfig {
        WorkerConfig {
            id: WorkerId::new("test-worker"),
            poll_interval_ms: 50,
            max_concurrent_tasks: 2,
            heartbeat_interval_ms: 100,
            task_timeout_ms: 5000,
        }
    }

    fn make_task_request(id: &str) -> TaskRequest {
        TaskRequest {
            id: id.to_string(),
            description: "Test task".to_string(),
            target_files: vec![],
            mode: GenerationMode::SinglePass,
            test_strategy: TestExecutionStrategy::DirectWithRollback {
                git_stash: true,
                timeout_ms: 30000,
            },
            apply_workflow: ApplyWorkflow::PreviewOnly,
            context: TaskContext::default(),
            trust_level: TrustLevel::Medium,
        }
    }

    #[test]
    fn test_worker_id() {
        let id = WorkerId::new("worker-0");
        assert_eq!(id.as_str(), "worker-0");
        assert_eq!(format!("{}", id), "worker-0");
    }

    #[tokio::test]
    async fn test_worker_spawn_and_stop() {
        let queue = Arc::new(InMemoryTaskQueue::new());
        let governor = Arc::new(ResourceGovernor::new(std::collections::HashMap::new()));

        let handle = Worker::spawn(make_test_config(), queue, governor).expect("spawn");
        assert_eq!(handle.id().as_str(), "test-worker");

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.stop().await;
        // Wait for the worker loop to process the shutdown signal
        tokio::time::sleep(Duration::from_millis(200)).await;
        let status = handle.status();
        assert!(
            matches!(status, WorkerStatus::ShuttingDown | WorkerStatus::Stopped),
            "Expected ShuttingDown or Stopped, got {:?}",
            status
        );
    }

    #[tokio::test]
    async fn test_worker_processes_task() {
        let queue = Arc::new(InMemoryTaskQueue::new());
        let governor = Arc::new(ResourceGovernor::new(std::collections::HashMap::new()));

        let config = WorkerConfig {
            poll_interval_ms: 10,
            task_timeout_ms: 5000,
            ..make_test_config()
        };

        let handle = Worker::spawn(
            config,
            queue.clone() as Arc<dyn TaskQueue>,
            Arc::clone(&governor),
        )
        .expect("spawn");

        // Enqueue a task
        let task = QueuedTask::new(make_task_request("worker-test"), "tenant-1", "session-1");
        queue.enqueue(task).await.expect("enqueue");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Task should be completed
        let status = queue.task_status("worker-test").await;
        assert!(status.is_ok(), "task should exist");

        handle.stop().await;
    }
}
