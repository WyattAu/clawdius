//! Workspace management and multi-repo support.
//!
//! A workspace groups multiple projects (codebases) under a single
//! entity. Each project has a root path on the filesystem. The agent
//! can read, edit, and search across any mounted project.
//!
//! # Architecture
//!
//! ```text
//! Workspace
//!   ├── Project (default): /home/user/api-server
//!   ├── Project:           /home/user/shared-lib
//!   └── Project:           /home/user/docs
//! ```
//!
//! One project is designated as the "default" — tool calls without an
//! explicit project target the default project.
//!
//! # Layers
//!
//! - **Types** ([`Project`], [`Workspace`], [`ProjectId`], [`WorkspaceId`])
//!   are plain data structs used everywhere.
//! - **[`WorkspaceManager`]** is the service layer that wraps a
//!   [`WorkspaceRepository`](crate::storage::WorkspaceRepository) with
//!   business logic (dedup, auto-naming, path resolution).
//! - **[`ContextAggregator`]** builds per-project context for the LLM.

pub mod aggregator;
pub mod manager;

#[cfg(feature = "vector-db")]
pub mod indexer;

pub use aggregator::{AggregatedContext, ContextAggregator};
pub use manager::{WorkspaceManager, WorkspaceSummary};

#[cfg(feature = "vector-db")]
pub use indexer::{IndexStats, WorkspaceIndexer};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────
// Project
// ─────────────────────────────────────────────────────────

/// Unique identifier for a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProjectId(pub String);

impl ProjectId {
    /// Generate a new unique project ID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A project represents a single codebase mounted in a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    /// Unique identifier.
    pub id: ProjectId,
    /// Human-readable name (e.g., "api-server", "shared-lib").
    pub name: String,
    /// Absolute path to the project root on the filesystem.
    pub root_path: PathBuf,
    /// Timestamp when the project was added to the workspace.
    pub created_at: DateTime<Utc>,
}

impl Project {
    /// Create a new project.
    pub fn new(name: impl Into<String>, root_path: impl Into<PathBuf>) -> Self {
        Self {
            id: ProjectId::new(),
            name: name.into(),
            root_path: root_path.into(),
            created_at: Utc::now(),
        }
    }
}

// ─────────────────────────────────────────────────────────
// Workspace
// ─────────────────────────────────────────────────────────

/// Unique identifier for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub String);

impl WorkspaceId {
    /// Generate a new unique workspace ID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A workspace groups multiple projects for unified multi-repo access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    /// Unique identifier.
    pub id: WorkspaceId,
    /// Human-readable name.
    pub name: String,
    /// The project designated as the default for tool calls.
    pub default_project_id: Option<ProjectId>,
    /// Timestamp when the workspace was created.
    pub created_at: DateTime<Utc>,
}

impl Workspace {
    /// Create a new empty workspace.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: WorkspaceId::new(),
            name: name.into(),
            default_project_id: None,
            created_at: Utc::now(),
        }
    }
}
