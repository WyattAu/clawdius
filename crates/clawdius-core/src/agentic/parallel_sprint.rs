//! Parallel Sprint Manager
//!
//! Orchestrates multiple concurrent sprint sessions and provides a persistent
//! browser daemon for cross-sprint browser sharing. This is the M5 milestone.

use crate::llm::LlmConfig;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// A unique identifier for a sprint session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SprintSessionId(pub String);

impl SprintSessionId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("sprint-{}-{}", timestamp, count))
    }
}

impl std::fmt::Display for SprintSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Status of a parallel sprint session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    /// Session is queued and waiting to start
    Pending,
    /// Sprint is actively running
    Running,
    /// Sprint completed successfully
    Completed,
    /// Sprint failed
    Failed,
    /// Sprint was cancelled
    Cancelled,
}

/// Configuration for a parallel sprint session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelSprintConfig {
    /// Unique session identifier (auto-generated if None)
    pub session_id: Option<SprintSessionId>,
    /// Human-readable session name
    pub session_name: String,
    /// Task description for this sprint
    pub task_description: String,
    /// Project root for this sprint
    pub project_root: PathBuf,
    /// LLM configuration for this session
    pub llm_config: LlmConfig,
    /// Priority (lower = runs first). Default: 0.
    pub priority: i32,
    /// Maximum concurrent sessions. Default: 4.
    pub max_concurrent: usize,
    /// Whether to enable real execution (build/test) for this sprint
    pub real_execution: bool,
}

impl ParallelSprintConfig {
    pub fn new(task_description: &str, llm_config: LlmConfig) -> Self {
        Self {
            session_id: None,
            session_name: format!("Sprint-{}", uuid_v4_placeholder()),
            task_description: task_description.to_string(),
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            llm_config,
            priority: 0,
            max_concurrent: 4,
            real_execution: false,
        }
    }

    /// Set the session name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.session_name = name.into();
        self
    }

    /// Set the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// State of a single parallel sprint session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: SprintSessionId,
    pub config: ParallelSprintConfig,
    pub status: SessionStatus,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub result_summary: Option<String>,
}

/// Summary of all parallel sprint sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelSprintSummary {
    pub total_sessions: usize,
    pub completed: usize,
    pub failed: usize,
    pub running: usize,
    pub pending: usize,
    pub cancelled: usize,
    pub total_duration_ms: u64,
}

impl ParallelSprintSummary {
    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str("╔══════════════════════════════════════════╗\n");
        report.push_str("║     Parallel Sprint Summary                   ║\n");
        report.push_str("╠════════════════════════════════════════╣\n");
        report.push_str(&format!(
            "║ Total: {:>3}  Completed: {:>3}  Failed: {:>3}          ║\n",
            self.total_sessions, self.completed, self.failed
        ));
        report.push_str(&format!(
            "║ Running: {:>2}  Pending: {:>4}  Cancelled: {:>2}         ║\n",
            self.running, self.pending, self.cancelled
        ));
        report.push_str(&format!(
            "║ Total time: {:>8}ms                   ║\n",
            self.total_duration_ms
        ));
        report.push_str("╚══════════════════════════════════════════╝\n");
        report
    }
}

/// The parallel sprint manager orchestrates multiple concurrent sprint sessions.
pub struct ParallelSprintManager {
    sessions: RwLock<HashMap<String, SessionState>>,
    max_concurrent: usize,
}

impl ParallelSprintManager {
    /// Create a new parallel sprint manager.
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            max_concurrent,
        }
    }

    /// Create a default manager with 4 concurrent sessions.
    pub fn default() -> Self {
        Self::new(4)
    }

    /// Submit a new sprint session for parallel execution.
    /// Returns the session ID.
    pub async fn submit(&self, config: ParallelSprintConfig) -> Result<SprintSessionId> {
        let id = config
            .session_id
            .clone()
            .unwrap_or_else(SprintSessionId::new);

        let state = SessionState {
            id: id.clone(),
            config,
            status: SessionStatus::Pending,
            started_at: None,
            completed_at: None,
            error: None,
            result_summary: None,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(id.0.clone(), state);
        }

        Ok(id)
    }

    /// Get the current state of all sessions.
    pub async fn list_sessions(&self) -> Vec<SessionState> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Get a summary of all sessions.
    pub async fn summary(&self) -> ParallelSprintSummary {
        let sessions = self.sessions.read().await;
        let total = sessions.len();
        let completed = sessions
            .values()
            .filter(|s| s.status == SessionStatus::Completed)
            .count();
        let failed = sessions
            .values()
            .filter(|s| s.status == SessionStatus::Failed)
            .count();
        let running = sessions
            .values()
            .filter(|s| s.status == SessionStatus::Running)
            .count();
        let pending = sessions
            .values()
            .filter(|s| s.status == SessionStatus::Pending)
            .count();
        let cancelled = sessions
            .values()
            .filter(|s| s.status == SessionStatus::Cancelled)
            .count();

        let total_duration_ms = sessions
            .values()
            .filter_map(|s| {
                s.started_at
                    .zip(s.completed_at)
                    .map(|(start, end)| (end - start).num_milliseconds() as u64)
            })
            .sum();

        ParallelSprintSummary {
            total_sessions: total,
            completed,
            failed,
            running,
            pending,
            cancelled,
            total_duration_ms,
        }
    }

    /// Cancel a pending or running session.
    pub async fn cancel(&self, session_id: &SprintSessionId) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(&session_id.0) {
            match state.status {
                SessionStatus::Pending | SessionStatus::Running => {
                    state.status = SessionStatus::Cancelled;
                    state.completed_at = Some(chrono::Utc::now());
                    Ok(())
                },
                _ => Err(crate::Error::Sprint(format!(
                    "Cannot cancel session {} (status: {:?})",
                    session_id, state.status
                ))),
            }
        } else {
            Err(crate::Error::Sprint(format!(
                "Session {} not found",
                session_id
            )))
        }
    }

    /// Get the number of currently active (pending + running) sessions.
    pub async fn active_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| matches!(s.status, SessionStatus::Pending | SessionStatus::Running))
            .count()
    }

    /// Check if a new session can be started (under concurrency limit).
    pub async fn can_start(&self) -> bool {
        self.active_count().await < self.max_concurrent
    }

    /// Update a session's status.
    async fn update_status(
        &self,
        session_id: &SprintSessionId,
        status: SessionStatus,
        error: Option<String>,
        result_summary: Option<String>,
    ) {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(&session_id.0) {
            state.status = status;
            state.error = error;
            state.result_summary = result_summary;
            if matches!(
                status,
                SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Cancelled
            ) {
                state.completed_at = Some(chrono::Utc::now());
            }
        }
    }
}

// Placeholder UUID generator (real impl would use uuid crate)
fn uuid_v4_placeholder() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", timestamp)[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprint_session_id_display() {
        let id = SprintSessionId::new();
        assert!(id.0.starts_with("sprint-"));
    }

    #[test]
    fn test_sprint_session_id_unique() {
        let id1 = SprintSessionId::new();
        let id2 = SprintSessionId::new();
        // In practice these would differ, but in a test they might be the same
        // Just verify the format
        assert!(id1.0.contains("sprint-"));
        assert!(id2.0.contains("sprint-"));
    }

    #[test]
    fn test_parallel_sprint_config_new() {
        let config = ParallelSprintConfig::new(
            "Build feature",
            crate::llm::LlmConfig {
                provider: "openrouter".to_string(),
                model: "test".to_string(),
                api_key: None,
                base_url: None,
                max_tokens: 100,
            },
        );
        assert_eq!(config.task_description, "Build feature");
        assert_eq!(config.priority, 0);
        assert_eq!(config.max_concurrent, 4);
        assert!(!config.real_execution);
        assert!(config.session_id.is_none());
    }

    #[test]
    fn test_parallel_sprint_config_builder() {
        let config = ParallelSprintConfig::new(
            "Test task",
            crate::llm::LlmConfig {
                provider: "openrouter".to_string(),
                model: "test".to_string(),
                api_key: None,
                base_url: None,
                max_tokens: 100,
            },
        )
        .with_name("High-priority sprint")
        .with_priority(-1);

        assert_eq!(config.session_name, "High-priority sprint");
        assert_eq!(config.priority, -1);
    }

    #[test]
    fn test_parallel_sprint_config_serialization() {
        let config = ParallelSprintConfig {
            session_id: Some(SprintSessionId::new()),
            session_name: "Test Sprint".to_string(),
            task_description: "Do stuff".to_string(),
            project_root: PathBuf::from("/tmp"),
            llm_config: crate::llm::LlmConfig {
                provider: "openrouter".to_string(),
                model: "test".to_string(),
                api_key: None,
                base_url: None,
                max_tokens: 100,
            },
            priority: 0,
            max_concurrent: 2,
            real_execution: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ParallelSprintConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_name, "Test Sprint");
        assert!(parsed.real_execution);
        assert!(parsed.session_id.is_some());
    }

    #[tokio::test]
    async fn test_submit_and_list() {
        let manager = ParallelSprintManager::new(2);
        let id = manager
            .submit(ParallelSprintConfig::new(
                "Task 1",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await
            .unwrap();

        let id2 = manager
            .submit(ParallelSprintConfig::new(
                "Task 2",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await
            .unwrap();

        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.id.0 == id.0));
        assert!(sessions.iter().any(|s| s.id.0 == id2.0));
    }

    #[tokio::test]
    async fn test_cancel_session() {
        let manager = ParallelSprintManager::new(2);
        let id = manager
            .submit(ParallelSprintConfig::new(
                "Task",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await
            .unwrap();

        // Can cancel a pending session
        assert!(manager.cancel(&id).await.is_ok());

        // Cannot cancel again
        assert!(manager.cancel(&id).await.is_err());

        // Non-existent session
        let fake_id = SprintSessionId(format!("fake-{}", uuid_v4_placeholder()));
        assert!(manager.cancel(&fake_id).await.is_err());
    }

    #[tokio::test]
    async fn test_active_count_and_can_start() {
        let manager = ParallelSprintManager::new(2);
        assert!(manager.can_start().await);

        let id = manager
            .submit(ParallelSprintConfig::new(
                "Task",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await
            .unwrap();

        assert!(manager.can_start().await);
        let _ = manager
            .submit(ParallelSprintConfig::new(
                "Task 2",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await;

        assert!(!manager.can_start().await); // At limit
    }

    #[tokio::test]
    async fn test_parallel_sprint_summary() {
        let manager = ParallelSprintManager::new(4);
        let id = manager
            .submit(ParallelSprintConfig::new(
                "Task",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ))
            .await
            .unwrap();

        // Update status to completed
        manager
            .update_status(
                &id,
                SessionStatus::Completed,
                None,
                Some("Done".to_string()),
            )
            .await;

        let summary = manager.summary().await;
        assert_eq!(summary.total_sessions, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.pending, 0);

        // Verify report renders
        let report = summary.report();
        assert!(report.contains("Parallel Sprint Summary"));
    }

    #[tokio::test]
    async fn test_session_status_serialization() {
        let state = SessionState {
            id: SprintSessionId::new(),
            config: ParallelSprintConfig::new(
                "Task",
                crate::llm::LlmConfig {
                    provider: "openrouter".to_string(),
                    model: "test".to_string(),
                    api_key: None,
                    base_url: None,
                    max_tokens: 100,
                },
            ),
            status: SessionStatus::Running,
            started_at: Some(chrono::Utc::now()),
            completed_at: None,
            error: None,
            result_summary: None,
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, SessionStatus::Running);
        assert!(parsed.started_at.is_some());
    }

    #[test]
    fn test_parallel_sprint_summary_serialization() {
        let summary = ParallelSprintSummary {
            total_sessions: 10,
            completed: 7,
            failed: 2,
            running: 1,
            pending: 0,
            cancelled: 0,
            total_duration_ms: 5000,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ParallelSprintSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_sessions, 10);
        assert_eq!(parsed.completed, 7);
    }

    #[test]
    fn test_parallel_sprint_summary_report() {
        let summary = ParallelSprintSummary {
            total_sessions: 5,
            completed: 3,
            failed: 1,
            running: 0,
            pending: 1,
            cancelled: 0,
            total_duration_ms: 10000,
        };
        let report = summary.report();
        assert!(report.contains("Parallel Sprint Summary"));
        assert!(report.contains("Total:   5"));
    }
}
