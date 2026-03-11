//! Event Sourcing for Nexus FSM Phase 3
//!
//! This module implements full event history persistence and event replay
//! for debugging and recovery purposes.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::events::NexusEvent;
use super::{NexusError, Result};

const EVENT_STORE_SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS event_store (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_version INTEGER NOT NULL DEFAULT 1,
    event_data TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    timestamp TEXT NOT NULL,
    sequence_number INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_event_store_aggregate ON event_store(aggregate_id);
CREATE INDEX IF NOT EXISTS idx_event_store_type ON event_store(event_type);
CREATE INDEX IF NOT EXISTS idx_event_store_timestamp ON event_store(timestamp);
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_store_sequence ON event_store(aggregate_id, sequence_number);

CREATE TABLE IF NOT EXISTS event_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    snapshot_data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(aggregate_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_snapshots_aggregate ON event_snapshots(aggregate_id);

CREATE TABLE IF NOT EXISTS event_subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscriber_id TEXT NOT NULL UNIQUE,
    last_processed_sequence INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS replay_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE,
    aggregate_id TEXT NOT NULL,
    from_sequence INTEGER NOT NULL,
    to_sequence INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    completed_at TEXT,
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_replay_aggregate ON replay_sessions(aggregate_id);
";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: i64,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event_type: String,
    pub event_version: u32,
    pub event_data: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: i64,
}

impl EventEnvelope {
    pub fn new(
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        event_type: impl Into<String>,
        event_data: serde_json::Value,
    ) -> Self {
        Self {
            id: 0,
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            event_type: event_type.into(),
            event_version: 1,
            event_data,
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            sequence_number: 0,
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn with_version(mut self, version: u32) -> Self {
        self.event_version = version;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub user_id: Option<String>,
    pub source: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

impl EventMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self {
            correlation_id: None,
            causation_id: None,
            user_id: None,
            source: None,
            custom: HashMap::new(),
        }
    }

    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn with_causation_id(mut self, id: impl Into<String>) -> Self {
        self.causation_id = Some(id.into());
        self
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub sequence_number: i64,
    pub snapshot_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl Snapshot {
    pub fn new(
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        sequence_number: i64,
        data: serde_json::Value,
    ) -> Self {
        Self {
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            sequence_number,
            snapshot_data: data,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for ReplayStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayStatus::Pending => write!(f, "pending"),
            ReplayStatus::Running => write!(f, "running"),
            ReplayStatus::Completed => write!(f, "completed"),
            ReplayStatus::Failed => write!(f, "failed"),
            ReplayStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for ReplayStatus {
    type Err = NexusError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ReplayStatus::Pending),
            "running" => Ok(ReplayStatus::Running),
            "completed" => Ok(ReplayStatus::Completed),
            "failed" => Ok(ReplayStatus::Failed),
            "cancelled" => Ok(ReplayStatus::Cancelled),
            _ => Err(NexusError::LockError(format!("Invalid replay status: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySession {
    pub session_id: String,
    pub aggregate_id: String,
    pub from_sequence: i64,
    pub to_sequence: i64,
    pub status: ReplayStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl ReplaySession {
    pub fn new(aggregate_id: impl Into<String>, from: i64, to: i64) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            aggregate_id: aggregate_id.into(),
            from_sequence: from,
            to_sequence: to,
            status: ReplayStatus::Pending,
            created_at: Utc::now(),
            completed_at: None,
            error: None,
        }
    }
}

pub trait EventProjection: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &str;
    fn handle(&mut self, event: &EventEnvelope) -> Result<()>;
    fn state(&self) -> serde_json::Value;
    fn reset(&mut self);
}

pub struct EventStore {
    conn: Mutex<Connection>,
    db_path: PathBuf,
    projections: Mutex<Vec<Box<dyn EventProjection>>>,
    snapshot_threshold: i64,
}

impl EventStore {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(NexusError::IoError)?;
        }

        let conn = Connection::open(&db_path).map_err(NexusError::DatabaseError)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(NexusError::DatabaseError)?;

        let store = Self {
            conn: Mutex::new(conn),
            db_path,
            projections: Mutex::new(Vec::new()),
            snapshot_threshold: 100,
        };
        store.initialize_schema()?;
        Ok(store)
    }

    #[must_use]
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        conn.execute_batch("PRAGMA synchronous=OFF; PRAGMA cache_size=-64000;")
            .expect("Failed to set PRAGMA options");

        let store = Self {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
            projections: Mutex::new(Vec::new()),
            snapshot_threshold: 100,
        };
        store
            .initialize_schema()
            .expect("Failed to initialize schema");
        store
    }

    fn get_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| NexusError::LockError(format!("Failed to acquire database lock: {e}")))
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute_batch(EVENT_STORE_SCHEMA_SQL)
            .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn append(&self, envelope: EventEnvelope) -> Result<i64> {
        let conn = self.get_connection()?;

        let next_sequence = self.get_next_sequence_with_conn(&conn, &envelope.aggregate_id)?;

        conn.execute(
            "INSERT INTO event_store 
             (aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                envelope.aggregate_id,
                envelope.aggregate_type,
                envelope.event_type,
                envelope.event_version as i32,
                serde_json::to_string(&envelope.event_data).unwrap_or_default(),
                serde_json::to_string(&envelope.metadata).unwrap_or_default(),
                envelope.timestamp.to_rfc3339(),
                next_sequence,
            ],
        )
        .map_err(NexusError::DatabaseError)?;

        let id = conn.last_insert_rowid();

        if next_sequence > 0 && next_sequence % self.snapshot_threshold == 0 {
            self.create_snapshot_internal(
                &conn,
                &envelope.aggregate_id,
                &envelope.aggregate_type,
                next_sequence,
            )?;
        }

        drop(conn);
        self.update_projections(&envelope)?;

        Ok(id)
    }

    fn get_next_sequence_with_conn(&self, conn: &Connection, aggregate_id: &str) -> Result<i64> {
        let max_seq: Option<i64> = conn
            .query_row(
                "SELECT MAX(sequence_number) FROM event_store WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(NexusError::DatabaseError)?;

        Ok(max_seq.unwrap_or(-1) + 1)
    }

    #[allow(dead_code)]
    fn get_next_sequence(&self, aggregate_id: &str) -> Result<i64> {
        let conn = self.get_connection()?;
        self.get_next_sequence_with_conn(&conn, aggregate_id)
    }

    fn create_snapshot_internal(
        &self,
        conn: &Connection,
        aggregate_id: &str,
        aggregate_type: &str,
        sequence_number: i64,
    ) -> Result<()> {
        let state = self.rebuild_state_with_conn(conn, aggregate_id)?;

        conn.execute(
            "INSERT INTO event_snapshots (aggregate_id, aggregate_type, sequence_number, snapshot_data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                aggregate_id,
                aggregate_type,
                sequence_number,
                serde_json::to_string(&state).unwrap_or_default(),
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;

        Ok(())
    }

    fn rebuild_state_with_conn(
        &self,
        conn: &Connection,
        aggregate_id: &str,
    ) -> Result<serde_json::Value> {
        let events = self.get_events_for_aggregate_with_conn(conn, aggregate_id, None, None)?;

        let mut state = serde_json::json!({});
        for event in events {
            self.apply_event_to_state(&mut state, &event);
        }
        Ok(state)
    }

    #[allow(dead_code)]
    fn rebuild_state_from_events(&self, aggregate_id: &str) -> Result<serde_json::Value> {
        let conn = self.get_connection()?;
        self.rebuild_state_with_conn(&conn, aggregate_id)
    }

    fn apply_event_to_state(&self, state: &mut serde_json::Value, event: &EventEnvelope) {
        if let serde_json::Value::Object(map) = state {
            if let serde_json::Value::Object(ref data) = event.event_data {
                for (key, value) in data {
                    map.insert(key.clone(), value.clone());
                }
            }
            map.insert(
                "last_event_type".to_string(),
                serde_json::json!(event.event_type),
            );
            map.insert(
                "last_event_time".to_string(),
                serde_json::json!(event.timestamp.to_rfc3339()),
            );
            map.insert(
                "current_sequence".to_string(),
                serde_json::json!(event.sequence_number),
            );
        }
    }

    fn update_projections(&self, envelope: &EventEnvelope) -> Result<()> {
        let mut projections = self.projections.lock().map_err(|e| {
            NexusError::LockError(format!("Failed to acquire projections lock: {e}"))
        })?;

        for projection in projections.iter_mut() {
            projection.handle(envelope)?;
        }
        Ok(())
    }

    pub fn get_events_for_aggregate(
        &self,
        aggregate_id: &str,
        from_sequence: Option<i64>,
        to_sequence: Option<i64>,
    ) -> Result<Vec<EventEnvelope>> {
        let conn = self.get_connection()?;

        let sql = match (from_sequence, to_sequence) {
            (Some(_from), Some(_to)) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number >= ?2 AND sequence_number <= ?3 ORDER BY sequence_number",
            (Some(_from), None) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number >= ?2 ORDER BY sequence_number",
            (None, Some(_to)) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number <= ?2 ORDER BY sequence_number",
            (None, None) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 ORDER BY sequence_number",
        };

        let mut stmt = conn.prepare(sql).map_err(NexusError::DatabaseError)?;

        let parse_row = |row: &rusqlite::Row<'_>| {
            Ok(EventEnvelope {
                id: row.get(0)?,
                aggregate_id: row.get(1)?,
                aggregate_type: row.get(2)?,
                event_type: row.get(3)?,
                event_version: row.get::<_, i32>(4)? as u32,
                event_data: serde_json::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(serde_json::json!({})),
                metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                sequence_number: row.get(8)?,
            })
        };

        let rows = match (from_sequence, to_sequence) {
            (Some(from), Some(to)) => stmt.query_map(params![aggregate_id, from, to], parse_row),
            (Some(from), None) => stmt.query_map(params![aggregate_id, from], parse_row),
            (None, Some(to)) => stmt.query_map(params![aggregate_id, to], parse_row),
            (None, None) => stmt.query_map(params![aggregate_id], parse_row),
        }
        .map_err(NexusError::DatabaseError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(events)
    }

    fn get_events_for_aggregate_with_conn(
        &self,
        conn: &Connection,
        aggregate_id: &str,
        from_sequence: Option<i64>,
        to_sequence: Option<i64>,
    ) -> Result<Vec<EventEnvelope>> {
        let sql = match (from_sequence, to_sequence) {
            (Some(_from), Some(_to)) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number >= ?2 AND sequence_number <= ?3 ORDER BY sequence_number",
            (Some(_from), None) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number >= ?2 ORDER BY sequence_number",
            (None, Some(_to)) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 AND sequence_number <= ?2 ORDER BY sequence_number",
            (None, None) => "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number FROM event_store WHERE aggregate_id = ?1 ORDER BY sequence_number",
        };

        let mut stmt = conn.prepare(sql).map_err(NexusError::DatabaseError)?;

        let parse_row = |row: &rusqlite::Row<'_>| {
            Ok(EventEnvelope {
                id: row.get(0)?,
                aggregate_id: row.get(1)?,
                aggregate_type: row.get(2)?,
                event_type: row.get(3)?,
                event_version: row.get::<_, i32>(4)? as u32,
                event_data: serde_json::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(serde_json::json!({})),
                metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                sequence_number: row.get(8)?,
            })
        };

        let rows = match (from_sequence, to_sequence) {
            (Some(from), Some(to)) => stmt.query_map(params![aggregate_id, from, to], parse_row),
            (Some(from), None) => stmt.query_map(params![aggregate_id, from], parse_row),
            (None, Some(to)) => stmt.query_map(params![aggregate_id, to], parse_row),
            (None, None) => stmt.query_map(params![aggregate_id], parse_row),
        }
        .map_err(NexusError::DatabaseError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(events)
    }

    pub fn get_events_by_type(&self, event_type: &str, limit: usize) -> Result<Vec<EventEnvelope>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number
                 FROM event_store WHERE event_type = ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(params![event_type, limit as i64], |row| {
                Ok(EventEnvelope {
                    id: row.get(0)?,
                    aggregate_id: row.get(1)?,
                    aggregate_type: row.get(2)?,
                    event_type: row.get(3)?,
                    event_version: row.get::<_, i32>(4)? as u32,
                    event_data: serde_json::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or(serde_json::json!({})),
                    metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    sequence_number: row.get(8)?,
                })
            })
            .map_err(NexusError::DatabaseError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(events)
    }

    pub fn get_events_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<EventEnvelope>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, aggregate_id, aggregate_type, event_type, event_version, event_data, metadata, timestamp, sequence_number
                 FROM event_store WHERE timestamp >= ?1 AND timestamp <= ?2 ORDER BY timestamp LIMIT ?3",
            )
            .map_err(NexusError::DatabaseError)?;

        let rows = stmt
            .query_map(
                params![from.to_rfc3339(), to.to_rfc3339(), limit as i64],
                |row| {
                    Ok(EventEnvelope {
                        id: row.get(0)?,
                        aggregate_id: row.get(1)?,
                        aggregate_type: row.get(2)?,
                        event_type: row.get(3)?,
                        event_version: row.get::<_, i32>(4)? as u32,
                        event_data: serde_json::from_str(&row.get::<_, String>(5)?)
                            .unwrap_or(serde_json::json!({})),
                        metadata: serde_json::from_str(&row.get::<_, String>(6)?)
                            .unwrap_or_default(),
                        timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                            .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                        sequence_number: row.get(8)?,
                    })
                },
            )
            .map_err(NexusError::DatabaseError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(NexusError::DatabaseError)?);
        }
        Ok(events)
    }

    pub fn get_latest_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT aggregate_id, aggregate_type, sequence_number, snapshot_data, created_at
                 FROM event_snapshots WHERE aggregate_id = ?1 ORDER BY sequence_number DESC LIMIT 1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![aggregate_id], |row| {
                Ok(Snapshot {
                    aggregate_id: row.get(0)?,
                    aggregate_type: row.get(1)?,
                    sequence_number: row.get(2)?,
                    snapshot_data: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(serde_json::json!({})),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn create_snapshot(&self, aggregate_id: &str) -> Result<Snapshot> {
        let conn = self.get_connection()?;

        let latest_sequence = self.get_next_sequence_with_conn(&conn, aggregate_id)? - 1;
        if latest_sequence < 0 {
            return Err(NexusError::LockError("No events to snapshot".to_string()));
        }

        let state = self.rebuild_state_with_conn(&conn, aggregate_id)?;

        conn.execute(
            "INSERT INTO event_snapshots (aggregate_id, aggregate_type, sequence_number, snapshot_data, created_at)
             SELECT ?1, aggregate_type, ?2, ?3, ?4 FROM event_store WHERE aggregate_id = ?1 LIMIT 1",
            params![
                aggregate_id,
                latest_sequence,
                serde_json::to_string(&state).unwrap_or_default(),
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;

        Ok(Snapshot::new(
            aggregate_id,
            "nexus_session",
            latest_sequence,
            state,
        ))
    }

    pub fn rebuild_state(&self, aggregate_id: &str) -> Result<serde_json::Value> {
        let snapshot = self.get_latest_snapshot(aggregate_id)?;

        let (mut state, from_sequence) = match snapshot {
            Some(snap) => (snap.snapshot_data, Some(snap.sequence_number + 1)),
            None => (serde_json::json!({}), None),
        };

        let events = self.get_events_for_aggregate(aggregate_id, from_sequence, None)?;

        for event in events {
            self.apply_event_to_state(&mut state, &event);
        }

        Ok(state)
    }

    pub fn replay<F>(&self, aggregate_id: &str, mut handler: F) -> Result<ReplaySession>
    where
        F: FnMut(&EventEnvelope) -> Result<()>,
    {
        let events = self.get_events_for_aggregate(aggregate_id, None, None)?;

        if events.is_empty() {
            return Err(NexusError::LockError("No events to replay".to_string()));
        }

        let mut session = ReplaySession::new(
            aggregate_id,
            events.first().map_or(0, |e| e.sequence_number),
            events.last().map_or(0, |e| e.sequence_number),
        );

        self.save_replay_session(&session)?;

        session.status = ReplayStatus::Running;

        for event in &events {
            if let Err(e) = handler(event) {
                session.status = ReplayStatus::Failed;
                session.error = Some(e.to_string());
                session.completed_at = Some(Utc::now());
                self.update_replay_session(&session)?;
                return Err(e);
            }
        }

        session.status = ReplayStatus::Completed;
        session.completed_at = Some(Utc::now());
        self.update_replay_session(&session)?;

        Ok(session)
    }

    pub fn replay_range<F>(
        &self,
        aggregate_id: &str,
        from_sequence: i64,
        to_sequence: i64,
        mut handler: F,
    ) -> Result<ReplaySession>
    where
        F: FnMut(&EventEnvelope) -> Result<()>,
    {
        let events =
            self.get_events_for_aggregate(aggregate_id, Some(from_sequence), Some(to_sequence))?;

        if events.is_empty() {
            return Err(NexusError::LockError(
                "No events in range to replay".to_string(),
            ));
        }

        let mut session = ReplaySession::new(aggregate_id, from_sequence, to_sequence);

        self.save_replay_session(&session)?;

        session.status = ReplayStatus::Running;

        for event in &events {
            if let Err(e) = handler(event) {
                session.status = ReplayStatus::Failed;
                session.error = Some(e.to_string());
                session.completed_at = Some(Utc::now());
                self.update_replay_session(&session)?;
                return Err(e);
            }
        }

        session.status = ReplayStatus::Completed;
        session.completed_at = Some(Utc::now());
        self.update_replay_session(&session)?;

        Ok(session)
    }

    fn save_replay_session(&self, session: &ReplaySession) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO replay_sessions (session_id, aggregate_id, from_sequence, to_sequence, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.session_id,
                session.aggregate_id,
                session.from_sequence,
                session.to_sequence,
                session.status.to_string(),
                session.created_at.to_rfc3339(),
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    fn update_replay_session(&self, session: &ReplaySession) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE replay_sessions SET status = ?1, completed_at = ?2, error = ?3 WHERE session_id = ?4",
            params![
                session.status.to_string(),
                session.completed_at.map(|t| t.to_rfc3339()),
                session.error,
                session.session_id,
            ],
        )
        .map_err(NexusError::DatabaseError)?;
        Ok(())
    }

    pub fn get_replay_session(&self, session_id: &str) -> Result<Option<ReplaySession>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, aggregate_id, from_sequence, to_sequence, status, created_at, completed_at, error
                 FROM replay_sessions WHERE session_id = ?1",
            )
            .map_err(NexusError::DatabaseError)?;

        let result = stmt
            .query_row(params![session_id], |row| {
                Ok(ReplaySession {
                    session_id: row.get(0)?,
                    aggregate_id: row.get(1)?,
                    from_sequence: row.get(2)?,
                    to_sequence: row.get(3)?,
                    status: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(ReplayStatus::Pending),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    completed_at: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    error: row.get(7)?,
                })
            })
            .optional()
            .map_err(NexusError::DatabaseError)?;

        Ok(result)
    }

    pub fn add_projection(&self, projection: Box<dyn EventProjection>) -> Result<()> {
        let mut projections = self.projections.lock().map_err(|e| {
            NexusError::LockError(format!("Failed to acquire projections lock: {e}"))
        })?;
        projections.push(projection);
        Ok(())
    }

    pub fn get_projection_state(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let projections = self.projections.lock().map_err(|e| {
            NexusError::LockError(format!("Failed to acquire projections lock: {e}"))
        })?;

        for projection in projections.iter() {
            if projection.id() == id {
                return Ok(Some(projection.state()));
            }
        }
        Ok(None)
    }

    pub fn event_count(&self, aggregate_id: &str) -> Result<i64> {
        let conn = self.get_connection()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_store WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| row.get(0),
            )
            .map_err(NexusError::DatabaseError)?;
        Ok(count)
    }

    pub fn total_event_count(&self) -> Result<i64> {
        let conn = self.get_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM event_store", [], |row| row.get(0))
            .map_err(NexusError::DatabaseError)?;
        Ok(count)
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}

impl std::fmt::Debug for EventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStore")
            .field("db_path", &self.db_path)
            .field("snapshot_threshold", &self.snapshot_threshold)
            .finish()
    }
}

#[derive(Debug)]
pub struct PhaseStatisticsProjection {
    id: String,
    phase_counts: HashMap<u8, u64>,
    total_transitions: u64,
    total_duration_ms: u64,
    failures: u64,
}

impl PhaseStatisticsProjection {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: "phase_statistics".to_string(),
            phase_counts: HashMap::new(),
            total_transitions: 0,
            total_duration_ms: 0,
            failures: 0,
        }
    }
}

impl EventProjection for PhaseStatisticsProjection {
    fn id(&self) -> &str {
        &self.id
    }

    fn handle(&mut self, event: &EventEnvelope) -> Result<()> {
        match event.event_type.as_str() {
            "PhaseTransitioned" => {
                self.total_transitions += 1;
                if let Some(to_phase) = event.event_data.get("to") {
                    if let Some(phase_num) = to_phase.as_u64() {
                        *self.phase_counts.entry(phase_num as u8).or_insert(0) += 1;
                    }
                }
                if let Some(duration) = event.event_data.get("duration_ms") {
                    if let Some(ms) = duration.as_u64() {
                        self.total_duration_ms += ms;
                    }
                }
            }
            "ErrorOccurred" => {
                self.failures += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn state(&self) -> serde_json::Value {
        serde_json::json!({
            "phase_counts": self.phase_counts,
            "total_transitions": self.total_transitions,
            "total_duration_ms": self.total_duration_ms,
            "failures": self.failures,
        })
    }

    fn reset(&mut self) {
        self.phase_counts.clear();
        self.total_transitions = 0;
        self.total_duration_ms = 0;
        self.failures = 0;
    }
}

impl Default for PhaseStatisticsProjection {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn nexus_event_to_envelope(session_id: &str, event: &NexusEvent) -> EventEnvelope {
    let (event_type, event_data) = match event {
        NexusEvent::PhaseStarted { phase, timestamp } => (
            "PhaseStarted",
            serde_json::json!({
                "phase": phase.0,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::PhaseCompleted {
            phase,
            duration_ms,
            timestamp,
        } => (
            "PhaseCompleted",
            serde_json::json!({
                "phase": phase.0,
                "duration_ms": duration_ms,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::PhaseTransitioned {
            from,
            to,
            timestamp,
        } => (
            "PhaseTransitioned",
            serde_json::json!({
                "from": from.0,
                "to": to.0,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::GateEvaluated {
            gate_id,
            phase,
            passed,
            timestamp,
        } => (
            "GateEvaluated",
            serde_json::json!({
                "gate_id": gate_id,
                "phase": phase.0,
                "passed": passed,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::GatesCompleted {
            phase,
            all_passed,
            failed_count,
            timestamp,
        } => (
            "GatesCompleted",
            serde_json::json!({
                "phase": phase.0,
                "all_passed": all_passed,
                "failed_count": failed_count,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ArtifactCreated {
            id,
            artifact_type,
            phase,
            timestamp,
        } => (
            "ArtifactCreated",
            serde_json::json!({
                "artifact_id": id.0,
                "artifact_type": artifact_type,
                "phase": phase.0,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ArtifactModified { id, timestamp } => (
            "ArtifactModified",
            serde_json::json!({
                "artifact_id": id.0,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ArtifactDeleted { id, timestamp } => (
            "ArtifactDeleted",
            serde_json::json!({
                "artifact_id": id.0,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ErrorOccurred {
            error,
            phase,
            timestamp,
        } => (
            "ErrorOccurred",
            serde_json::json!({
                "error": error,
                "phase": phase.map(|p| p.0),
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ProjectInitialized {
            project_root,
            timestamp,
        } => (
            "ProjectInitialized",
            serde_json::json!({
                "project_root": project_root,
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
        NexusEvent::ProjectFinalized { timestamp } => (
            "ProjectFinalized",
            serde_json::json!({
                "timestamp": timestamp.to_rfc3339(),
            }),
        ),
    };

    EventEnvelope::new(session_id, "nexus_session", event_type, event_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::PhaseId;
    use std::str::FromStr;

    fn create_test_store() -> EventStore {
        EventStore::in_memory()
    }

    #[test]
    fn test_event_envelope_creation() {
        let envelope = EventEnvelope::new(
            "session-1",
            "nexus_session",
            "PhaseTransitioned",
            serde_json::json!({"from": 0, "to": 1}),
        )
        .with_metadata("correlation_id", serde_json::json!("corr-123"))
        .with_version(2);

        assert_eq!(envelope.aggregate_id, "session-1");
        assert_eq!(envelope.event_type, "PhaseTransitioned");
        assert_eq!(envelope.event_version, 2);
        assert!(envelope.metadata.contains_key("correlation_id"));
    }

    #[test]
    fn test_event_metadata_builder() {
        let metadata = EventMetadata::new()
            .with_correlation_id("corr-123")
            .with_causation_id("cause-456")
            .with_user("user-1")
            .with_source("test");

        assert_eq!(metadata.correlation_id, Some("corr-123".to_string()));
        assert_eq!(metadata.causation_id, Some("cause-456".to_string()));
        assert_eq!(metadata.user_id, Some("user-1".to_string()));
    }

    #[test]
    fn test_event_store_append() {
        let store = create_test_store();

        let envelope = EventEnvelope::new(
            "session-1",
            "nexus_session",
            "PhaseStarted",
            serde_json::json!({"phase": 0}),
        );

        let id = store.append(envelope).unwrap();
        assert!(id > 0);

        let count = store.event_count("session-1").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_event_store_get_events() {
        let store = create_test_store();

        for i in 0..5 {
            let envelope = EventEnvelope::new(
                "session-1",
                "nexus_session",
                "PhaseTransitioned",
                serde_json::json!({"from": i, "to": i + 1}),
            );
            store.append(envelope).unwrap();
        }

        let events = store
            .get_events_for_aggregate("session-1", None, None)
            .unwrap();
        assert_eq!(events.len(), 5);

        let events = store
            .get_events_for_aggregate("session-1", Some(2), Some(4))
            .unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_event_store_get_events_by_type() {
        let store = create_test_store();

        store
            .append(EventEnvelope::new(
                "s1",
                "session",
                "PhaseStarted",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .append(EventEnvelope::new(
                "s1",
                "session",
                "PhaseCompleted",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .append(EventEnvelope::new(
                "s1",
                "session",
                "PhaseStarted",
                serde_json::json!({}),
            ))
            .unwrap();

        let events = store.get_events_by_type("PhaseStarted", 10).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_event_store_rebuild_state() {
        let store = create_test_store();

        store
            .append(EventEnvelope::new(
                "session-1",
                "session",
                "PhaseStarted",
                serde_json::json!({"phase": 0, "status": "active"}),
            ))
            .unwrap();

        store
            .append(EventEnvelope::new(
                "session-1",
                "session",
                "PhaseTransitioned",
                serde_json::json!({"from": 0, "to": 1, "current_phase": 1}),
            ))
            .unwrap();

        let state = store.rebuild_state("session-1").unwrap();

        assert!(state.get("current_phase").is_some());
        assert_eq!(state.get("current_phase").unwrap().as_i64().unwrap(), 1);
    }

    #[test]
    fn test_event_store_snapshot() {
        let store = create_test_store();

        for i in 0..5 {
            store
                .append(EventEnvelope::new(
                    "session-1",
                    "session",
                    "PhaseTransitioned",
                    serde_json::json!({"phase": i}),
                ))
                .unwrap();
        }

        let snapshot = store.create_snapshot("session-1").unwrap();
        assert_eq!(snapshot.sequence_number, 4);

        let loaded = store.get_latest_snapshot("session-1").unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn test_event_store_replay() {
        let store = create_test_store();

        for i in 0..3 {
            store
                .append(EventEnvelope::new(
                    "session-1",
                    "session",
                    "PhaseTransitioned",
                    serde_json::json!({"phase": i}),
                ))
                .unwrap();
        }

        let mut replayed = Vec::new();
        let session = store
            .replay("session-1", |event| {
                replayed.push(event.sequence_number);
                Ok(())
            })
            .unwrap();

        assert_eq!(session.status, ReplayStatus::Completed);
        assert_eq!(replayed.len(), 3);
    }

    #[test]
    fn test_event_store_replay_range() {
        let store = create_test_store();

        for i in 0..5 {
            store
                .append(EventEnvelope::new(
                    "session-1",
                    "session",
                    "PhaseTransitioned",
                    serde_json::json!({"phase": i}),
                ))
                .unwrap();
        }

        let mut replayed = Vec::new();
        let session = store
            .replay_range("session-1", 1, 3, |event| {
                replayed.push(event.sequence_number);
                Ok(())
            })
            .unwrap();

        assert_eq!(session.status, ReplayStatus::Completed);
        assert_eq!(replayed.len(), 3);
    }

    #[test]
    fn test_phase_statistics_projection() {
        let mut projection = PhaseStatisticsProjection::new();

        projection
            .handle(&EventEnvelope::new(
                "s1",
                "session",
                "PhaseTransitioned",
                serde_json::json!({"to": 1, "duration_ms": 100}),
            ))
            .unwrap();

        projection
            .handle(&EventEnvelope::new(
                "s1",
                "session",
                "PhaseTransitioned",
                serde_json::json!({"to": 2, "duration_ms": 200}),
            ))
            .unwrap();

        projection
            .handle(&EventEnvelope::new(
                "s1",
                "session",
                "ErrorOccurred",
                serde_json::json!({"error": "test"}),
            ))
            .unwrap();

        let state = projection.state();
        assert_eq!(state["total_transitions"], 2);
        assert_eq!(state["total_duration_ms"], 300);
        assert_eq!(state["failures"], 1);
    }

    #[test]
    fn test_nexus_event_to_envelope() {
        let event = NexusEvent::phase_transitioned(PhaseId(0), PhaseId(1));
        let envelope = nexus_event_to_envelope("session-1", &event);

        assert_eq!(envelope.aggregate_id, "session-1");
        assert_eq!(envelope.event_type, "PhaseTransitioned");
    }

    #[test]
    fn test_event_store_with_projection() {
        let store = create_test_store();
        store
            .add_projection(Box::new(PhaseStatisticsProjection::new()))
            .unwrap();

        store
            .append(EventEnvelope::new(
                "s1",
                "session",
                "PhaseTransitioned",
                serde_json::json!({"to": 1, "duration_ms": 100}),
            ))
            .unwrap();

        let state = store.get_projection_state("phase_statistics").unwrap();
        assert!(state.is_some());
        assert_eq!(state.unwrap()["total_transitions"], 1);
    }

    #[test]
    fn test_replay_status_parsing() {
        assert_eq!(
            ReplayStatus::from_str("pending").unwrap(),
            ReplayStatus::Pending
        );
        assert_eq!(
            ReplayStatus::from_str("COMPLETED").unwrap(),
            ReplayStatus::Completed
        );
        assert!(ReplayStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_snapshot_creation() {
        let snapshot = Snapshot::new("agg-1", "session", 10, serde_json::json!({"state": "test"}));

        assert_eq!(snapshot.aggregate_id, "agg-1");
        assert_eq!(snapshot.sequence_number, 10);
    }

    #[test]
    fn test_replay_session_creation() {
        let session = ReplaySession::new("agg-1", 0, 10);

        assert_eq!(session.aggregate_id, "agg-1");
        assert_eq!(session.from_sequence, 0);
        assert_eq!(session.to_sequence, 10);
        assert_eq!(session.status, ReplayStatus::Pending);
    }

    #[test]
    fn test_event_store_events_in_range() {
        let store = create_test_store();
        let now = Utc::now();

        let envelope = EventEnvelope::new("s1", "session", "Test", serde_json::json!({}));
        store.append(envelope).unwrap();

        let from = now - chrono::Duration::seconds(1);
        let to = Utc::now() + chrono::Duration::seconds(1);

        let events = store.get_events_in_range(from, to, 10).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_projection_reset() {
        let mut projection = PhaseStatisticsProjection::new();

        projection
            .handle(&EventEnvelope::new(
                "s1",
                "session",
                "PhaseTransitioned",
                serde_json::json!({"to": 1}),
            ))
            .unwrap();

        assert_eq!(projection.state()["total_transitions"], 1);

        projection.reset();

        assert_eq!(projection.state()["total_transitions"], 0);
    }
}
