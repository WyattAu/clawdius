//! Event bus for Nexus FSM
//!
//! This module implements an async event bus for publishing and subscribing to
//! FSM events. Events are used for notifications, audit trails, and integration
//! with external systems.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use super::{ArtifactId, PhaseId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    PhaseStarted,
    PhaseCompleted,
    PhaseTransitioned,
    GateEvaluated,
    GatesCompleted,
    ArtifactCreated,
    ArtifactModified,
    ArtifactDeleted,
    ErrorOccurred,
    ProjectInitialized,
    ProjectFinalized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NexusEvent {
    PhaseStarted {
        phase: PhaseId,
        timestamp: DateTime<Utc>,
    },
    PhaseCompleted {
        phase: PhaseId,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    PhaseTransitioned {
        from: PhaseId,
        to: PhaseId,
        timestamp: DateTime<Utc>,
    },

    GateEvaluated {
        gate_id: String,
        phase: PhaseId,
        passed: bool,
        timestamp: DateTime<Utc>,
    },
    GatesCompleted {
        phase: PhaseId,
        all_passed: bool,
        failed_count: u32,
        timestamp: DateTime<Utc>,
    },

    ArtifactCreated {
        id: ArtifactId,
        artifact_type: String,
        phase: PhaseId,
        timestamp: DateTime<Utc>,
    },
    ArtifactModified {
        id: ArtifactId,
        timestamp: DateTime<Utc>,
    },
    ArtifactDeleted {
        id: ArtifactId,
        timestamp: DateTime<Utc>,
    },

    ErrorOccurred {
        error: String,
        phase: Option<PhaseId>,
        timestamp: DateTime<Utc>,
    },

    ProjectInitialized {
        project_root: String,
        timestamp: DateTime<Utc>,
    },

    ProjectFinalized {
        timestamp: DateTime<Utc>,
    },
}

impl NexusEvent {
    #[must_use]
    pub fn phase_started(phase: PhaseId) -> Self {
        NexusEvent::PhaseStarted {
            phase,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn phase_completed(phase: PhaseId, duration_ms: u64) -> Self {
        NexusEvent::PhaseCompleted {
            phase,
            duration_ms,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn phase_transitioned(from: PhaseId, to: PhaseId) -> Self {
        NexusEvent::PhaseTransitioned {
            from,
            to,
            timestamp: Utc::now(),
        }
    }

    pub fn gate_evaluated(gate_id: impl Into<String>, phase: PhaseId, passed: bool) -> Self {
        NexusEvent::GateEvaluated {
            gate_id: gate_id.into(),
            phase,
            passed,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn gates_completed(phase: PhaseId, all_passed: bool, failed_count: u32) -> Self {
        NexusEvent::GatesCompleted {
            phase,
            all_passed,
            failed_count,
            timestamp: Utc::now(),
        }
    }

    pub fn artifact_created(
        id: ArtifactId,
        artifact_type: impl Into<String>,
        phase: PhaseId,
    ) -> Self {
        NexusEvent::ArtifactCreated {
            id,
            artifact_type: artifact_type.into(),
            phase,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn artifact_modified(id: ArtifactId) -> Self {
        NexusEvent::ArtifactModified {
            id,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn artifact_deleted(id: ArtifactId) -> Self {
        NexusEvent::ArtifactDeleted {
            id,
            timestamp: Utc::now(),
        }
    }

    pub fn error(error: impl Into<String>, phase: Option<PhaseId>) -> Self {
        NexusEvent::ErrorOccurred {
            error: error.into(),
            phase,
            timestamp: Utc::now(),
        }
    }

    pub fn project_initialized(project_root: impl Into<String>) -> Self {
        NexusEvent::ProjectInitialized {
            project_root: project_root.into(),
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn project_finalized() -> Self {
        NexusEvent::ProjectFinalized {
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            NexusEvent::PhaseStarted { timestamp, .. } => timestamp,
            NexusEvent::PhaseCompleted { timestamp, .. } => timestamp,
            NexusEvent::PhaseTransitioned { timestamp, .. } => timestamp,
            NexusEvent::GateEvaluated { timestamp, .. } => timestamp,
            NexusEvent::GatesCompleted { timestamp, .. } => timestamp,
            NexusEvent::ArtifactCreated { timestamp, .. } => timestamp,
            NexusEvent::ArtifactModified { timestamp, .. } => timestamp,
            NexusEvent::ArtifactDeleted { timestamp, .. } => timestamp,
            NexusEvent::ErrorOccurred { timestamp, .. } => timestamp,
            NexusEvent::ProjectInitialized { timestamp, .. } => timestamp,
            NexusEvent::ProjectFinalized { timestamp } => timestamp,
        }
    }

    #[must_use]
    pub fn event_type(&self) -> EventType {
        match self {
            NexusEvent::PhaseStarted { .. } => EventType::PhaseStarted,
            NexusEvent::PhaseCompleted { .. } => EventType::PhaseCompleted,
            NexusEvent::PhaseTransitioned { .. } => EventType::PhaseTransitioned,
            NexusEvent::GateEvaluated { .. } => EventType::GateEvaluated,
            NexusEvent::GatesCompleted { .. } => EventType::GatesCompleted,
            NexusEvent::ArtifactCreated { .. } => EventType::ArtifactCreated,
            NexusEvent::ArtifactModified { .. } => EventType::ArtifactModified,
            NexusEvent::ArtifactDeleted { .. } => EventType::ArtifactDeleted,
            NexusEvent::ErrorOccurred { .. } => EventType::ErrorOccurred,
            NexusEvent::ProjectInitialized { .. } => EventType::ProjectInitialized,
            NexusEvent::ProjectFinalized { .. } => EventType::ProjectFinalized,
        }
    }

    #[must_use]
    pub fn phase(&self) -> Option<PhaseId> {
        match self {
            NexusEvent::PhaseStarted { phase, .. } => Some(*phase),
            NexusEvent::PhaseCompleted { phase, .. } => Some(*phase),
            NexusEvent::PhaseTransitioned { to, .. } => Some(*to),
            NexusEvent::GateEvaluated { phase, .. } => Some(*phase),
            NexusEvent::GatesCompleted { phase, .. } => Some(*phase),
            NexusEvent::ArtifactCreated { phase, .. } => Some(*phase),
            NexusEvent::ErrorOccurred { phase, .. } => *phase,
            _ => None,
        }
    }
}

pub trait EventHandler: Send + Sync + std::fmt::Debug {
    fn handle(&self, event: &NexusEvent) -> std::result::Result<(), EventHandlerError>;
    fn interests(&self) -> Vec<EventType> {
        vec![]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventHandlerError {
    #[error("Handler execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Handler timeout")]
    Timeout,

    #[error("Handler not registered")]
    NotRegistered,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

const METRICS_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_type TEXT NOT NULL,
    value INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_metrics_type ON metrics(metric_type);

INSERT OR IGNORE INTO metrics (metric_type, value, updated_at) VALUES 
    ('phase_transitions', 0, datetime('now')),
    ('artifacts_created', 0, datetime('now')),
    ('gates_passed', 0, datetime('now')),
    ('gates_failed', 0, datetime('now'));
";

const AUDIT_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_event_type ON audit_log(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
";

#[derive(Debug)]
pub struct MetricsStorage {
    conn: Mutex<Connection>,
}

impl MetricsStorage {
    pub fn new(db_path: PathBuf) -> std::result::Result<Self, EventHandlerError> {
        let conn = Self::create_connection(&db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.initialize_schema()?;
        Ok(storage)
    }

    #[must_use]
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory connection");
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage
            .initialize_schema()
            .expect("Failed to initialize in-memory metrics schema");
        storage
    }

    fn create_connection(db_path: &PathBuf) -> std::result::Result<Connection, EventHandlerError> {
        let conn = if db_path.to_string_lossy() == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(db_path)
        }
        .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        Ok(conn)
    }

    fn get_connection(
        &self,
    ) -> std::result::Result<std::sync::MutexGuard<'_, Connection>, EventHandlerError> {
        self.conn
            .lock()
            .map_err(|e| EventHandlerError::DatabaseError(format!("Lock error: {e}")))
    }

    fn initialize_schema(&self) -> std::result::Result<(), EventHandlerError> {
        let conn = self.get_connection()?;
        conn.execute_batch(METRICS_SCHEMA_SQL)
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn increment(&self, metric_type: &str) -> std::result::Result<(), EventHandlerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE metrics SET value = value + 1, updated_at = datetime('now') WHERE metric_type = ?1",
            params![metric_type],
        )
        .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get(&self, metric_type: &str) -> std::result::Result<u64, EventHandlerError> {
        let conn = self.get_connection()?;
        let value: i64 = conn
            .query_row(
                "SELECT value FROM metrics WHERE metric_type = ?1",
                params![metric_type],
                |row| row.get(0),
            )
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(value as u64)
    }

    pub fn get_all(&self) -> std::result::Result<MetricsSnapshot, EventHandlerError> {
        Ok(MetricsSnapshot {
            phase_transitions: self.get("phase_transitions")?,
            artifacts_created: self.get("artifacts_created")?,
            gates_passed: self.get("gates_passed")?,
            gates_failed: self.get("gates_failed")?,
        })
    }
}

impl Clone for MetricsStorage {
    fn clone(&self) -> Self {
        panic!("MetricsStorage cannot be cloned - use Arc<MetricsStorage> instead");
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub phase_transitions: u64,
    pub artifacts_created: u64,
    pub gates_passed: u64,
    pub gates_failed: u64,
}

#[derive(Debug)]
pub struct AuditStorage {
    conn: Mutex<Connection>,
}

impl AuditStorage {
    pub fn new(db_path: PathBuf) -> std::result::Result<Self, EventHandlerError> {
        let conn = Self::create_connection(&db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.initialize_schema()?;
        Ok(storage)
    }

    #[must_use]
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory connection");
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage
            .initialize_schema()
            .expect("Failed to initialize in-memory audit schema");
        storage
    }

    fn create_connection(db_path: &PathBuf) -> std::result::Result<Connection, EventHandlerError> {
        let conn = if db_path.to_string_lossy() == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(db_path)
        }
        .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        Ok(conn)
    }

    fn get_connection(
        &self,
    ) -> std::result::Result<std::sync::MutexGuard<'_, Connection>, EventHandlerError> {
        self.conn
            .lock()
            .map_err(|e| EventHandlerError::DatabaseError(format!("Lock error: {e}")))
    }

    fn initialize_schema(&self) -> std::result::Result<(), EventHandlerError> {
        let conn = self.get_connection()?;
        conn.execute_batch(AUDIT_SCHEMA_SQL)
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn record(&self, event: &NexusEvent) -> std::result::Result<(), EventHandlerError> {
        let conn = self.get_connection()?;
        let event_type = event.event_type();
        let event_data = serde_json::to_string(event)
            .map_err(|e| EventHandlerError::ExecutionFailed(e.to_string()))?;
        let timestamp = event.timestamp().to_rfc3339();

        conn.execute(
            "INSERT INTO audit_log (event_type, event_data, timestamp) VALUES (?1, ?2, ?3)",
            params![format!("{:?}", event_type), event_data, timestamp],
        )
        .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get_records(&self, limit: usize) -> std::result::Result<Vec<String>, EventHandlerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT event_data FROM audit_log ORDER BY id DESC LIMIT ?1")
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        let records = stmt
            .query_map(params![limit as i64], |row| row.get(0))
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        Ok(records)
    }

    pub fn get_records_for_phase(
        &self,
        phase: PhaseId,
        limit: usize,
    ) -> std::result::Result<Vec<String>, EventHandlerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT event_data FROM audit_log WHERE event_type IN \
                 ('PhaseStarted', 'PhaseCompleted', 'PhaseTransitioned', 'GateEvaluated', \
                  'GatesCompleted', 'ArtifactCreated', 'ErrorOccurred') \
                 ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        let records = stmt
            .query_map(params![limit as i64], |row| row.get(0))
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;

        Ok(records
            .into_iter()
            .filter(|r| {
                if let Ok(event) = serde_json::from_str::<NexusEvent>(r) {
                    event.phase() == Some(phase)
                } else {
                    false
                }
            })
            .collect())
    }

    pub fn count(&self) -> std::result::Result<usize, EventHandlerError> {
        let conn = self.get_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        Ok(count as usize)
    }
}

pub struct EventBus {
    subscribers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    history: Arc<RwLock<Vec<NexusEvent>>>,
    max_history: usize,
}

impl EventBus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history: 1000,
        }
    }

    #[must_use]
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history,
        }
    }

    pub async fn subscribe(&self, handler: Box<dyn EventHandler>) {
        self.subscribers.write().await.push(handler);
    }

    pub fn subscribe_sync(&self, handler: Box<dyn EventHandler>) {
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                handle.block_on(async {
                    self.subscribers.write().await.push(handler);
                });
            },
            Err(_) => {
                tracing::warn!("No Tokio runtime available for event subscription");
            },
        }
    }

    pub async fn unsubscribe_all(&self) {
        self.subscribers.write().await.clear();
    }

    pub async fn publish(&self, event: NexusEvent) {
        let mut history = self.history.write().await;

        if history.len() >= self.max_history {
            history.remove(0);
        }
        history.push(event.clone());
        drop(history);

        let subscribers = self.subscribers.read().await;
        for handler in subscribers.iter() {
            let interests = handler.interests();
            if interests.is_empty() || interests.contains(&event.event_type()) {
                if let Err(e) = handler.handle(&event) {
                    tracing::error!("Event handler error: {:?}", e);
                }
            }
        }
    }

    pub fn publish_sync(&self, event: NexusEvent) {
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                handle.block_on(async {
                    self.publish(event).await;
                });
            },
            Err(_) => {
                tracing::warn!("No Tokio runtime available for event publishing");
            },
        }
    }

    pub async fn history(&self) -> Vec<NexusEvent> {
        self.history.read().await.clone()
    }

    pub async fn history_for_phase(&self, phase: PhaseId) -> Vec<NexusEvent> {
        self.history
            .read()
            .await
            .iter()
            .filter(|e| e.phase() == Some(phase))
            .cloned()
            .collect()
    }

    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("max_history", &self.max_history)
            .finish()
    }
}

#[derive(Debug)]
pub struct LoggingHandler;

impl EventHandler for LoggingHandler {
    fn handle(&self, event: &NexusEvent) -> std::result::Result<(), EventHandlerError> {
        match event {
            NexusEvent::PhaseTransitioned { from, to, .. } => {
                tracing::info!("[Nexus] Phase transition: {} -> {}", from.0, to.0);
            },
            NexusEvent::ArtifactCreated {
                id, artifact_type, ..
            } => {
                tracing::info!(
                    "[Nexus] Artifact created: {} (type: {})",
                    id.0,
                    artifact_type
                );
            },
            NexusEvent::ErrorOccurred { error, phase, .. } => {
                tracing::error!("[Nexus] Error: {} (phase: {:?})", error, phase);
            },
            NexusEvent::GateEvaluated {
                gate_id,
                passed,
                phase,
                ..
            } => {
                tracing::debug!(
                    "[Nexus] Gate '{}' evaluated for phase {}: {}",
                    gate_id,
                    phase.0,
                    if *passed { "PASSED" } else { "FAILED" }
                );
            },
            _ => {},
        }
        Ok(())
    }

    fn interests(&self) -> Vec<EventType> {
        vec![
            EventType::PhaseTransitioned,
            EventType::ArtifactCreated,
            EventType::ErrorOccurred,
            EventType::GateEvaluated,
        ]
    }
}

impl Default for MetricsHandler {
    fn default() -> Self {
        Self {
            phase_transitions: std::sync::atomic::AtomicU64::new(0),
            artifacts_created: std::sync::atomic::AtomicU64::new(0),
            gates_passed: std::sync::atomic::AtomicU64::new(0),
            gates_failed: std::sync::atomic::AtomicU64::new(0),
            storage: None,
        }
    }
}

#[derive(Debug)]
pub struct MetricsHandler {
    phase_transitions: std::sync::atomic::AtomicU64,
    artifacts_created: std::sync::atomic::AtomicU64,
    gates_passed: std::sync::atomic::AtomicU64,
    gates_failed: std::sync::atomic::AtomicU64,
    storage: Option<MetricsStorage>,
}

impl MetricsHandler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_storage(db_path: PathBuf) -> std::result::Result<Self, EventHandlerError> {
        Ok(Self {
            phase_transitions: std::sync::atomic::AtomicU64::new(0),
            artifacts_created: std::sync::atomic::AtomicU64::new(0),
            gates_passed: std::sync::atomic::AtomicU64::new(0),
            gates_failed: std::sync::atomic::AtomicU64::new(0),
            storage: Some(MetricsStorage::new(db_path)?),
        })
    }

    pub fn phase_transitions(&self) -> u64 {
        self.phase_transitions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn artifacts_created(&self) -> u64 {
        self.artifacts_created
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn gates_passed(&self) -> u64 {
        self.gates_passed.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn gates_failed(&self) -> u64 {
        self.gates_failed.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            phase_transitions: self.phase_transitions(),
            artifacts_created: self.artifacts_created(),
            gates_passed: self.gates_passed(),
            gates_failed: self.gates_failed(),
        }
    }

    pub fn persist(&self) -> std::result::Result<(), EventHandlerError> {
        if let Some(ref storage) = self.storage {
            let snapshot = self.snapshot();
            let conn = storage.get_connection()?;
            conn.execute(
                "UPDATE metrics SET value = ?1, updated_at = datetime('now') WHERE metric_type = 'phase_transitions'",
                params![snapshot.phase_transitions as i64],
            ).map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE metrics SET value = ?1, updated_at = datetime('now') WHERE metric_type = 'artifacts_created'",
                params![snapshot.artifacts_created as i64],
            ).map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE metrics SET value = ?1, updated_at = datetime('now') WHERE metric_type = 'gates_passed'",
                params![snapshot.gates_passed as i64],
            ).map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE metrics SET value = ?1, updated_at = datetime('now') WHERE metric_type = 'gates_failed'",
                params![snapshot.gates_failed as i64],
            ).map_err(|e| EventHandlerError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }
}

impl EventHandler for MetricsHandler {
    fn handle(&self, event: &NexusEvent) -> std::result::Result<(), EventHandlerError> {
        match event {
            NexusEvent::PhaseTransitioned { .. } => {
                self.phase_transitions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
            NexusEvent::ArtifactCreated { .. } => {
                self.artifacts_created
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
            NexusEvent::GateEvaluated { passed, .. } => {
                if *passed {
                    self.gates_passed
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                } else {
                    self.gates_failed
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            },
            _ => {},
        }
        Ok(())
    }

    fn interests(&self) -> Vec<EventType> {
        vec![
            EventType::PhaseTransitioned,
            EventType::ArtifactCreated,
            EventType::GateEvaluated,
        ]
    }
}

#[derive(Debug)]
pub struct AuditHandler {
    records: Arc<RwLock<Vec<String>>>,
    storage: Option<Arc<Mutex<AuditStorage>>>,
}

impl Default for AuditHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            storage: None,
        }
    }

    pub fn with_storage(db_path: PathBuf) -> std::result::Result<Self, EventHandlerError> {
        Ok(Self {
            records: Arc::new(RwLock::new(Vec::new())),
            storage: Some(Arc::new(Mutex::new(AuditStorage::new(db_path)?))),
        })
    }

    pub async fn records(&self) -> Vec<String> {
        self.records.read().await.clone()
    }

    pub fn records_from_storage(
        &self,
        limit: usize,
    ) -> std::result::Result<Vec<String>, EventHandlerError> {
        if let Some(ref storage) = self.storage {
            let storage = storage
                .lock()
                .map_err(|e| EventHandlerError::DatabaseError(format!("Lock error: {e}")))?;
            storage.get_records(limit)
        } else {
            Ok(vec![])
        }
    }

    pub fn count_in_storage(&self) -> std::result::Result<usize, EventHandlerError> {
        if let Some(ref storage) = self.storage {
            let storage = storage
                .lock()
                .map_err(|e| EventHandlerError::DatabaseError(format!("Lock error: {e}")))?;
            storage.count()
        } else {
            Ok(0)
        }
    }
}

impl EventHandler for AuditHandler {
    fn handle(&self, event: &NexusEvent) -> std::result::Result<(), EventHandlerError> {
        let record = serde_json::to_string(event)
            .map_err(|e| EventHandlerError::ExecutionFailed(e.to_string()))?;

        if let Some(ref storage) = self.storage {
            let storage = storage
                .lock()
                .map_err(|e| EventHandlerError::DatabaseError(format!("Lock error: {e}")))?;
            storage.record(event)?;
        }

        let records = self.records.clone();
        let record = record.clone();
        tokio::spawn(async move {
            records.write().await.push(record);
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = NexusEvent::phase_started(PhaseId(0));
        assert!(matches!(event, NexusEvent::PhaseStarted { .. }));
        assert_eq!(event.event_type(), EventType::PhaseStarted);
        assert_eq!(event.phase(), Some(PhaseId(0)));
    }

    #[test]
    fn test_phase_transition_event() {
        let event = NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1));

        if let NexusEvent::PhaseTransitioned { from, to, .. } = event {
            assert_eq!(from, PhaseId(0));
            assert_eq!(to, PhaseId(1));
        } else {
            panic!("Wrong event type");
        }
        assert_eq!(event.event_type(), EventType::PhaseTransitioned);
        assert_eq!(event.phase(), Some(PhaseId(1)));
    }

    #[test]
    fn test_artifact_created_event() {
        let id = ArtifactId::new("test-artifact");
        let event = NexusEvent::artifact_created(id.clone(), "Documentation", PhaseId(5));

        if let NexusEvent::ArtifactCreated {
            id: event_id,
            artifact_type,
            phase,
            ..
        } = event
        {
            assert_eq!(event_id, id);
            assert_eq!(artifact_type, "Documentation");
            assert_eq!(phase, PhaseId(5));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_error_event() {
        let event = NexusEvent::error("Test error", Some(PhaseId(5)));

        if let NexusEvent::ErrorOccurred { error, phase, .. } = event {
            assert_eq!(error, "Test error");
            assert_eq!(phase, Some(PhaseId(5)));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_gates_completed_event() {
        let event = NexusEvent::gates_completed(PhaseId(3), true, 0);
        if let NexusEvent::GatesCompleted {
            phase,
            all_passed,
            failed_count,
            ..
        } = event
        {
            assert_eq!(phase, PhaseId(3));
            assert!(all_passed);
            assert_eq!(failed_count, 0);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_event_bus_creation() {
        let bus = EventBus::new();
        assert_eq!(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(bus.subscriber_count()),
            0
        );
    }

    #[tokio::test]
    async fn test_event_bus_subscribe() {
        let bus = EventBus::new();
        bus.subscribe(Box::new(LoggingHandler)).await;

        assert_eq!(bus.subscriber_count().await, 1);
    }

    #[tokio::test]
    async fn test_event_bus_history() {
        let bus = EventBus::new();

        bus.publish(NexusEvent::phase_started(PhaseId(0))).await;
        bus.publish(NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)))
            .await;

        let history = bus.history().await;
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_event_bus_max_history() {
        let bus = EventBus::with_max_history(5);

        for i in 0..10 {
            bus.publish(NexusEvent::phase_started(PhaseId(i))).await;
        }

        let history = bus.history().await;
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_event_bus_history_for_phase() {
        let bus = EventBus::new();

        bus.publish(NexusEvent::phase_started(PhaseId(0))).await;
        bus.publish(NexusEvent::phase_started(PhaseId(1))).await;
        bus.publish(NexusEvent::phase_started(PhaseId(0))).await;

        let history = bus.history_for_phase(PhaseId(0)).await;
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_logging_handler() {
        let handler = LoggingHandler;
        let event = NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1));

        assert!(handler.handle(&event).is_ok());
        assert!(!handler.interests().is_empty());
    }

    #[test]
    fn test_metrics_handler() {
        let handler = MetricsHandler::new();

        handler
            .handle(&NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)))
            .unwrap();
        assert_eq!(handler.phase_transitions(), 1);

        handler
            .handle(&NexusEvent::artifact_created(
                ArtifactId::new("test"),
                "Doc",
                PhaseId(0),
            ))
            .unwrap();
        assert_eq!(handler.artifacts_created(), 1);

        handler
            .handle(&NexusEvent::gate_evaluated("gate1", PhaseId(0), true))
            .unwrap();
        assert_eq!(handler.gates_passed(), 1);

        handler
            .handle(&NexusEvent::gate_evaluated("gate2", PhaseId(0), false))
            .unwrap();
        assert_eq!(handler.gates_failed(), 1);
    }

    #[tokio::test]
    async fn test_audit_handler() {
        let handler = AuditHandler::new();
        let event = NexusEvent::phase_started(PhaseId(0));

        handler.handle(&event).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let records = handler.records().await;
        assert_eq!(records.len(), 1);
    }

    #[tokio::test]
    async fn test_event_bus_unsubscribe() {
        let bus = EventBus::new();
        bus.subscribe(Box::new(LoggingHandler)).await;
        assert_eq!(bus.subscriber_count().await, 1);

        bus.unsubscribe_all().await;
        assert_eq!(bus.subscriber_count().await, 0);
    }

    #[test]
    fn test_metrics_storage_in_memory() {
        let storage = MetricsStorage::in_memory();

        assert_eq!(storage.get("phase_transitions").unwrap(), 0);
        storage.increment("phase_transitions").unwrap();
        assert_eq!(storage.get("phase_transitions").unwrap(), 1);

        storage.increment("artifacts_created").unwrap();
        storage.increment("artifacts_created").unwrap();
        assert_eq!(storage.get("artifacts_created").unwrap(), 2);
    }

    #[test]
    fn test_metrics_storage_snapshot() {
        let storage = MetricsStorage::in_memory();

        storage.increment("phase_transitions").unwrap();
        storage.increment("gates_passed").unwrap();
        storage.increment("gates_passed").unwrap();
        storage.increment("gates_failed").unwrap();

        let snapshot = storage.get_all().unwrap();
        assert_eq!(snapshot.phase_transitions, 1);
        assert_eq!(snapshot.gates_passed, 2);
        assert_eq!(snapshot.gates_failed, 1);
        assert_eq!(snapshot.artifacts_created, 0);
    }

    #[test]
    fn test_metrics_handler_snapshot() {
        let handler = MetricsHandler::new();

        handler
            .handle(&NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1)))
            .unwrap();
        handler
            .handle(&NexusEvent::phase_transitioned(PhaseId(1), PhaseId(2)))
            .unwrap();
        handler
            .handle(&NexusEvent::artifact_created(
                ArtifactId::new("test"),
                "Doc",
                PhaseId(2),
            ))
            .unwrap();
        handler
            .handle(&NexusEvent::gate_evaluated("gate1", PhaseId(2), true))
            .unwrap();
        handler
            .handle(&NexusEvent::gate_evaluated("gate2", PhaseId(2), false))
            .unwrap();

        let snapshot = handler.snapshot();
        assert_eq!(snapshot.phase_transitions, 2);
        assert_eq!(snapshot.artifacts_created, 1);
        assert_eq!(snapshot.gates_passed, 1);
        assert_eq!(snapshot.gates_failed, 1);
    }

    #[test]
    fn test_audit_storage_in_memory() {
        let storage = AuditStorage::in_memory();

        let event = NexusEvent::phase_started(PhaseId(0));
        storage.record(&event).unwrap();

        let event2 = NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1));
        storage.record(&event2).unwrap();

        assert_eq!(storage.count().unwrap(), 2);

        let records = storage.get_records(10).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_audit_storage_limit() {
        let storage = AuditStorage::in_memory();

        for i in 0..10 {
            storage
                .record(&NexusEvent::phase_started(PhaseId(i)))
                .unwrap();
        }

        assert_eq!(storage.count().unwrap(), 10);

        let records = storage.get_records(5).unwrap();
        assert_eq!(records.len(), 5);
    }

    #[tokio::test]
    async fn test_audit_handler_with_storage() {
        let handler = AuditHandler::with_storage(PathBuf::from(":memory:")).unwrap();

        let event = NexusEvent::phase_started(PhaseId(0));
        handler.handle(&event).unwrap();

        let event2 = NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1));
        handler.handle(&event2).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert_eq!(handler.count_in_storage().unwrap(), 2);

        let records = handler.records_from_storage(10).unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_audit_handler_without_storage() {
        let handler = AuditHandler::new();

        let event = NexusEvent::phase_started(PhaseId(0));
        handler.handle(&event).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert_eq!(handler.count_in_storage().unwrap(), 0);
        assert_eq!(handler.records_from_storage(10).unwrap().len(), 0);
    }
}
