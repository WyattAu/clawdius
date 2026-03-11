//! State Persistence for Nexus FSM Phase 3
//!
//! This module implements SQLite-based state persistence with crash recovery support.
//! It enables saving and restoring FSM state for durability and recovery.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use super::{NexusError, PhaseId, Result};

const PERSISTENCE_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS fsm_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE,
    current_phase INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_fsm_state_session ON fsm_state(session_id);
CREATE INDEX IF NOT EXISTS idx_fsm_state_status ON fsm_state(status);

CREATE TABLE IF NOT EXISTS phase_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    phase INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,
    artifacts_produced TEXT NOT NULL DEFAULT '[]',
    gates_passed TEXT NOT NULL DEFAULT '[]',
    gates_failed TEXT NOT NULL DEFAULT '[]',
    metadata TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (session_id) REFERENCES fsm_state(session_id) ON DELETE CASCADE,
    UNIQUE(session_id, phase)
);

CREATE INDEX IF NOT EXISTS idx_phase_state_session ON phase_state(session_id);
CREATE INDEX IF NOT EXISTS idx_phase_state_phase ON phase_state(phase);

CREATE TABLE IF NOT EXISTS state_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    phase INTEGER NOT NULL,
    snapshot_type TEXT NOT NULL,
    data TEXT NOT NULL,
    checksum TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES fsm_state(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_snapshots_session ON state_snapshots(session_id);
CREATE INDEX IF NOT EXISTS idx_snapshots_snapshot_id ON state_snapshots(snapshot_id);

CREATE TABLE IF NOT EXISTS recovery_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES fsm_state(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_recovery_session ON recovery_log(session_id);
CREATE INDEX IF NOT EXISTS idx_recovery_timestamp ON recovery_log(timestamp);

CREATE TABLE IF NOT EXISTS checkpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    checkpoint_id TEXT NOT NULL UNIQUE,
    phase INTEGER NOT NULL,
    artifact_ids TEXT NOT NULL DEFAULT '[]',
    event_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES fsm_state(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_session ON checkpoints(session_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_phase ON checkpoints(phase);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Paused,
    Completed,
    Failed,
    Recovering,
    Archived,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Active => write!(f, "active"),
            SessionStatus::Paused => write!(f, "paused"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Failed => write!(f, "failed"),
            SessionStatus::Recovering => write!(f, "recovering"),
            SessionStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = NexusError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "active" => Ok(SessionStatus::Active),
            "paused" => Ok(SessionStatus::Paused),
            "completed" => Ok(SessionStatus::Completed),
            "failed" => Ok(SessionStatus::Failed),
            "recovering" => Ok(SessionStatus::Recovering),
            "archived" => Ok(SessionStatus::Archived),
            _ => Err(NexusError::LockError(format!(
                "Invalid session status: {s}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for PhaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhaseStatus::Pending => write!(f, "pending"),
            PhaseStatus::InProgress => write!(f, "in_progress"),
            PhaseStatus::Completed => write!(f, "completed"),
            PhaseStatus::Failed => write!(f, "failed"),
            PhaseStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for PhaseStatus {
    type Err = NexusError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(PhaseStatus::Pending),
            "in_progress" => Ok(PhaseStatus::InProgress),
            "completed" => Ok(PhaseStatus::Completed),
            "failed" => Ok(PhaseStatus::Failed),
            "skipped" => Ok(PhaseStatus::Skipped),
            _ => Err(NexusError::LockError(format!("Invalid phase status: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        SessionId(id.into())
    }

    #[must_use]
    pub fn generate() -> Self {
        SessionId(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

impl SnapshotId {
    #[must_use]
    pub fn generate() -> Self {
        SnapshotId(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    #[must_use]
    pub fn generate() -> Self {
        CheckpointId(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmState {
    pub session_id: SessionId,
    pub current_phase: PhaseId,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl FsmState {
    #[must_use]
    pub fn new(session_id: SessionId, current_phase: PhaseId) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            current_phase,
            status: SessionStatus::Active,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStateRecord {
    pub session_id: SessionId,
    pub phase: PhaseId,
    pub status: PhaseStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub artifacts_produced: Vec<String>,
    pub gates_passed: Vec<String>,
    pub gates_failed: Vec<String>,
    pub metadata: serde_json::Value,
}

impl PhaseStateRecord {
    #[must_use]
    pub fn new(session_id: SessionId, phase: PhaseId) -> Self {
        Self {
            session_id,
            phase,
            status: PhaseStatus::Pending,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            artifacts_produced: Vec::new(),
            gates_passed: Vec::new(),
            gates_failed: Vec::new(),
            metadata: serde_json::json!({}),
        }
    }

    pub fn start(&mut self) {
        self.status = PhaseStatus::InProgress;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self, artifacts: Vec<String>, passed: Vec<String>, failed: Vec<String>) {
        self.status = PhaseStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.artifacts_produced = artifacts;
        self.gates_passed = passed;
        self.gates_failed = failed;
        if let Some(started) = self.started_at {
            self.duration_ms = Some((Utc::now() - started).num_milliseconds() as u64);
        }
    }

    pub fn fail(&mut self) {
        self.status = PhaseStatus::Failed;
        self.completed_at = Some(Utc::now());
        if let Some(started) = self.started_at {
            self.duration_ms = Some((Utc::now() - started).num_milliseconds() as u64);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub snapshot_id: SnapshotId,
    pub session_id: SessionId,
    pub phase: PhaseId,
    pub snapshot_type: SnapshotType,
    pub data: serde_json::Value,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SnapshotType {
    Full,
    Incremental,
    Checkpoint,
    Recovery,
}

impl std::fmt::Display for SnapshotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotType::Full => write!(f, "full"),
            SnapshotType::Incremental => write!(f, "incremental"),
            SnapshotType::Checkpoint => write!(f, "checkpoint"),
            SnapshotType::Recovery => write!(f, "recovery"),
        }
    }
}

impl std::str::FromStr for SnapshotType {
    type Err = NexusError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "full" => Ok(SnapshotType::Full),
            "incremental" => Ok(SnapshotType::Incremental),
            "checkpoint" => Ok(SnapshotType::Checkpoint),
            "recovery" => Ok(SnapshotType::Recovery),
            _ => Err(NexusError::LockError(format!("Invalid snapshot type: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryEvent {
    pub session_id: SessionId,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl RecoveryEvent {
    pub fn new(
        session_id: SessionId,
        event_type: impl Into<String>,
        event_data: serde_json::Value,
    ) -> Self {
        Self {
            session_id,
            event_type: event_type.into(),
            event_data,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub checkpoint_id: CheckpointId,
    pub session_id: SessionId,
    pub phase: PhaseId,
    pub artifact_ids: Vec<String>,
    pub event_count: u64,
    pub created_at: DateTime<Utc>,
}

impl Checkpoint {
    #[must_use]
    pub fn new(session_id: SessionId, phase: PhaseId) -> Self {
        Self {
            checkpoint_id: CheckpointId::generate(),
            session_id,
            phase,
            artifact_ids: Vec::new(),
            event_count: 0,
            created_at: Utc::now(),
        }
    }

    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Vec<String>) -> Self {
        self.artifact_ids = artifacts;
        self
    }

    #[must_use]
    pub fn with_event_count(mut self, count: u64) -> Self {
        self.event_count = count;
        self
    }
}

pub struct StatePersistence {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl StatePersistence {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(NexusError::IoError)?;
        }

        let conn = Connection::open(&db_path).map_err(NexusError::DatabaseError)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(NexusError::DatabaseError)?;

        let persistence = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        persistence.initialize_schema()?;
        Ok(persistence)
    }

    #[must_use]
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("Failed to set PRAGMA options");

        let persistence = Self {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };
        persistence
            .initialize_schema()
            .expect("Failed to initialize schema");
        persistence
    }

    fn get_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| NexusError::LockError(format!("Failed to acquire database lock: {e}")))
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute_batch(PERSISTENCE_SCHEMA_SQL)
            .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn create_session(&self, state: &FsmState) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO fsm_state (session_id, current_phase, status, created_at, updated_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                state.session_id.0,
                i32::from(state.current_phase.0),
                state.status.to_string(),
                state.created_at.to_rfc3339(),
                state.updated_at.to_rfc3339(),
                serde_json::to_string(&state.metadata).unwrap_or_default(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;

        for phase_num in 0..=state.current_phase.0 {
            let phase_record = PhaseStateRecord::new(state.session_id.clone(), PhaseId(phase_num));
            self.save_phase_state(&phase_record)?;
        }

        Ok(())
    }

    pub fn load_session(&self, session_id: &SessionId) -> Result<Option<FsmState>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, current_phase, status, created_at, updated_at, metadata
                 FROM fsm_state WHERE session_id = ?1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![session_id.0], |row| {
                Ok(FsmState {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    current_phase: PhaseId(row.get::<_, i32>(1)? as u8),
                    status: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or(SessionStatus::Active),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    metadata: serde_json::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or(serde_json::json!({})),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn update_session(&self, state: &FsmState) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE fsm_state SET current_phase = ?1, status = ?2, updated_at = ?3, metadata = ?4
             WHERE session_id = ?5",
            params![
                i32::from(state.current_phase.0),
                state.status.to_string(),
                state.updated_at.to_rfc3339(),
                serde_json::to_string(&state.metadata).unwrap_or_default(),
                state.session_id.0,
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &SessionId) -> Result<bool> {
        let conn = self.get_connection()?;
        let rows = conn
            .execute(
                "DELETE FROM fsm_state WHERE session_id = ?1",
                params![session_id.0],
            )
            .map_err(NexusError::DatabaseError)?;
        Ok(rows > 0)
    }

    pub fn list_sessions(&self, status: Option<SessionStatus>) -> Result<Vec<FsmState>> {
        let conn = self.get_connection()?;
        let sql = match status {
            Some(_) => "SELECT session_id, current_phase, status, created_at, updated_at, metadata FROM fsm_state WHERE status = ?1",
            None => "SELECT session_id, current_phase, status, created_at, updated_at, metadata FROM fsm_state",
        };

        let mut stmt = conn.prepare(sql).map_err(NexusError::DatabaseError)?;

        let parse_row = |row: &rusqlite::Row<'_>| {
            Ok(FsmState {
                session_id: SessionId::new(row.get::<_, String>(0)?),
                current_phase: PhaseId(row.get::<_, i32>(1)? as u8),
                status: row
                    .get::<_, String>(2)?
                    .parse()
                    .unwrap_or(SessionStatus::Active),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                metadata: serde_json::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(serde_json::json!({})),
            })
        };

        let rows = match status {
            Some(s) => stmt.query_map(params![s.to_string()], parse_row),
            None => stmt.query_map([], parse_row),
        }
        .map_err(NexusError::DatabaseError)?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(sessions)
    }

    pub fn save_phase_state(&self, state: &PhaseStateRecord) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO phase_state 
             (session_id, phase, status, started_at, completed_at, duration_ms, 
              artifacts_produced, gates_passed, gates_failed, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                state.session_id.0,
                i32::from(state.phase.0),
                state.status.to_string(),
                state.started_at.map(|t| t.to_rfc3339()),
                state.completed_at.map(|t| t.to_rfc3339()),
                state.duration_ms.map(|d| d as i64),
                serde_json::to_string(&state.artifacts_produced).unwrap_or_default(),
                serde_json::to_string(&state.gates_passed).unwrap_or_default(),
                serde_json::to_string(&state.gates_failed).unwrap_or_default(),
                serde_json::to_string(&state.metadata).unwrap_or_default(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn load_phase_state(
        &self,
        session_id: &SessionId,
        phase: PhaseId,
    ) -> Result<Option<PhaseStateRecord>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, phase, status, started_at, completed_at, duration_ms,
                        artifacts_produced, gates_passed, gates_failed, metadata
                 FROM phase_state WHERE session_id = ?1 AND phase = ?2",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![session_id.0, i32::from(phase.0)], |row| {
                Ok(PhaseStateRecord {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    phase: PhaseId(row.get::<_, i32>(1)? as u8),
                    status: PhaseStatus::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(PhaseStatus::Pending),
                    started_at: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    completed_at: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    duration_ms: row.get::<_, Option<i64>>(5)?.map(|d| d as u64),
                    artifacts_produced: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or_default(),
                    gates_passed: serde_json::from_str(&row.get::<_, String>(7)?)
                        .unwrap_or_default(),
                    gates_failed: serde_json::from_str(&row.get::<_, String>(8)?)
                        .unwrap_or_default(),
                    metadata: serde_json::from_str(&row.get::<_, String>(9)?)
                        .unwrap_or(serde_json::json!({})),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn load_all_phase_states(&self, session_id: &SessionId) -> Result<Vec<PhaseStateRecord>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, phase, status, started_at, completed_at, duration_ms,
                        artifacts_produced, gates_passed, gates_failed, metadata
                 FROM phase_state WHERE session_id = ?1 ORDER BY phase",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(params![session_id.0], |row| {
                Ok(PhaseStateRecord {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    phase: PhaseId(row.get::<_, i32>(1)? as u8),
                    status: PhaseStatus::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(PhaseStatus::Pending),
                    started_at: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    completed_at: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    duration_ms: row.get::<_, Option<i64>>(5)?.map(|d| d as u64),
                    artifacts_produced: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or_default(),
                    gates_passed: serde_json::from_str(&row.get::<_, String>(7)?)
                        .unwrap_or_default(),
                    gates_failed: serde_json::from_str(&row.get::<_, String>(8)?)
                        .unwrap_or_default(),
                    metadata: serde_json::from_str(&row.get::<_, String>(9)?)
                        .unwrap_or(serde_json::json!({})),
                })
            })
            .map_err(NexusError::DatabaseError)?;

        let mut states = Vec::new();
        for row in rows {
            states.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(states)
    }

    pub fn create_snapshot(&self, snapshot: &StateSnapshot) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO state_snapshots 
             (session_id, snapshot_id, phase, snapshot_type, data, checksum, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snapshot.session_id.0,
                snapshot.snapshot_id.0,
                i32::from(snapshot.phase.0),
                snapshot.snapshot_type.to_string(),
                serde_json::to_string(&snapshot.data).unwrap_or_default(),
                snapshot.checksum,
                snapshot.created_at.to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn load_snapshot(&self, snapshot_id: &SnapshotId) -> Result<Option<StateSnapshot>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, snapshot_id, phase, snapshot_type, data, checksum, created_at
                 FROM state_snapshots WHERE snapshot_id = ?1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![snapshot_id.0], |row| {
                Ok(StateSnapshot {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    snapshot_id: SnapshotId(row.get::<_, String>(1)?),
                    phase: PhaseId(row.get::<_, i32>(2)? as u8),
                    snapshot_type: SnapshotType::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(SnapshotType::Full),
                    data: serde_json::from_str(&row.get::<_, String>(4)?)
                        .unwrap_or(serde_json::json!({})),
                    checksum: row.get::<_, String>(5)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn list_snapshots(&self, session_id: &SessionId) -> Result<Vec<StateSnapshot>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, snapshot_id, phase, snapshot_type, data, checksum, created_at
                 FROM state_snapshots WHERE session_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(params![session_id.0], |row| {
                Ok(StateSnapshot {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    snapshot_id: SnapshotId(row.get::<_, String>(1)?),
                    phase: PhaseId(row.get::<_, i32>(2)? as u8),
                    snapshot_type: SnapshotType::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(SnapshotType::Full),
                    data: serde_json::from_str(&row.get::<_, String>(4)?)
                        .unwrap_or(serde_json::json!({})),
                    checksum: row.get::<_, String>(5)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .map_err(NexusError::DatabaseError)?;

        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(snapshots)
    }

    pub fn delete_snapshot(&self, snapshot_id: &SnapshotId) -> Result<bool> {
        let conn = self.get_connection()?;
        let rows = conn
            .execute(
                "DELETE FROM state_snapshots WHERE snapshot_id = ?1",
                params![snapshot_id.0],
            )
            .map_err(NexusError::DatabaseError)?;
        Ok(rows > 0)
    }

    pub fn log_recovery_event(&self, event: &RecoveryEvent) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO recovery_log (session_id, event_type, event_data, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                event.session_id.0,
                event.event_type,
                serde_json::to_string(&event.event_data).unwrap_or_default(),
                event.timestamp.to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn get_recovery_events(
        &self,
        session_id: &SessionId,
        limit: usize,
    ) -> Result<Vec<RecoveryEvent>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, event_type, event_data, timestamp
                 FROM recovery_log WHERE session_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(params![session_id.0, limit as i64], |row| {
                Ok(RecoveryEvent {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    event_type: row.get::<_, String>(1)?,
                    event_data: serde_json::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(serde_json::json!({})),
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .map_err(NexusError::DatabaseError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(events)
    }

    pub fn create_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO checkpoints (session_id, checkpoint_id, phase, artifact_ids, event_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                checkpoint.session_id.0,
                checkpoint.checkpoint_id.0,
                i32::from(checkpoint.phase.0),
                serde_json::to_string(&checkpoint.artifact_ids).unwrap_or_default(),
                checkpoint.event_count as i64,
                checkpoint.created_at.to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<Option<Checkpoint>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, checkpoint_id, phase, artifact_ids, event_count, created_at
                 FROM checkpoints WHERE checkpoint_id = ?1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![checkpoint_id.0], |row| {
                Ok(Checkpoint {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    checkpoint_id: CheckpointId(row.get::<_, String>(1)?),
                    phase: PhaseId(row.get::<_, i32>(2)? as u8),
                    artifact_ids: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or_default(),
                    event_count: row.get::<_, i64>(4)? as u64,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn get_latest_checkpoint(&self, session_id: &SessionId) -> Result<Option<Checkpoint>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, checkpoint_id, phase, artifact_ids, event_count, created_at
                 FROM checkpoints WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![session_id.0], |row| {
                Ok(Checkpoint {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    checkpoint_id: CheckpointId(row.get::<_, String>(1)?),
                    phase: PhaseId(row.get::<_, i32>(2)? as u8),
                    artifact_ids: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or_default(),
                    event_count: row.get::<_, i64>(4)? as u64,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn list_checkpoints(&self, session_id: &SessionId) -> Result<Vec<Checkpoint>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, checkpoint_id, phase, artifact_ids, event_count, created_at
                 FROM checkpoints WHERE session_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(params![session_id.0], |row| {
                Ok(Checkpoint {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    checkpoint_id: CheckpointId(row.get::<_, String>(1)?),
                    phase: PhaseId(row.get::<_, i32>(2)? as u8),
                    artifact_ids: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or_default(),
                    event_count: row.get::<_, i64>(4)? as u64,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .map_err(NexusError::DatabaseError)?;

        let mut checkpoints = Vec::new();
        for row in rows {
            checkpoints.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(checkpoints)
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<bool> {
        let conn = self.get_connection()?;
        let rows = conn
            .execute(
                "DELETE FROM checkpoints WHERE checkpoint_id = ?1",
                params![checkpoint_id.0],
            )
            .map_err(NexusError::DatabaseError)?;
        Ok(rows > 0)
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}

impl std::fmt::Debug for StatePersistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatePersistence")
            .field("db_path", &self.db_path)
            .finish()
    }
}

pub struct CrashRecovery {
    persistence: Arc<StatePersistence>,
}

impl CrashRecovery {
    pub fn new(persistence: Arc<StatePersistence>) -> Self {
        Self { persistence }
    }

    pub fn detect_crashed_sessions(&self) -> Result<Vec<FsmState>> {
        let sessions = self
            .persistence
            .list_sessions(Some(SessionStatus::Active))?;
        let crashed: Vec<FsmState> = sessions
            .into_iter()
            .filter(|s| {
                if let Ok(Some(_)) = self.persistence.get_latest_checkpoint(&s.session_id) {
                    let updated_age = (Utc::now() - s.updated_at).num_seconds();
                    updated_age > 300
                } else {
                    false
                }
            })
            .collect();
        Ok(crashed)
    }

    pub fn recover_session(&self, session_id: &SessionId) -> Result<Option<Checkpoint>> {
        let mut state = self
            .persistence
            .load_session(session_id)?
            .ok_or_else(|| NexusError::LockError(format!("Session not found: {session_id}")))?;

        state.status = SessionStatus::Recovering;
        state.touch();
        self.persistence.update_session(&state)?;

        let checkpoint = self.persistence.get_latest_checkpoint(session_id)?;

        if let Some(ref cp) = checkpoint {
            state.current_phase = cp.phase;
            state.touch();
            self.persistence.update_session(&state)?;

            self.persistence.log_recovery_event(&RecoveryEvent::new(
                session_id.clone(),
                "recovery_started",
                serde_json::json!({
                    "checkpoint_id": cp.checkpoint_id.0,
                    "phase": cp.phase.0,
                }),
            ))?;
        }

        state.status = SessionStatus::Active;
        state.touch();
        self.persistence.update_session(&state)?;

        Ok(checkpoint)
    }

    pub fn save_checkpoint(
        &self,
        session_id: &SessionId,
        phase: PhaseId,
        artifact_ids: Vec<String>,
    ) -> Result<Checkpoint> {
        let checkpoint = Checkpoint::new(session_id.clone(), phase).with_artifacts(artifact_ids);
        self.persistence.create_checkpoint(&checkpoint)?;

        self.persistence.log_recovery_event(&RecoveryEvent::new(
            session_id.clone(),
            "checkpoint_created",
            serde_json::json!({
                "checkpoint_id": checkpoint.checkpoint_id.0,
                "phase": phase.0,
            }),
        ))?;

        Ok(checkpoint)
    }

    pub fn get_recovery_history(&self, session_id: &SessionId) -> Result<Vec<RecoveryEvent>> {
        self.persistence.get_recovery_events(session_id, 100)
    }
}

impl std::fmt::Debug for CrashRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrashRecovery").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_persistence() -> StatePersistence {
        StatePersistence::in_memory()
    }

    #[test]
    fn test_session_id_generation() {
        let id1 = SessionId::generate();
        let id2 = SessionId::generate();
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_fsm_state_creation() {
        let state = FsmState::new(SessionId::new("test"), PhaseId(5))
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(state.current_phase, PhaseId(5));
        assert_eq!(state.status, SessionStatus::Active);
        assert!(state.metadata.is_object());
    }

    #[test]
    fn test_phase_state_record_lifecycle() {
        let mut record = PhaseStateRecord::new(SessionId::new("test"), PhaseId(0));
        assert_eq!(record.status, PhaseStatus::Pending);

        record.start();
        assert_eq!(record.status, PhaseStatus::InProgress);
        assert!(record.started_at.is_some());

        record.complete(
            vec!["artifact1".to_string()],
            vec!["gate1".to_string()],
            vec![],
        );
        assert_eq!(record.status, PhaseStatus::Completed);
        assert!(record.completed_at.is_some());
        assert_eq!(record.artifacts_produced.len(), 1);
    }

    #[test]
    fn test_persistence_create_and_load_session() {
        let persistence = create_test_persistence();
        let state = FsmState::new(SessionId::new("test-session"), PhaseId(3));

        persistence.create_session(&state).unwrap();

        let loaded = persistence
            .load_session(&SessionId::new("test-session"))
            .unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.session_id.0, "test-session");
        assert_eq!(loaded.current_phase, PhaseId(3));
        assert_eq!(loaded.status, SessionStatus::Active);
    }

    #[test]
    fn test_persistence_update_session() {
        let persistence = create_test_persistence();
        let state = FsmState::new(SessionId::new("test"), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let mut loaded = persistence
            .load_session(&SessionId::new("test"))
            .unwrap()
            .unwrap();
        loaded.current_phase = PhaseId(5);
        loaded.status = SessionStatus::Paused;
        loaded.touch();
        persistence.update_session(&loaded).unwrap();

        let reloaded = persistence
            .load_session(&SessionId::new("test"))
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.current_phase, PhaseId(5));
        assert_eq!(reloaded.status, SessionStatus::Paused);
    }

    #[test]
    fn test_persistence_delete_session() {
        let persistence = create_test_persistence();
        let state = FsmState::new(SessionId::new("test"), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let deleted = persistence.delete_session(&SessionId::new("test")).unwrap();
        assert!(deleted);

        let loaded = persistence.load_session(&SessionId::new("test")).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_persistence_list_sessions() {
        let persistence = create_test_persistence();

        for i in 0..3 {
            let state = FsmState::new(SessionId::new(format!("session-{}", i)), PhaseId(i));
            persistence.create_session(&state).unwrap();
        }

        let sessions = persistence.list_sessions(None).unwrap();
        assert_eq!(sessions.len(), 3);

        let mut session = persistence
            .load_session(&SessionId::new("session-1"))
            .unwrap()
            .unwrap();
        session.status = SessionStatus::Completed;
        persistence.update_session(&session).unwrap();

        let active = persistence
            .list_sessions(Some(SessionStatus::Active))
            .unwrap();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_persistence_phase_state() {
        let persistence = create_test_persistence();
        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let mut phase_record = PhaseStateRecord::new(session_id.clone(), PhaseId(5));
        phase_record.start();
        phase_record.complete(
            vec!["artifact1".to_string()],
            vec!["gate1".to_string()],
            vec![],
        );
        persistence.save_phase_state(&phase_record).unwrap();

        let loaded = persistence
            .load_phase_state(&session_id, PhaseId(5))
            .unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.status, PhaseStatus::Completed);
        assert_eq!(loaded.artifacts_produced.len(), 1);
    }

    #[test]
    fn test_persistence_snapshots() {
        let persistence = create_test_persistence();
        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let snapshot = StateSnapshot {
            snapshot_id: SnapshotId::generate(),
            session_id: session_id.clone(),
            phase: PhaseId(5),
            snapshot_type: SnapshotType::Full,
            data: serde_json::json!({"key": "value"}),
            checksum: "abc123".to_string(),
            created_at: Utc::now(),
        };

        persistence.create_snapshot(&snapshot).unwrap();

        let loaded = persistence.load_snapshot(&snapshot.snapshot_id).unwrap();
        assert!(loaded.is_some());

        let snapshots = persistence.list_snapshots(&session_id).unwrap();
        assert_eq!(snapshots.len(), 1);

        let deleted = persistence.delete_snapshot(&snapshot.snapshot_id).unwrap();
        assert!(deleted);

        let snapshots = persistence.list_snapshots(&session_id).unwrap();
        assert!(snapshots.is_empty());
    }

    #[test]
    fn test_persistence_checkpoints() {
        let persistence = create_test_persistence();
        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let checkpoint = Checkpoint::new(session_id.clone(), PhaseId(5))
            .with_artifacts(vec!["a1".to_string(), "a2".to_string()])
            .with_event_count(10);

        persistence.create_checkpoint(&checkpoint).unwrap();

        let loaded = persistence
            .load_checkpoint(&checkpoint.checkpoint_id)
            .unwrap();
        assert!(loaded.is_some());

        let latest = persistence.get_latest_checkpoint(&session_id).unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().phase, PhaseId(5));

        let checkpoints = persistence.list_checkpoints(&session_id).unwrap();
        assert_eq!(checkpoints.len(), 1);
    }

    #[test]
    fn test_crash_recovery() {
        let persistence = Arc::new(create_test_persistence());
        let recovery = CrashRecovery::new(persistence.clone());

        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(5));
        persistence.create_session(&state).unwrap();

        recovery
            .save_checkpoint(&session_id, PhaseId(3), vec!["artifact1".to_string()])
            .unwrap();

        let history = recovery.get_recovery_history(&session_id).unwrap();
        assert!(!history.is_empty());

        let recovered = recovery.recover_session(&session_id).unwrap();
        assert!(recovered.is_some());
        assert_eq!(recovered.unwrap().phase, PhaseId(3));
    }

    #[test]
    fn test_session_status_parsing() {
        assert_eq!(
            SessionStatus::from_str("active").unwrap(),
            SessionStatus::Active
        );
        assert_eq!(
            SessionStatus::from_str("PAUSED").unwrap(),
            SessionStatus::Paused
        );
        assert_eq!(
            SessionStatus::from_str("Completed").unwrap(),
            SessionStatus::Completed
        );
        assert!(SessionStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_phase_status_parsing() {
        assert_eq!(
            PhaseStatus::from_str("pending").unwrap(),
            PhaseStatus::Pending
        );
        assert_eq!(
            PhaseStatus::from_str("in_progress").unwrap(),
            PhaseStatus::InProgress
        );
        assert_eq!(
            PhaseStatus::from_str("COMPLETED").unwrap(),
            PhaseStatus::Completed
        );
        assert!(PhaseStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_snapshot_type_parsing() {
        assert_eq!(SnapshotType::from_str("full").unwrap(), SnapshotType::Full);
        assert_eq!(
            SnapshotType::from_str("incremental").unwrap(),
            SnapshotType::Incremental
        );
        assert_eq!(
            SnapshotType::from_str("CHECKPOINT").unwrap(),
            SnapshotType::Checkpoint
        );
        assert!(SnapshotType::from_str("invalid").is_err());
    }

    #[test]
    fn test_persistence_all_phase_states() {
        let persistence = create_test_persistence();
        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence.create_session(&state).unwrap();

        for i in 0..3 {
            let mut record = PhaseStateRecord::new(session_id.clone(), PhaseId(i));
            record.complete(vec![], vec![], vec![]);
            persistence.save_phase_state(&record).unwrap();
        }

        let all_states = persistence.load_all_phase_states(&session_id).unwrap();
        assert_eq!(all_states.len(), 3);
    }

    #[test]
    fn test_recovery_events() {
        let persistence = create_test_persistence();
        let session_id = SessionId::new("test");
        let state = FsmState::new(session_id.clone(), PhaseId(0));
        persistence.create_session(&state).unwrap();

        let event1 = RecoveryEvent::new(
            session_id.clone(),
            "phase_started",
            serde_json::json!({"phase": 0}),
        );
        let event2 = RecoveryEvent::new(
            session_id.clone(),
            "phase_completed",
            serde_json::json!({"phase": 0, "duration_ms": 100}),
        );

        persistence.log_recovery_event(&event1).unwrap();
        persistence.log_recovery_event(&event2).unwrap();

        let events = persistence.get_recovery_events(&session_id, 10).unwrap();
        assert_eq!(events.len(), 2);
    }
}
