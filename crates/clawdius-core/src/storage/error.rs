//! Storage error types

use std::path::PathBuf;

/// Storage-specific errors that map to crate-level errors.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The requested record was not found.
    #[error("record not found: {kind} id={id}")]
    NotFound { kind: &'static str, id: String },

    /// A unique constraint was violated (duplicate key).
    #[error("unique constraint violated: {table} column={column} value={value}")]
    UniqueViolation {
        table: &'static str,
        column: &'static str,
        value: String,
    },

    /// A foreign key constraint was violated.
    #[error("foreign key constraint violated: {table} references={references}")]
    ForeignKeyViolation { table: &'static str, references: &'static str },

    /// The database connection failed or was lost.
    #[error("connection error: {0}")]
    Connection(String),

    /// A query execution failed.
    #[error("query error: {statement} — {reason}")]
    Query { statement: String, reason: String },

    /// A transaction failed.
    #[error("transaction error: {0}")]
    Transaction(String),

    /// A migration failed.
    #[error("migration error: {reason}")]
    Migration { reason: String },

    /// The storage backend is not configured.
    #[error("storage not configured")]
    NotConfigured,

    /// An I/O error occurred (file system operations for snapshots).
    #[error("io error: {path}: {reason}")]
    Io { path: PathBuf, reason: String },

    /// Row deserialization failed.
    #[error("row conversion error: {reason}")]
    RowConversion { reason: String },
}

impl StorageError {
    /// Create a not-found error for a session.
    pub fn session_not_found(id: impl std::fmt::Display) -> Self {
        Self::NotFound { kind: "session", id: id.to_string() }
    }

    /// Create a not-found error for a checkpoint.
    pub fn checkpoint_not_found(id: impl std::fmt::Display) -> Self {
        Self::NotFound { kind: "checkpoint", id: id.to_string() }
    }
}

#[cfg(feature = "postgres")]
impl From<tokio_postgres::Error> for StorageError {
    fn from(err: tokio_postgres::Error) -> Self {
        let msg = err.to_string();
        if let Some(db_err) = err.as_db_error() {
            if db_err.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION {
                return Self::UniqueViolation {
                    table: "unknown",
                    column: "unknown",
                    value: msg,
                };
            }
            if db_err.code() == &tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION {
                return Self::ForeignKeyViolation {
                    table: "unknown",
                    references: "unknown",
                };
            }
        }
        Self::Query {
            statement: "unknown".to_string(),
            reason: msg,
        }
    }
}

#[cfg(feature = "mariadb")]
impl From<mysql_async::Error> for StorageError {
    fn from(err: mysql_async::Error) -> Self {
        let msg = err.to_string();
        Self::Query {
            statement: "unknown".to_string(),
            reason: msg,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        let msg = err.to_string();
        match err {
            rusqlite::Error::QueryReturnedNoRows => Self::NotFound {
                kind: "record",
                id: "unknown".to_string(),
            },
            other => Self::Query {
                statement: "unknown".to_string(),
                reason: msg,
            },
        }
    }
}

impl From<StorageError> for crate::error::Error {
    fn from(err: StorageError) -> Self {
        Self::Checkpoint(err.to_string())
    }
}
