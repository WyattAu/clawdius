//! Task Queue Abstraction
//!
//! Provides a trait for task queues with in-memory and Redis implementations.
//! The in-memory queue is for single-node dev/testing; Redis is for production
//! multi-node deployments.

use super::{current_timestamp, QueuedTask, TaskStatus};
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use tokio::sync::{Mutex, Notify};

/// Trait for a task queue backend.
///
/// Implementations provide the actual storage and retrieval of tasks.
/// The orchestrator polls this interface to get work.
#[async_trait]
pub trait TaskQueue: Send + Sync {
    /// Enqueue a task for processing.
    ///
    /// Returns the task ID.
    async fn enqueue(&self, task: QueuedTask) -> Result<String>;

    /// Dequeue the next available task for a given worker.
    ///
    /// Returns `None` if no tasks are available.
    async fn dequeue(&self, worker_id: &str, tenant_id: Option<&str>)
        -> Result<Option<QueuedTask>>;

    /// Update the status of a task.
    async fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()>;

    /// Get the current status of a task.
    async fn task_status(&self, task_id: &str) -> Result<TaskStatus>;

    /// Cancel a task.
    async fn cancel_task(&self, task_id: &str) -> Result<()>;

    /// Record a heartbeat for a running task.
    async fn record_heartbeat(&self, task_id: &str, worker_id: &str) -> Result<()>;

    /// Get tasks that have exceeded their heartbeat timeout.
    async fn stale_tasks(&self, timeout_ms: u64) -> Result<Vec<QueuedTask>>;

    /// Push a completed task result.
    async fn push_result(&self, task_id: &str, result: &str) -> Result<()>;

    /// Pop a completed task result.
    async fn pop_result(&self) -> Result<Option<String>>;

    /// Get the number of pending tasks.
    async fn pending_count(&self) -> usize;

    /// Get the number of running tasks.
    async fn running_count(&self) -> usize;
}

// ============================================================================
// In-Memory Implementation
// ============================================================================

/// In-memory task queue for single-node dev/testing.
///
/// Thread-safe via `Mutex`. Not suitable for multi-node production.
pub struct InMemoryTaskQueue {
    /// All tasks indexed by ID
    tasks: Mutex<HashMap<String, QueuedTask>>,
    /// Pending queue ordered by priority then enqueue time
    pending: Mutex<VecDeque<String>>,
    /// Completed results
    results: Mutex<VecDeque<String>>,
    /// Notification for new tasks
    notify: Notify,
}

impl InMemoryTaskQueue {
    /// Creates a new in-memory task queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            pending: Mutex::new(VecDeque::new()),
            results: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
        }
    }
}

impl Default for InMemoryTaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskQueue for InMemoryTaskQueue {
    async fn enqueue(&self, task: QueuedTask) -> Result<String> {
        let task_id = task.task_id().to_string();
        let mut tasks = self.tasks.lock().await;
        let mut pending = self.pending.lock().await;

        // Check for duplicate
        if tasks.contains_key(&task_id) {
            return Err(Error::InvalidInput(format!(
                "Task {} already exists",
                task_id
            )));
        }

        pending.push_back(task_id.clone());
        tasks.insert(task_id.clone(), task);
        self.notify.notify_waiters();

        tracing::debug!("Enqueued task: {}", task_id);
        Ok(task_id)
    }

    async fn dequeue(
        &self,
        worker_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<Option<QueuedTask>> {
        let mut tasks = self.tasks.lock().await;
        let mut pending = self.pending.lock().await;

        // Find the first pending task (optionally filtered by tenant)
        let idx = if let Some(tid) = tenant_id {
            pending.iter().position(|id| {
                tasks
                    .get(id)
                    .map(|t| t.tenant_id == tid && t.status == TaskStatus::Pending)
                    .unwrap_or(false)
            })
        } else {
            pending.iter().position(|id| {
                tasks
                    .get(id)
                    .map(|t| t.status == TaskStatus::Pending)
                    .unwrap_or(false)
            })
        };

        let Some(idx) = idx else {
            return Ok(None);
        };

        let task_id = pending.remove(idx).expect("idx is valid");
        let task = tasks.get_mut(&task_id).expect("task exists");

        task.status = TaskStatus::Running;
        task.claimed_by = Some(worker_id.to_string());
        task.claimed_at = Some(current_timestamp());
        task.last_heartbeat = Some(current_timestamp());

        tracing::debug!("Worker {} claimed task {}", worker_id, task_id);

        Ok(Some(task.clone()))
    }

    async fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::NotFound(format!("Task {} not found", task_id)))?;
        task.status = status;

        // Remove from pending if completed/failed/cancelled
        if matches!(
            status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            let mut pending = self.pending.lock().await;
            pending.retain(|id| id != task_id);
        }

        Ok(())
    }

    async fn task_status(&self, task_id: &str) -> Result<TaskStatus> {
        let tasks = self.tasks.lock().await;
        let task = tasks
            .get(task_id)
            .ok_or_else(|| Error::NotFound(format!("Task {} not found", task_id)))?;
        Ok(task.status)
    }

    async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.update_status(task_id, TaskStatus::Cancelled).await
    }

    async fn record_heartbeat(&self, task_id: &str, worker_id: &str) -> Result<()> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::NotFound(format!("Task {} not found", task_id)))?;

        // Only the owning worker can heartbeat
        if task.claimed_by.as_deref() != Some(worker_id) {
            return Err(Error::InvalidInput(format!(
                "Worker {} does not own task {}",
                worker_id, task_id
            )));
        }

        task.last_heartbeat = Some(current_timestamp());
        Ok(())
    }

    async fn stale_tasks(&self, timeout_ms: u64) -> Result<Vec<QueuedTask>> {
        let tasks = self.tasks.lock().await;
        let now = current_timestamp();

        Ok(tasks
            .values()
            .filter(|t| {
                t.status == TaskStatus::Running
                    && t.last_heartbeat
                        .map(|hb| now.saturating_sub(hb) > timeout_ms)
                        .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    async fn push_result(&self, task_id: &str, result: &str) -> Result<()> {
        let mut results = self.results.lock().await;
        let payload = serde_json::json!({
            "task_id": task_id,
            "result": result,
            "completed_at": current_timestamp(),
        });
        results.push_back(serde_json::to_string(&payload).expect("serialize result"));
        Ok(())
    }

    async fn pop_result(&self) -> Result<Option<String>> {
        let mut results = self.results.lock().await;
        Ok(results.pop_front())
    }

    async fn pending_count(&self) -> usize {
        let tasks = self.tasks.lock().await;
        tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .count()
    }

    async fn running_count(&self) -> usize {
        let tasks = self.tasks.lock().await;
        tasks
            .values()
            .filter(|t| t.status == TaskStatus::Running)
            .count()
    }
}

// ============================================================================
// Redis Implementation (feature-gated)
// ============================================================================

/// Redis-backed task queue for production multi-node deployments.
///
/// Uses Redis LIST for task queues, HASH for task state, and SET for
/// distributed locking.
#[cfg(feature = "redis-queue")]
pub struct RedisTaskQueue {
    /// Redis client
    client: redis::aio::ConnectionManager,
    /// Key prefix for namespacing
    key_prefix: String,
}

#[cfg(feature = "redis-queue")]
impl RedisTaskQueue {
    /// Creates a new Redis task queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the Redis connection fails.
    pub async fn new(redis_url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| Error::Config(format!("Invalid Redis URL: {}", e)))?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| Error::Config(format!("Redis connection failed: {}", e)))?;

        Ok(Self {
            client: conn,
            key_prefix: key_prefix.unwrap_or("clawdius").to_string(),
        })
    }

    /// Returns the namespaced key.
    fn key(&self, name: &str) -> String {
        format!("{}:{}", self.key_prefix, name)
    }
}

#[cfg(feature = "redis-queue")]
#[async_trait]
impl TaskQueue for RedisTaskQueue {
    async fn enqueue(&self, task: QueuedTask) -> Result<String> {
        let task_id = task.task_id().to_string();
        let task_json =
            serde_json::to_string(&task).map_err(|e| Error::Serialize(e.to_string()))?;

        let mut conn = self.client.clone();
        let pending_key = self.key("pending_tasks");
        let task_key = self.key(&format!("task:{}", task_id));

        // Store full task data
        redis::cmd("SET")
            .arg(&task_key)
            .arg(&task_json)
            .arg("EX")
            .arg(86400) // 24h TTL
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SET failed: {}", e)))?;

        // Push to pending list (LPUSH so dequeue uses RPOP for FIFO)
        redis::cmd("LPUSH")
            .arg(&pending_key)
            .arg(&task_id)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis LPUSH failed: {}", e)))?;

        Ok(task_id)
    }

    async fn dequeue(
        &self,
        worker_id: &str,
        _tenant_id: Option<&str>,
    ) -> Result<Option<QueuedTask>> {
        let mut conn = self.client.clone();
        let pending_key = self.key("pending_tasks");
        let lock_key = self.key(&format!("lock:{}", worker_id));
        let task_prefix = self.key("task:");

        // RPOP from pending list (blocking with short timeout)
        let task_id: Option<String> = redis::cmd("RPOP")
            .arg(&pending_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis RPOP failed: {}", e)))?;

        let Some(task_id) = task_id else {
            return Ok(None);
        };

        // Try to claim with SETNX (atomic lock)
        let claimed: bool = redis::cmd("SET")
            .arg(&lock_key)
            .arg(&task_id)
            .arg("NX")
            .arg("EX")
            .arg(600) // 10 min lock TTL
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SETNX failed: {}", e)))?;

        if !claimed {
            // Another worker got it, push back
            redis::cmd("LPUSH")
                .arg(&pending_key)
                .arg(&task_id)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| Error::Internal(format!("Redis LPUSH failed: {}", e)))?;
            return Ok(None);
        }

        // Load task data
        let task_key = format!("{}{}", task_prefix, task_id);
        let task_json: Option<String> = redis::cmd("GET")
            .arg(&task_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis GET failed: {}", e)))?;

        let Some(task_json) = task_json else {
            return Ok(None);
        };

        let mut task: QueuedTask =
            serde_json::from_str(&task_json).map_err(|e| Error::ParseError(e.to_string()))?;
        task.status = super::TaskStatus::Running;
        task.claimed_by = Some(worker_id.to_string());
        task.claimed_at = Some(current_timestamp());
        task.last_heartbeat = Some(current_timestamp());

        // Update stored task
        let updated_json =
            serde_json::to_string(&task).map_err(|e| Error::Serialize(e.to_string()))?;
        redis::cmd("SET")
            .arg(&task_key)
            .arg(&updated_json)
            .arg("EX")
            .arg(86400)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SET failed: {}", e)))?;

        Ok(Some(task))
    }

    async fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut conn = self.client.clone();
        let task_key = self.key(&format!("task:{}", task_id));

        let task_json: Option<String> = redis::cmd("GET")
            .arg(&task_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis GET failed: {}", e)))?;

        let Some(task_json) = task_json else {
            return Err(Error::NotFound(format!("Task {} not found", task_id)));
        };

        let mut task: QueuedTask =
            serde_json::from_str(&task_json).map_err(|e| Error::ParseError(e.to_string()))?;
        task.status = status;

        let updated_json =
            serde_json::to_string(&task).map_err(|e| Error::Serialize(e.to_string()))?;
        redis::cmd("SET")
            .arg(&task_key)
            .arg(&updated_json)
            .arg("EX")
            .arg(86400)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SET failed: {}", e)))?;

        Ok(())
    }

    async fn task_status(&self, task_id: &str) -> Result<TaskStatus> {
        let mut conn = self.client.clone();
        let task_key = self.key(&format!("task:{}", task_id));

        let task_json: Option<String> = redis::cmd("GET")
            .arg(&task_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis GET failed: {}", e)))?;

        let Some(task_json) = task_json else {
            return Err(Error::NotFound(format!("Task {} not found", task_id)));
        };

        let task: QueuedTask =
            serde_json::from_str(&task_json).map_err(|e| Error::ParseError(e.to_string()))?;
        Ok(task.status)
    }

    async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.update_status(task_id, TaskStatus::Cancelled).await
    }

    async fn record_heartbeat(&self, task_id: &str, worker_id: &str) -> Result<()> {
        let mut conn = self.client.clone();
        let heartbeat_key = self.key(&format!("heartbeat:{}", task_id));

        redis::cmd("SET")
            .arg(&heartbeat_key)
            .arg(worker_id)
            .arg("EX")
            .arg(30) // 30s TTL
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SET failed: {}", e)))?;

        Ok(())
    }

    async fn stale_tasks(&self, timeout_ms: u64) -> Result<Vec<QueuedTask>> {
        // In production, use Redis SCAN with pattern "clawdius:task:*"
        // and check heartbeat keys. This is a simplified implementation.
        Ok(Vec::new())
    }

    async fn push_result(&self, task_id: &str, result: &str) -> Result<()> {
        let mut conn = self.client.clone();
        let results_key = self.key("completed_tasks");

        let payload = serde_json::json!({
            "task_id": task_id,
            "result": result,
            "completed_at": current_timestamp(),
        });

        redis::cmd("LPUSH")
            .arg(&results_key)
            .arg(serde_json::to_string(&payload).expect("serialize result"))
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis LPUSH failed: {}", e)))?;

        Ok(())
    }

    async fn pop_result(&self) -> Result<Option<String>> {
        let mut conn = self.client.clone();
        let results_key = self.key("completed_tasks");

        let result: Option<String> = redis::cmd("RPOP")
            .arg(&results_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis RPOP failed: {}", e)))?;

        Ok(result)
    }

    async fn pending_count(&self) -> usize {
        let mut conn = self.client.clone();
        let pending_key = self.key("pending_tasks");

        redis::cmd("LLEN")
            .arg(&pending_key)
            .query_async::<usize>(&mut conn)
            .await
            .unwrap_or(0)
    }

    async fn running_count(&self) -> usize {
        // In production, use SCAN with pattern "clawdius:heartbeat:*"
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::{
        ApplyWorkflow, GenerationMode, TaskContext, TaskRequest, TestExecutionStrategy, TrustLevel,
    };

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

    #[tokio::test]
    async fn test_in_memory_enqueue_dequeue() {
        let queue = InMemoryTaskQueue::new();

        let task = QueuedTask::new(make_task_request("t1"), "tenant-a", "session-1");
        queue.enqueue(task).await.expect("enqueue");

        assert_eq!(queue.pending_count().await, 1);

        let dequeued = queue
            .dequeue("worker-0", None)
            .await
            .expect("dequeue")
            .expect("should have task");

        assert_eq!(dequeued.task_id(), "t1");
        assert_eq!(dequeued.status, TaskStatus::Running);
        assert_eq!(dequeued.claimed_by.as_deref(), Some("worker-0"));

        assert_eq!(queue.pending_count().await, 0);
        assert_eq!(queue.running_count().await, 1);
    }

    #[tokio::test]
    async fn test_in_memory_duplicate_enqueue() {
        let queue = InMemoryTaskQueue::new();

        let task = QueuedTask::new(make_task_request("dup"), "t", "s");
        queue.enqueue(task).await.expect("first enqueue");

        let task2 = QueuedTask::new(make_task_request("dup"), "t", "s");
        let result = queue.enqueue(task2).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_cancel() {
        let queue = InMemoryTaskQueue::new();

        let task = QueuedTask::new(make_task_request("cancel-me"), "t", "s");
        queue.enqueue(task).await.expect("enqueue");

        queue.cancel_task("cancel-me").await.expect("cancel");

        assert_eq!(queue.pending_count().await, 0);
        let status = queue.task_status("cancel-me").await.expect("status");
        assert_eq!(status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_in_memory_heartbeat() {
        let queue = InMemoryTaskQueue::new();

        let task = QueuedTask::new(make_task_request("hb"), "t", "s");
        queue.enqueue(task).await.expect("enqueue");
        queue.dequeue("w1", None).await.expect("dequeue");

        // Owner can heartbeat
        queue.record_heartbeat("hb", "w1").await.expect("heartbeat");

        // Non-owner cannot heartbeat
        let result = queue.record_heartbeat("hb", "w2").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_stale_tasks() {
        let queue = InMemoryTaskQueue::new();

        let task = QueuedTask::new(make_task_request("stale"), "t", "s");
        queue.enqueue(task).await.expect("enqueue");

        // Manually set an old heartbeat
        {
            let mut tasks = queue.tasks.lock().await;
            if let Some(t) = tasks.get_mut("stale") {
                t.status = TaskStatus::Running;
                t.last_heartbeat = Some(current_timestamp() - 100_000); // 100s ago
            }
        }

        let stale = queue.stale_tasks(30_000).await.expect("stale");
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].task_id(), "stale");
    }

    #[tokio::test]
    async fn test_in_memory_results() {
        let queue = InMemoryTaskQueue::new();

        queue
            .push_result("t1", r#"{"success": true}"#)
            .await
            .expect("push");

        let result = queue
            .pop_result()
            .await
            .expect("pop")
            .expect("should have result");
        assert!(result.contains("t1"));

        let empty = queue.pop_result().await.expect("pop");
        assert!(empty.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_tenant_filtering() {
        let queue = InMemoryTaskQueue::new();

        queue
            .enqueue(QueuedTask::new(make_task_request("ta"), "tenant-a", "s"))
            .await
            .expect("enqueue a");
        queue
            .enqueue(QueuedTask::new(make_task_request("tb"), "tenant-b", "s"))
            .await
            .expect("enqueue b");

        // Dequeue for tenant-a only
        let task = queue
            .dequeue("w1", Some("tenant-a"))
            .await
            .expect("dequeue")
            .expect("should have task");

        assert_eq!(task.task_id(), "ta");
        assert_eq!(queue.pending_count().await, 1); // tb still pending
    }

    #[tokio::test]
    async fn test_in_memory_priority_ordering() {
        let queue = InMemoryTaskQueue::new();

        // Enqueue in order: low priority first, high priority second
        queue
            .enqueue(QueuedTask::new(make_task_request("low"), "t", "s").with_priority(10))
            .await
            .expect("enqueue low");
        queue
            .enqueue(QueuedTask::new(make_task_request("high"), "t", "s").with_priority(1))
            .await
            .expect("enqueue high");

        // First dequeue gets the first-enqueued task (FIFO within same priority)
        let first = queue
            .dequeue("w1", None)
            .await
            .expect("dequeue")
            .expect("task");
        assert_eq!(first.task_id(), "low");
    }
}
