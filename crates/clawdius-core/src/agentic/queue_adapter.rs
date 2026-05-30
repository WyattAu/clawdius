//! SQLite Queue Adapter
//!
//! Bridges [`crate::agentic::task_queue::TaskQueue`] (SQLite-backed) to the
//! [`crate::orchestrator::TaskQueue`] trait, allowing the Orchestrator to use
//! persistent SQLite storage instead of in-memory or Redis queues.

use async_trait::async_trait;
use std::sync::Arc;

use crate::agentic::task_queue::{NewTask, TaskQueue as SqliteTaskQueue, TaskStatus as SqliteStatus};
use crate::error::{Error, Result};
use crate::orchestrator::{current_timestamp, queue::TaskQueue, QueuedTask, TaskStatus};

/// Adapter wrapping a SQLite-backed task queue to implement the orchestrator
/// `TaskQueue` trait.
///
/// Maps between the orchestrator's `QueuedTask` and the agentic `NewTask`/`Task`
/// types. Metadata fields not natively supported by the SQLite schema (e.g.
/// `claimed_by`, `last_heartbeat`, `tenant_id`) are stored as JSON in the
/// `metadata` column.
pub struct SqliteQueueAdapter {
    inner: Arc<SqliteTaskQueue>,
}

impl SqliteQueueAdapter {
    /// Creates a new adapter wrapping the given SQLite task queue.
    #[must_use]
    pub fn new(queue: Arc<SqliteTaskQueue>) -> Self {
        Self { inner: queue }
    }
}

#[async_trait]
impl TaskQueue for SqliteQueueAdapter {
    async fn enqueue(&self, task: QueuedTask) -> Result<String> {
        let new_task = NewTask {
            agent_id: task
                .claimed_by
                .clone()
                .unwrap_or_else(|| "unassigned".to_string()),
            task_type: "orchestrator".to_string(),
            priority: -(task.priority as i32), // Negate: orchestrator priority 0=highest, SQLite DESC
            input: serde_json::to_value(&task.request).map_err(Error::Serialization)?,
            max_retries: task.max_retries,
            parent_id: None,
            deadline: None,
            metadata: Some(serde_json::json!({
                "tenant_id": task.tenant_id,
                "session_id": task.session_id,
                "claimed_by": task.claimed_by,
                "proxy_url": task.proxy_url,
                "enqueued_at": task.enqueued_at,
                "retry_count": task.retry_count,
            })),
        };

        let id = self
            .inner
            .enqueue_with_id(task.task_id().to_string(), new_task)
            .await?;
        Ok(id)
    }

    async fn dequeue(
        &self,
        worker_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<Option<QueuedTask>> {
        // List pending tasks and claim the first matching one.
        // We cannot use SQLite dequeue directly because it filters by agent_id
        // and the adapter uses 'unassigned' as agent_id during enqueue.
        let filter = crate::agentic::task_queue::TaskFilter {
            status: Some(SqliteStatus::Pending),
            ..Default::default()
        };
        let pending = self.inner.list(&filter).await?;

        // Find first matching task
        let target = if let Some(expected_tenant) = tenant_id {
            pending
                .into_iter()
                .find(|t| {
                    t.metadata
                        .as_ref()
                        .and_then(|m| m.get("tenant_id"))
                        .and_then(|v: &serde_json::Value| v.as_str())
                        == Some(expected_tenant)
                })
        } else {
            pending.into_iter().next()
        };

        let Some(target) = target else {
            return Ok(None);
        };

        // Claim the task (pending -> running)
        let task = match self.inner.claim_task(&target.id, worker_id).await? {
            Some(t) => t,
            None => return Ok(None),
        };

        // Reconstruct QueuedTask from SQLite Task
        let request = serde_json::from_value::<crate::agentic::TaskRequest>(task.input.clone())
            .unwrap_or_else(|_| default_task_request(&task.id));

        let mut qt = QueuedTask::new(request, "", "");

        qt.status = match task.status {
            SqliteStatus::Pending => TaskStatus::Pending,
            SqliteStatus::Running => TaskStatus::Running,
            SqliteStatus::Completed => TaskStatus::Completed,
            SqliteStatus::Failed => TaskStatus::Failed,
            SqliteStatus::Cancelled => TaskStatus::Cancelled,
        };
        qt.claimed_by = Some(worker_id.to_string());

        // Restore metadata fields
        if let Some(ref meta) = task.metadata {
            qt.tenant_id = meta
                .get("tenant_id")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("")
                .to_string();
            qt.session_id = meta
                .get("session_id")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("")
                .to_string();
            qt.proxy_url = meta
                .get("proxy_url")
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(String::from);
            qt.enqueued_at = meta
                .get("enqueued_at")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0);
            qt.retry_count = meta
                .get("retry_count")
                .and_then(|v: &serde_json::Value| v.as_u64().map(|n| n as u32))
                .unwrap_or(0);
        }

        // Note: heartbeat is NOT set here. The orchestrator worker calls
        // record_heartbeat() separately via the heartbeat loop.

        Ok(Some(qt))
    }

    async fn update_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        match status {
            TaskStatus::Completed => {
                self.inner
                    .complete(task_id, serde_json::json!({"status": "completed"}))
                    .await?;
            }
            TaskStatus::Failed => {
                self.inner
                    .force_fail(task_id, "orchestrator: task failed")
                    .await?;
            }
            TaskStatus::Cancelled => {
                self.inner.cancel(task_id).await?;
            }
            TaskStatus::Pending => {
                // Reset to pending (for retry)
                self.inner.fail(task_id, "reset to pending").await?;
            }
            TaskStatus::Running | TaskStatus::TimedOut => {
                let _ = self.inner.claim_task(task_id, "adapter").await?;
            }
        }
        Ok(())
    }

    async fn task_status(&self, task_id: &str) -> Result<TaskStatus> {
        let task = self.inner.get(task_id).await?;
        Ok(match task.status {
            SqliteStatus::Pending => TaskStatus::Pending,
            SqliteStatus::Running => TaskStatus::Running,
            SqliteStatus::Completed => TaskStatus::Completed,
            SqliteStatus::Failed => TaskStatus::Failed,
            SqliteStatus::Cancelled => TaskStatus::Cancelled,
        })
    }

    async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.inner.cancel(task_id).await
    }

    async fn record_heartbeat(&self, task_id: &str, _worker_id: &str) -> Result<()> {
        self.inner
            .update_metadata(
                task_id,
                serde_json::json!({"last_heartbeat": current_timestamp()}),
            )
            .await
    }

    async fn stale_tasks(&self, timeout_ms: u64) -> Result<Vec<QueuedTask>> {
        let filter = crate::agentic::task_queue::TaskFilter {
            status: Some(SqliteStatus::Running),
            ..Default::default()
        };
        let tasks = self.inner.list(&filter).await?;
        let now = current_timestamp();

        Ok(tasks
            .into_iter()
            .filter(|t| {
                t.metadata
                    .as_ref()
                    .and_then(|m| m.get("last_heartbeat"))
                    .and_then(|v: &serde_json::Value| v.as_u64())
                    .map(|hb| now.saturating_sub(hb) > timeout_ms)
                    .unwrap_or(true) // No heartbeat = stale
            })
            .map(|t| {
                let tenant_id = t
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("tenant_id"))
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let session_id = t
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("session_id"))
                    .and_then(|v: &serde_json::Value| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let request = serde_json::from_value::<crate::agentic::TaskRequest>(t.input.clone())
                    .unwrap_or_else(|_| default_task_request(&t.id));
                QueuedTask::new(request, tenant_id, session_id)
            })
            .collect())
    }

    async fn push_result(&self, task_id: &str, result: &str) -> Result<()> {
        self.inner
            .update_metadata(task_id, serde_json::json!({"result": result}))
            .await
    }

    async fn pop_result(&self) -> Result<Option<String>> {
        // SQLite queue does not have a separate results queue.
        // Callers should query tasks directly for results.
        Ok(None)
    }

    async fn pending_count(&self) -> usize {
        self.inner
            .stats()
            .await
            .map(|s| s.pending as usize)
            .unwrap_or(0)
    }

    async fn running_count(&self) -> usize {
        self.inner
            .stats()
            .await
            .map(|s| s.running as usize)
            .unwrap_or(0)
    }
}

/// Creates a default `TaskRequest` with the given ID for fallback deserialization.
fn default_task_request(id: &str) -> crate::agentic::TaskRequest {
    crate::agentic::TaskRequest {
        id: id.to_string(),
        description: String::new(),
        target_files: Vec::new(),
        mode: crate::agentic::GenerationMode::SinglePass,
        test_strategy: crate::agentic::TestExecutionStrategy::Skip,
        apply_workflow: crate::agentic::ApplyWorkflow::PreviewOnly,
        context: Default::default(),
        trust_level: crate::agentic::TrustLevel::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::task_queue::TaskQueue as SqliteQueue;
    use crate::agentic::{
        ApplyWorkflow, GenerationMode, TaskRequest, TaskContext, TestExecutionStrategy, TrustLevel,
    };

    fn make_task_request(id: &str) -> TaskRequest {
        TaskRequest {
            id: id.to_string(),
            description: "Adapter test".to_string(),
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
    async fn test_adapter_enqueue_dequeue() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        let task = QueuedTask::new(make_task_request("adapter-1"), "tenant-a", "session-1");
        let id = adapter.enqueue(task).await.expect("enqueue");
        assert_eq!(id, "adapter-1");
        assert_eq!(adapter.pending_count().await, 1);

        let dequeued = adapter
            .dequeue("worker-0", None)
            .await
            .expect("dequeue")
            .expect("task");
        assert_eq!(dequeued.task_id(), "adapter-1");
        assert_eq!(dequeued.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_adapter_cancel() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("cancel-1"), "t", "s"))
            .await
            .expect("enqueue");

        adapter.cancel_task("cancel-1").await.expect("cancel");
        let status = adapter.task_status("cancel-1").await.expect("status");
        assert_eq!(status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_adapter_status_lifecycle() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("lifecycle"), "t", "s"))
            .await
            .expect("enqueue");

        assert_eq!(
            adapter.task_status("lifecycle").await.expect("status"),
            TaskStatus::Pending
        );

        adapter
            .update_status("lifecycle", TaskStatus::Running)
            .await
            .expect("update running");
        assert_eq!(
            adapter.task_status("lifecycle").await.expect("status"),
            TaskStatus::Running
        );

        adapter
            .update_status("lifecycle", TaskStatus::Completed)
            .await
            .expect("update completed");
        assert_eq!(
            adapter.task_status("lifecycle").await.expect("status"),
            TaskStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_adapter_counts() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("c1"), "t", "s"))
            .await
            .expect("enqueue");
        adapter
            .enqueue(QueuedTask::new(make_task_request("c2"), "t", "s"))
            .await
            .expect("enqueue");

        assert_eq!(adapter.pending_count().await, 2);
        assert_eq!(adapter.running_count().await, 0);

        adapter.dequeue("w1", None).await.expect("dequeue");
        assert_eq!(adapter.pending_count().await, 1);
        assert_eq!(adapter.running_count().await, 1);
    }

    #[tokio::test]
    async fn test_adapter_heartbeat() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("hb-1"), "t", "s"))
            .await
            .expect("enqueue");
        adapter.dequeue("w1", None).await.expect("dequeue");

        adapter
            .record_heartbeat("hb-1", "w1")
            .await
            .expect("heartbeat");

        // Task should not be stale immediately
        let stale = adapter.stale_tasks(60_000).await.expect("stale");
        assert!(stale.is_empty());
    }

    #[tokio::test]
    async fn test_adapter_stale_detection() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("stale-1"), "t", "s"))
            .await
            .expect("enqueue");
        adapter.dequeue("w1", None).await.expect("dequeue");

        // No heartbeat recorded on dequeue, so task with timeout=0 should be stale
        let stale = adapter.stale_tasks(0).await.expect("stale");
        assert_eq!(stale.len(), 1);
    }

    #[tokio::test]
    async fn test_adapter_push_pop_result() {
        let sqlite = SqliteQueue::new_in_memory().expect("sqlite queue");
        let adapter = SqliteQueueAdapter::new(Arc::new(sqlite));

        adapter
            .enqueue(QueuedTask::new(make_task_request("r1"), "t", "s"))
            .await
            .expect("enqueue");

        adapter
            .push_result("r1", r#"{"success": true}"#)
            .await
            .expect("push");

        // pop_result returns None since SQLite has no separate results queue
        let result = adapter.pop_result().await.expect("pop");
        assert!(result.is_none());
    }
}
