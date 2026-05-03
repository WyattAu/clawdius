use super::SqliteBackend;
use crate::error::Result;
use crate::storage::error::StorageError;
use rusqlite::params;

pub(super) const SCHEMA_VERSION: i32 = 1;

pub(super) const INIT_SQL: &str = r"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    title TEXT,
    provider TEXT,
    model TEXT,
    working_dir TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    extra TEXT NOT NULL DEFAULT '{}',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tokens INTEGER,
    tool_calls TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);

-- Indexes for sessions
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_content ON messages(content);

-- Timeline: tracked files
CREATE TABLE IF NOT EXISTS tracked_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    tracked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Timeline: checkpoints
CREATE TABLE IF NOT EXISTS checkpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    timestamp TEXT NOT NULL,
    files_count INTEGER NOT NULL DEFAULT 0,
    total_size INTEGER NOT NULL DEFAULT 0
);

-- Timeline: file versions
CREATE TABLE IF NOT EXISTS file_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    checksum TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_file_versions_path ON file_versions(path);
CREATE INDEX IF NOT EXISTS idx_file_versions_checkpoint ON file_versions(checkpoint_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_timestamp ON checkpoints(timestamp DESC);

-- Workspace: projects (for multi-repo support)
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Workspace: workspaces (grouping of projects)
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_project_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Workspace: workspace-project membership
CREATE TABLE IF NOT EXISTS workspace_projects (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (workspace_id, project_id)
);

-- Graph: files (for code knowledge graph)
CREATE TABLE IF NOT EXISTS graph_files (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT NOT NULL,
    language TEXT,
    last_modified TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: symbols
CREATE TABLE IF NOT EXISTS graph_symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES graph_files(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    signature TEXT,
    doc_comment TEXT,
    start_line INTEGER,
    end_line INTEGER,
    start_col INTEGER,
    end_col INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: symbol references
CREATE TABLE IF NOT EXISTS graph_symbol_refs (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    file_id INTEGER REFERENCES graph_files(id) ON DELETE CASCADE,
    line INTEGER,
    col INTEGER,
    context TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: relationships
CREATE TABLE IF NOT EXISTS graph_relationships (
    id INTEGER PRIMARY KEY,
    from_symbol INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    to_symbol INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph indexes
CREATE INDEX IF NOT EXISTS idx_graph_symbols_name ON graph_symbols(name);
CREATE INDEX IF NOT EXISTS idx_graph_symbols_kind ON graph_symbols(kind);
CREATE INDEX IF NOT EXISTS idx_graph_symbols_file ON graph_symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_graph_refs_symbol ON graph_symbol_refs(symbol_id);
CREATE INDEX IF NOT EXISTS idx_graph_refs_file ON graph_symbol_refs(file_id);
CREATE INDEX IF NOT EXISTS idx_graph_rels_from ON graph_relationships(from_symbol);
CREATE INDEX IF NOT EXISTS idx_graph_rels_to ON graph_relationships(to_symbol);
CREATE INDEX IF NOT EXISTS idx_graph_rels_type ON graph_relationships(relationship_type);
";

impl SqliteBackend {
    pub fn initialize(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch(INIT_SQL)
                .map_err(|e| StorageError::Migration {
                    reason: e.to_string(),
                })?;
            conn.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )
            .map_err(|e| StorageError::Migration {
                reason: e.to_string(),
            })?;
            Ok(())
        })
    }
}
