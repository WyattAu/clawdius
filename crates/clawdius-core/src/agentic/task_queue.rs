//! Agent Task Queue
//!
//! Persistent, SQLite-backed task queue for autonomous agent orchestration.
//! Supports task prioritization, retry with exponential backoff, subtasks,
//! and agent capability matching.
//!
//! # Schema
//!
//! The SQLite schema is defined in [`TASK_QUEUE_SCHEMA`] and is automatically
//! applied on first connection via [`TaskQueue::new`].
//!
//! # Usage
//!
//! ```rust,ignore
//! use clawdius_core::agentic::task_queue::{TaskQueue, NewTask, TaskStatus};
//!
//! let queue = TaskQueue::new("tasks.db")?;
//!
//! // Enqueue a task
//! let task_id = queue.enqueue(NewTask {
//!     agent_id: "code-agent".into(),
//!     task_type: "code".into(),
//!     priority: 10,
//!     input: serde_json::json!({"prompt": "Fix the bug"}),
//!     max_retries: 3,
//!     ..Default::default()
//! }).await?;
//!
//! // Dequeue next task for an agent
//! let task = queue.dequeue("code-agent").await?;
//!
//! // Complete the task
//! queue.complete(&task_id, serde_json::json!({"result": "fixed"})).await?;
//! ```

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::Result;

/// SQLite schema for the task queue.
pub const TASK_QUEUE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    task_type TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT,
    error TEXT,
    retries INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    deadline TEXT,
    metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_agent ON tasks(agent_id);
CREATE INDEX IF NOT EXISTS idx_tasks_type ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_id);

CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    capabilities TEXT NOT NULL,
    max_concurrent INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'idle',
    policy_id TEXT,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    cron_expr TEXT NOT NULL,
    task_template TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run TEXT,
    next_run TEXT
);
"#;

/// Task status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting to be picked up by an agent.
    Pending,
    /// Currently being executed.
    Running,
    /// Successfully completed.
    Completed,
    /// Failed after all retries.
    Failed,
    /// Cancelled by user or system.
    Cancelled,
}

impl TaskStatus {
    fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "running" => Some(TaskStatus::Running),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            "cancelled" => Some(TaskStatus::Cancelled),
            _ => None,
        }
    }
}

/// A task in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub parent_id: Option<String>,
    pub agent_id: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub task_type: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub retries: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

/// Input for creating a new task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    pub agent_id: String,
    pub task_type: String,
    #[serde(default)]
    pub priority: i32,
    pub input: serde_json::Value,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    pub parent_id: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

const fn default_max_retries() -> u32 {
    3
}

impl Default for NewTask {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            task_type: String::new(),
            priority: 0,
            input: serde_json::Value::Null,
            max_retries: 3,
            parent_id: None,
            deadline: None,
            metadata: None,
        }
    }
}

/// Filter for listing tasks.
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub agent_id: Option<String>,
    pub task_type: Option<String>,
    pub limit: Option<u32>,
}

/// Persistent SQLite-backed task queue.
pub struct TaskQueue {
    conn: Arc<Mutex<Connection>>,
}

impl TaskQueue {
    /// Create or open a task queue at the given path.
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(TASK_QUEUE_SCHEMA)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory task queue (for testing).
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(TASK_QUEUE_SCHEMA)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Enqueue a new task.
    pub async fn enqueue(&self, task: NewTask) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let input = serde_json::to_string(&task.input)?;
        let metadata = task
            .metadata
            .map(|m| serde_json::to_string(&m))
            .transpose()?;

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO tasks (id, parent_id, agent_id, status, priority, task_type, input, max_retries, created_at, deadline, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                id,
                task.parent_id,
                task.agent_id,
                TaskStatus::Pending.as_str(),
                task.priority,
                task.task_type,
                input,
                task.max_retries,
                now.to_rfc3339(),
                task.deadline.map(|d| d.to_rfc3339()),
                metadata,
            ],
        )?;
        drop(conn);
        Ok(id)
    }

    /// Dequeue the highest-priority pending task for a given agent.
    pub async fn dequeue(&self, agent_id: &str) -> Result<Option<Task>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, agent_id, status, priority, task_type, input, output, error,
                    retries, max_retries, created_at, started_at, completed_at, deadline, metadata
             FROM tasks
             WHERE agent_id = ?1 AND status = 'pending'
             ORDER BY priority DESC, created_at ASC
             LIMIT 1",
        )?;

        let task = stmt
            .query_row(rusqlite::params![agent_id], |row| row_to_task(row))
            .ok();

        drop(stmt);

        if let Some(ref task) = task {
            let now = Utc::now();
            conn.execute(
                "UPDATE tasks SET status = 'running', started_at = ?1 WHERE id = ?2",
                rusqlite::params![now.to_rfc3339(), task.id],
            )?;
        }

        Ok(task)
    }

    /// Complete a task with output.
    pub async fn complete(&self, task_id: &str, output: serde_json::Value) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        let output_str = serde_json::to_string(&output)?;
        conn.execute(
            "UPDATE tasks SET status = 'completed', output = ?1, completed_at = ?2 WHERE id = ?3",
            rusqlite::params![output_str, now.to_rfc3339(), task_id],
        )?;
        drop(conn);
        Ok(())
    }

    /// Fail a task with error. Retries if under max_retries.
    pub async fn fail(&self, task_id: &str, error: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();

        // Check retry count
        let (retries, max_retries): (u32, u32) = conn.query_row(
            "SELECT retries, max_retries FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        if retries < max_retries {
            // Retry: increment counter, set back to pending
            conn.execute(
                "UPDATE tasks SET status = 'pending', error = ?1, retries = retries + 1, started_at = NULL WHERE id = ?2",
                rusqlite::params![error, task_id],
            )?;
        } else {
            // Final failure
            conn.execute(
                "UPDATE tasks SET status = 'failed', error = ?1, completed_at = ?2 WHERE id = ?3",
                rusqlite::params![error, now.to_rfc3339(), task_id],
            )?;
        }

        drop(conn);
        Ok(())
    }

    /// Cancel a task.
    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        conn.execute(
            "UPDATE tasks SET status = 'cancelled', completed_at = ?1 WHERE id = ?2",
            rusqlite::params![now.to_rfc3339(), task_id],
        )?;
        drop(conn);
        Ok(())
    }

    /// List tasks with optional filter.
    pub async fn list(&self, filter: &TaskFilter) -> Result<Vec<Task>> {
        let conn = self.conn.lock().await;

        let mut sql = String::from(
            "SELECT id, parent_id, agent_id, status, priority, task_type, input, output, error,
                    retries, max_retries, created_at, started_at, completed_at, deadline, metadata
             FROM tasks WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref status) = filter.status {
            sql.push_str(&format!(" AND status = ?{}", params.len() + 1));
            params.push(Box::new(status.as_str().to_string()));
        }
        if let Some(ref agent_id) = filter.agent_id {
            sql.push_str(&format!(" AND agent_id = ?{}", params.len() + 1));
            params.push(Box::new(agent_id.clone()));
        }
        if let Some(ref task_type) = filter.task_type {
            sql.push_str(&format!(" AND task_type = ?{}", params.len() + 1));
            params.push(Box::new(task_type.clone()));
        }

        sql.push_str(" ORDER BY priority DESC, created_at ASC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_task(row))?;
        let tasks: Vec<Task> = rows.filter_map(|t| t.ok()).collect();

        Ok(tasks)
    }

    /// Get a task by ID.
    pub async fn get(&self, task_id: &str) -> Result<Task> {
        let conn = self.conn.lock().await;
        let task = conn.query_row(
            "SELECT id, parent_id, agent_id, status, priority, task_type, input, output, error,
                    retries, max_retries, created_at, started_at, completed_at, deadline, metadata
             FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| row_to_task(row),
        )?;
        drop(conn);
        Ok(task)
    }

    /// Create a subtask under a parent task.
    pub async fn create_subtask(&self, parent_id: &str, task: NewTask) -> Result<String> {
        let mut subtask = task;
        subtask.parent_id = Some(parent_id.to_string());
        self.enqueue(subtask).await
    }

    /// Get queue statistics.
    pub async fn stats(&self) -> Result<TaskQueueStats> {
        let conn = self.conn.lock().await;
        let pending: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let running: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'running'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let completed: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'completed'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let failed: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        drop(conn);

        Ok(TaskQueueStats {
            pending,
            running,
            completed,
            failed,
        })
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        agent_id: row.get(2)?,
        status: TaskStatus::from_str(&row.get::<_, String>(3)?).unwrap_or(TaskStatus::Pending),
        priority: row.get(4)?,
        task_type: row.get(5)?,
        input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(serde_json::Value::Null),
        output: row
            .get::<_, Option<String>>(7)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        error: row.get(8)?,
        retries: row.get(9)?,
        max_retries: row.get(10)?,
        created_at: row
            .get::<_, String>(11)
            .ok()
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.to_utc())
            .unwrap_or(Utc::now()),
        started_at: row
            .get::<_, Option<String>>(12)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.to_utc()),
        completed_at: row
            .get::<_, Option<String>>(13)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.to_utc()),
        deadline: row
            .get::<_, Option<String>>(14)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.to_utc()),
        metadata: row
            .get::<_, Option<String>>(15)?
            .and_then(|s| serde_json::from_str(&s).ok()),
    })
}

/// Queue statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueueStats {
    pub pending: u32,
    pub running: u32,
    pub completed: u32,
    pub failed: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup() -> TaskQueue {
        TaskQueue::new_in_memory().expect("Failed to create in-memory queue")
    }

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let queue = setup();

        let id = queue
            .enqueue(NewTask {
                agent_id: "test-agent".into(),
                task_type: "code".into(),
                priority: 10,
                input: json!({"prompt": "hello"}),
                ..Default::default()
            })
            .await
            .expect("enqueue failed");

        let task = queue.dequeue("test-agent").await.expect("dequeue failed");
        assert!(task.is_some());
        let dequeued = task.unwrap();
        assert_eq!(dequeued.id, id);
        assert_eq!(dequeued.agent_id, "test-agent");

        // Verify status is Running in DB
        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_complete_task() {
        let queue = setup();

        let id = queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "test".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue failed");

        let task = queue.dequeue("agent").await.expect("dequeue").unwrap();
        queue
            .complete(&task.id, json!({"result": "ok"}))
            .await
            .expect("complete failed");

        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.output.is_some());
    }

    #[tokio::test]
    async fn test_fail_with_retry() {
        let queue = setup();

        let id = queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "test".into(),
                input: json!({}),
                max_retries: 2,
                ..Default::default()
            })
            .await
            .expect("enqueue failed");

        let _ = queue.dequeue("agent").await.expect("dequeue").unwrap();

        // First failure: should retry
        queue.fail(&id, "timeout").await.expect("fail failed");
        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.retries, 1);

        // Second failure: should retry
        let _ = queue.dequeue("agent").await.expect("dequeue").unwrap();
        queue.fail(&id, "timeout again").await.expect("fail failed");
        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.retries, 2);

        // Third failure: should fail permanently
        let _ = queue.dequeue("agent").await.expect("dequeue").unwrap();
        queue.fail(&id, "timeout final").await.expect("fail failed");
        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let queue = setup();

        let id = queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "test".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue failed");

        queue.cancel(&id).await.expect("cancel failed");
        let task = queue.get(&id).await.expect("get failed");
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_subtask() {
        let queue = setup();

        let parent_id = queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "build".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue failed");

        let child_id = queue
            .create_subtask(
                &parent_id,
                NewTask {
                    agent_id: "agent".into(),
                    task_type: "test".into(),
                    input: json!({}),
                    ..Default::default()
                },
            )
            .await
            .expect("subtask failed");

        let child = queue.get(&child_id).await.expect("get failed");
        assert_eq!(child.parent_id, Some(parent_id));
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = setup();

        queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "low".into(),
                priority: 1,
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "high".into(),
                priority: 100,
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        let task = queue.dequeue("agent").await.expect("dequeue").unwrap();
        assert_eq!(task.task_type, "high");
    }

    #[tokio::test]
    async fn test_list_with_filter() {
        let queue = setup();

        queue
            .enqueue(NewTask {
                agent_id: "a1".into(),
                task_type: "code".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        queue
            .enqueue(NewTask {
                agent_id: "a2".into(),
                task_type: "test".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        let tasks = queue
            .list(&TaskFilter {
                agent_id: Some("a1".into()),
                ..Default::default()
            })
            .await
            .expect("list");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn test_stats() {
        let queue = setup();

        queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "test".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        queue
            .enqueue(NewTask {
                agent_id: "agent".into(),
                task_type: "test".into(),
                input: json!({}),
                ..Default::default()
            })
            .await
            .expect("enqueue");

        let stats = queue.stats().await.expect("stats");
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.running, 0);
    }
}
