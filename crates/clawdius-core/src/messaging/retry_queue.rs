//! Webhook Delivery Retry Queue
//!
//! Manages retry logic for failed webhook deliveries using exponential backoff
//! with jitter. Supports SQLite persistence and a dead letter queue for
//! permanently failed tasks.

#![deny(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

use super::state_store::StateStore;
use super::types::{MessagingError, Platform, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryTask {
    pub id: String,
    pub platform: Platform,
    pub chat_id: String,
    pub message: String,
    pub attempt: u32,
    pub max_retries: u32,
    pub next_retry_at: u64,
    pub created_at: u64,
    pub last_error: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f64,
    pub jitter_factor: f64,
    pub max_queue_size: usize,
    pub dead_letter_enabled: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 300_000,
            exponential_base: 2.0,
            jitter_factor: 0.1,
            max_queue_size: 10_000,
            dead_letter_enabled: true,
        }
    }
}

#[derive(Clone)]
pub struct RetryQueue {
    tasks: Arc<RwLock<HashMap<String, RetryTask>>>,
    dead_letter: Arc<RwLock<HashMap<String, RetryTask>>>,
    config: RetryConfig,
    db_path: Option<String>,
    pub state_store: Option<Arc<dyn StateStore>>,
}

impl RetryQueue {
    #[must_use]
    pub fn new(config: RetryConfig) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(HashMap::new())),
            config,
            db_path: None,
            state_store: None,
        }
    }

    #[must_use]
    pub fn with_persistence(db_path: impl Into<String>, config: RetryConfig) -> Self {
        let path = db_path.into();
        Self::init_db(&path);
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(HashMap::new())),
            config,
            db_path: Some(path),
            state_store: None,
        }
    }

    pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.state_store = Some(store);
        self
    }

    pub fn has_persistence(&self) -> bool {
        self.db_path.is_some()
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn compute_delay_ms(&self, attempt: u32) -> u64 {
        let exponential_delay =
            self.config.initial_delay_ms as f64 * self.config.exponential_base.powi(attempt as i32);
        let capped = exponential_delay.min(self.config.max_delay_ms as f64);
        let jitter_range = 2.0 * self.config.jitter_factor;
        let jitter_offset = if jitter_range > 0.0 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let pseudo_random = ((nanos as f64) * 1.0e-9).fract();
            jitter_range * pseudo_random
        } else {
            0.0
        };
        let jittered = capped * (1.0 - self.config.jitter_factor + jitter_offset);
        let result = jittered.round() as u64;
        result.min(self.config.max_delay_ms)
    }

    pub async fn enqueue(
        &self,
        platform: Platform,
        chat_id: impl Into<String>,
        message: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<String> {
        let mut tasks = self.tasks.write().await;

        if tasks.len() + self.dead_letter.read().await.len() >= self.config.max_queue_size {
            return Err(MessagingError::SendFailed(
                "Retry queue is full".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = Self::current_timestamp();

        let task = RetryTask {
            id: id.clone(),
            platform,
            chat_id: chat_id.into(),
            message: message.into(),
            attempt: 0,
            max_retries: self.config.max_retries,
            next_retry_at: now,
            created_at: now,
            last_error: None,
            payload,
        };

        let task_clone = task.clone();
        tasks.insert(id.clone(), task);
        drop(tasks);

        self.persist_task_db(&task_clone).await;
        debug!(task_id = %id, "Enqueued retry task");

        Ok(id)
    }

    pub async fn dequeue_ready(&self) -> Vec<RetryTask> {
        let now = Self::current_timestamp();
        let tasks = self.tasks.read().await;

        let ready: Vec<RetryTask> = tasks
            .values()
            .filter(|t| t.next_retry_at <= now)
            .cloned()
            .collect();

        ready
    }

    pub async fn mark_success(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        if tasks.remove(task_id).is_none() {
            return Err(MessagingError::ParseError(format!(
                "Task {task_id} not found"
            )));
        }
        drop(tasks);

        self.delete_task_db(task_id).await;
        debug!(task_id = %task_id, "Retry task succeeded");
        Ok(())
    }

    pub async fn mark_failed(&self, task_id: &str, error: impl Into<String>) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| MessagingError::ParseError(format!("Task {task_id} not found")))?;

        task.attempt += 1;
        task.last_error = Some(error.into());

        if task.attempt >= task.max_retries && self.config.dead_letter_enabled {
            let exhausted = tasks.remove(task_id).ok_or_else(|| {
                MessagingError::ParseError(format!("Task {task_id} not found after remove"))
            })?;
            drop(tasks);

            let mut dead_letter = self.dead_letter.write().await;
            dead_letter.insert(task_id.to_string(), exhausted.clone());
            drop(dead_letter);

            self.persist_task_db(&exhausted).await;
            warn!(
                task_id = %task_id,
                attempt = exhausted.attempt,
                "Retry task exhausted, moved to dead letter queue"
            );
            return Ok(());
        }

        if task.attempt >= task.max_retries {
            let exhausted = tasks.remove(task_id).ok_or_else(|| {
                MessagingError::ParseError(format!("Task {task_id} not found after remove"))
            })?;
            drop(tasks);

            self.delete_task_db(task_id).await;
            warn!(
                task_id = %task_id,
                attempt = exhausted.attempt,
                "Retry task exhausted, discarded (dead letter disabled)"
            );
            return Ok(());
        }

        let delay_ms = self.compute_delay_ms(task.attempt);
        task.next_retry_at = Self::current_timestamp() + delay_ms / 1000 + 1;

        let updated = task.clone();
        drop(tasks);

        self.persist_task_db(&updated).await;
        debug!(
            task_id = %task_id,
            attempt = updated.attempt,
            delay_ms,
            "Retry task scheduled for next attempt"
        );
        Ok(())
    }

    pub async fn pending_count(&self) -> usize {
        self.tasks.read().await.len()
    }

    pub async fn dead_letter_count(&self) -> usize {
        self.dead_letter.read().await.len()
    }

    pub async fn retryable_count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|t| t.attempt < t.max_retries).count()
    }

    pub async fn purge(&self) -> Result<usize> {
        let mut dead_letter = self.dead_letter.write().await;
        let count = dead_letter.len();
        let ids: Vec<String> = dead_letter.keys().cloned().collect();
        dead_letter.clear();
        drop(dead_letter);

        for id in &ids {
            self.delete_task_db(id).await;
        }

        debug!(count, "Purged dead letter queue");
        Ok(count)
    }

    pub async fn dead_letter_tasks(&self) -> Vec<RetryTask> {
        self.dead_letter.read().await.values().cloned().collect()
    }

    /// Prepare for shutdown. Returns (pending_count, dead_letter_count).
    /// The caller should drain pending tasks or persist them before drop.
    pub async fn shutdown(&self) -> (usize, usize) {
        let pending = self.tasks.read().await.len();
        let dead = self.dead_letter.read().await.len();
        warn!(
            pending = pending,
            dead_letter = dead,
            "RetryQueue shutting down"
        );
        (pending, dead)
    }

    pub async fn load_from_db(&self) -> usize {
        if let Some(store) = &self.state_store {
            let keys = match store.keys("messaging_retry_queue", "*").await {
                Ok(k) => k,
                Err(_) => return 0,
            };
            let mut active_count = 0usize;
            let mut dead_count = 0usize;
            let mut tasks = self.tasks.write().await;
            let mut dl = self.dead_letter.write().await;
            for key in &keys {
                if let Ok(Some(value)) = store.get("messaging_retry_queue", key).await {
                    if let Ok(task) = serde_json::from_slice::<RetryTask>(&value) {
                        if task.attempt >= task.max_retries && self.config.dead_letter_enabled {
                            dl.entry(key.clone()).or_insert(task);
                            dead_count += 1;
                        } else {
                            tasks.entry(key.clone()).or_insert(task);
                            active_count += 1;
                        }
                    }
                }
            }
            return active_count + dead_count;
        }

        let Some(ref path) = self.db_path else {
            return 0;
        };
        let path = path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = match rusqlite::Connection::open(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to open retry queue DB for load");
                    return (Vec::new(), Vec::new());
                }
            };

            let mut active: Vec<RetryTask> = Vec::new();
            let mut dead: Vec<RetryTask> = Vec::new();

            let mut stmt = match conn.prepare(
                "SELECT id, platform, chat_id, message, attempt, max_retries, next_retry_at, created_at, last_error, payload, is_dead_letter FROM retry_queue",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to prepare retry queue load query");
                    return (active, dead);
                }
            };

            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, i32>(10)?,
                ))
            });

            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    let (id, platform_str, chat_id, message, attempt, max_retries, next_retry_at, created_at, last_error, payload_str, is_dead_letter): (String, String, String, String, u32, u32, i64, i64, Option<String>, Option<String>, i32) = row;

                    let platform = match platform_str.as_str() {
                        "telegram" => Platform::Telegram,
                        "discord" => Platform::Discord,
                        "matrix" => Platform::Matrix,
                        "signal" => Platform::Signal,
                        "rocketchat" => Platform::RocketChat,
                        "whatsapp" => Platform::WhatsApp,
                        "slack" => Platform::Slack,
                        "webhook" => Platform::Webhook,
                        _ => continue,
                    };

                    let payload = payload_str
                        .and_then(|p| serde_json::from_str(&p).ok())
                        .unwrap_or(serde_json::Value::Null);

                    let task = RetryTask {
                        id,
                        platform,
                        chat_id,
                        message,
                        attempt,
                        max_retries,
                        next_retry_at: next_retry_at as u64,
                        created_at: created_at as u64,
                        last_error,
                        payload,
                    };

                    if is_dead_letter != 0 {
                        dead.push(task);
                    } else {
                        active.push(task);
                    }
                }
            }

            (active, dead)
        })
        .await;

        match result {
            Ok((active, dead)) => {
                let total = active.len() + dead.len();
                let active_count = active.len();
                let dead_count = dead.len();
                let mut tasks = self.tasks.write().await;
                for task in active {
                    tasks.entry(task.id.clone()).or_insert(task);
                }
                let mut dl = self.dead_letter.write().await;
                for task in dead {
                    dl.entry(task.id.clone()).or_insert(task);
                }
                debug!(
                    active = active_count,
                    dead = dead_count,
                    "Loaded retry tasks from DB"
                );
                total
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load retry queue from DB");
                0
            },
        }
    }

    fn init_db(path: &str) {
        let conn = match rusqlite::Connection::open(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Failed to open retry queue database");
                return;
            },
        };
        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS retry_queue (
                id              TEXT PRIMARY KEY,
                platform        TEXT NOT NULL,
                chat_id         TEXT NOT NULL,
                message         TEXT NOT NULL,
                attempt         INTEGER NOT NULL DEFAULT 0,
                max_retries     INTEGER NOT NULL DEFAULT 5,
                next_retry_at   INTEGER NOT NULL,
                created_at      INTEGER NOT NULL,
                last_error      TEXT,
                payload         TEXT,
                is_dead_letter  INTEGER NOT NULL DEFAULT 0
            )",
            [],
        ) {
            tracing::error!(error = %e, "Failed to create retry_queue table");
        }
    }

    async fn persist_task_db(&self, task: &RetryTask) {
        if let Some(store) = &self.state_store {
            let key = task.id.clone();
            let value = serde_json::to_vec(task).unwrap_or_default();
            let _ = store.set("messaging_retry_queue", &key, &value, None).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let id = task.id.clone();
        let platform = task.platform.as_str().to_string();
        let chat_id = task.chat_id.clone();
        let message = task.message.clone();
        let attempt = task.attempt;
        let max_retries = task.max_retries;
        let next_retry_at = task.next_retry_at as i64;
        let created_at = task.created_at as i64;
        let last_error = task.last_error.clone();
        let payload_str = serde_json::to_string(&task.payload).unwrap_or_default();
        let is_dead_letter = self.dead_letter.read().await.contains_key(&task.id);

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "INSERT OR REPLACE INTO retry_queue (id, platform, chat_id, message, attempt, max_retries, next_retry_at, created_at, last_error, payload, is_dead_letter) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![id, platform, chat_id, message, attempt, max_retries, next_retry_at, created_at, last_error, payload_str, is_dead_letter],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to persist retry task"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for retry task persist"),
        }
    }

    async fn delete_task_db(&self, task_id: &str) {
        if let Some(store) = &self.state_store {
            let _ = store.delete("messaging_retry_queue", task_id).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let id = task_id.to_string();

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "DELETE FROM retry_queue WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {},
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to delete retry task from DB"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for retry task delete"),
        }
    }
}

impl Default for RetryQueue {
    fn default() -> Self {
        Self::new(RetryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RetryConfig {
        RetryConfig {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 60_000,
            exponential_base: 2.0,
            jitter_factor: 0.0,
            max_queue_size: 100,
            dead_letter_enabled: true,
        }
    }

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let queue = RetryQueue::new(test_config());
        let id = queue
            .enqueue(Platform::Telegram, "chat1", "hello", serde_json::json!({}))
            .await
            .unwrap();

        let ready = queue.dequeue_ready().await;
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, id);
        assert_eq!(ready[0].platform, Platform::Telegram);
        assert_eq!(ready[0].chat_id, "chat1");
        assert_eq!(ready[0].attempt, 0);
    }

    #[tokio::test]
    async fn test_dequeue_only_returns_ready_tasks() {
        let mut config = test_config();
        config.initial_delay_ms = 100_000;
        let queue = RetryQueue::new(config);

        queue
            .enqueue(
                Platform::Discord,
                "chat1",
                "future msg",
                serde_json::json!({}),
            )
            .await
            .unwrap();

        let mut tasks = queue.tasks.write().await;
        for task in tasks.values_mut() {
            task.next_retry_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 100_000;
        }
        drop(tasks);

        let ready = queue.dequeue_ready().await;
        assert!(ready.is_empty());
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 300_000,
            exponential_base: 2.0,
            jitter_factor: 0.0,
            max_queue_size: 100,
            dead_letter_enabled: true,
        };
        let queue = RetryQueue::new(config);

        let d0 = queue.compute_delay_ms(0);
        assert_eq!(d0, 1000);

        let d1 = queue.compute_delay_ms(1);
        assert_eq!(d1, 2000);

        let d2 = queue.compute_delay_ms(2);
        assert_eq!(d2, 4000);

        let d3 = queue.compute_delay_ms(3);
        assert_eq!(d3, 8000);
    }

    #[tokio::test]
    async fn test_jitter_within_expected_range() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 300_000,
            exponential_base: 2.0,
            jitter_factor: 0.1,
            max_queue_size: 100,
            dead_letter_enabled: true,
        };
        let queue = RetryQueue::new(config);

        let base_delay = 1000.0;
        let jitter_pct = 0.1;
        let mut all_in_range = true;

        for _ in 0..50 {
            let delay = queue.compute_delay_ms(0) as f64;
            let min = base_delay * (1.0 - jitter_pct);
            let max = base_delay * (1.0 + jitter_pct);
            if delay < min || delay > max {
                all_in_range = false;
                break;
            }
        }
        assert!(all_in_range, "Jittered delays should be within ±10%");
    }

    #[tokio::test]
    async fn test_max_delay_cap() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            exponential_base: 10.0,
            jitter_factor: 0.0,
            max_queue_size: 100,
            dead_letter_enabled: true,
        };
        let queue = RetryQueue::new(config);

        let d5 = queue.compute_delay_ms(5);
        assert_eq!(d5, 5000);

        let d10 = queue.compute_delay_ms(10);
        assert_eq!(d10, 5000);
    }

    #[tokio::test]
    async fn test_mark_success_removes_task() {
        let queue = RetryQueue::new(test_config());
        let id = queue
            .enqueue(Platform::Telegram, "chat1", "msg", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(queue.pending_count().await, 1);
        queue.mark_success(&id).await.unwrap();
        assert_eq!(queue.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_mark_failed_increments_attempt() {
        let queue = RetryQueue::new(test_config());
        let id = queue
            .enqueue(Platform::Discord, "chat1", "msg", serde_json::json!({}))
            .await
            .unwrap();

        queue.mark_failed(&id, "timeout").await.unwrap();

        let tasks = queue.tasks.read().await;
        let task = tasks.get(&id).unwrap();
        assert_eq!(task.attempt, 1);
        assert_eq!(task.last_error.as_deref(), Some("timeout"));
    }

    #[tokio::test]
    async fn test_max_retries_to_dead_letter() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay_ms: 100,
            max_delay_ms: 60_000,
            exponential_base: 2.0,
            jitter_factor: 0.0,
            max_queue_size: 100,
            dead_letter_enabled: true,
        };
        let queue = RetryQueue::new(config);
        let id = queue
            .enqueue(Platform::Matrix, "chat1", "msg", serde_json::json!({}))
            .await
            .unwrap();

        queue.mark_failed(&id, "err1").await.unwrap();
        queue.mark_failed(&id, "err2").await.unwrap();

        assert_eq!(queue.pending_count().await, 0);
        assert_eq!(queue.dead_letter_count().await, 1);
    }

    #[tokio::test]
    async fn test_dead_letter_tasks_returns_exhausted() {
        let queue = RetryQueue::new(test_config());
        let id = queue
            .enqueue(Platform::Slack, "chat1", "msg", serde_json::json!({}))
            .await
            .unwrap();

        for i in 0..3 {
            queue.mark_failed(&id, format!("err{i}")).await.unwrap();
        }

        let dl = queue.dead_letter_tasks().await;
        assert_eq!(dl.len(), 1);
        assert_eq!(dl[0].id, id);
        assert_eq!(dl[0].attempt, 3);
    }

    #[tokio::test]
    async fn test_pending_count_and_retryable_count() {
        let queue = RetryQueue::new(test_config());
        let id1 = queue
            .enqueue(Platform::Telegram, "chat1", "msg1", serde_json::json!({}))
            .await
            .unwrap();
        let _id2 = queue
            .enqueue(Platform::Discord, "chat2", "msg2", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(queue.pending_count().await, 2);
        assert_eq!(queue.retryable_count().await, 2);

        for _ in 0..3 {
            queue.mark_failed(&id1, "err").await.unwrap();
        }

        assert_eq!(queue.pending_count().await, 1);
        assert_eq!(queue.retryable_count().await, 1);
        assert_eq!(queue.dead_letter_count().await, 1);
    }

    #[tokio::test]
    async fn test_purge_removes_dead_letter() {
        let queue = RetryQueue::new(test_config());
        let id = queue
            .enqueue(Platform::WhatsApp, "chat1", "msg", serde_json::json!({}))
            .await
            .unwrap();

        for _ in 0..3 {
            queue.mark_failed(&id, "err").await.unwrap();
        }

        assert_eq!(queue.dead_letter_count().await, 1);
        let purged = queue.purge().await.unwrap();
        assert_eq!(purged, 1);
        assert_eq!(queue.dead_letter_count().await, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_persistence_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("retry_queue.db");
        let path_str = path.to_str().unwrap().to_string();

        let queue = RetryQueue::with_persistence(&path_str, test_config());
        assert!(queue.has_persistence());

        let id1 = queue
            .enqueue(
                Platform::Telegram,
                "chat1",
                "hello",
                serde_json::json!({"k": "v1"}),
            )
            .await
            .unwrap();
        let _id2 = queue
            .enqueue(
                Platform::Discord,
                "chat2",
                "world",
                serde_json::json!({"k": "v2"}),
            )
            .await
            .unwrap();

        queue.mark_failed(&id1, "timeout").await.unwrap();

        for _ in 0..2 {
            queue.mark_failed(&id1, "err").await.unwrap();
        }

        drop(queue);

        let queue2 = RetryQueue::with_persistence(&path_str, test_config());
        let loaded = queue2.load_from_db().await;
        assert!(loaded >= 1, "should load at least 1 task, got {loaded}");

        assert_eq!(queue2.pending_count().await, 1);
        assert_eq!(queue2.dead_letter_count().await, 1);
    }

    #[tokio::test]
    async fn test_max_queue_size_enforcement() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 60_000,
            exponential_base: 2.0,
            jitter_factor: 0.0,
            max_queue_size: 2,
            dead_letter_enabled: true,
        };
        let queue = RetryQueue::new(config);

        queue
            .enqueue(Platform::Telegram, "c1", "m1", serde_json::json!({}))
            .await
            .unwrap();
        queue
            .enqueue(Platform::Discord, "c2", "m2", serde_json::json!({}))
            .await
            .unwrap();

        let result = queue
            .enqueue(Platform::Slack, "c3", "m3", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 300_000);
        assert_eq!(config.exponential_base, 2.0);
        assert!((config.jitter_factor - 0.1).abs() < f64::EPSILON);
        assert_eq!(config.max_queue_size, 10_000);
        assert!(config.dead_letter_enabled);
    }

    #[test]
    fn test_default_queue() {
        let queue = RetryQueue::default();
        assert!(!queue.has_persistence());
    }

    #[tokio::test]
    async fn test_mark_success_nonexistent_returns_error() {
        let queue = RetryQueue::new(test_config());
        let result = queue.mark_success("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mark_failed_nonexistent_returns_error() {
        let queue = RetryQueue::new(test_config());
        let result = queue.mark_failed("nonexistent", "err").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_no_persistence() {
        let queue = RetryQueue::new(test_config());
        assert!(!queue.has_persistence());
        let loaded = queue.load_from_db().await;
        assert_eq!(loaded, 0);
    }
}
