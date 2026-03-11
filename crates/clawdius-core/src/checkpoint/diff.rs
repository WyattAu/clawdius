//! Diff utilities

use serde::{Deserialize, Serialize};

/// A diff between two states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    /// Hunks in the diff
    pub hunks: Vec<DiffHunk>,
}

/// A single diff hunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Old file range
    pub old_start: usize,
    pub old_lines: usize,
    /// New file range
    pub new_start: usize,
    pub new_lines: usize,
    /// Lines in the hunk
    pub lines: Vec<DiffLine>,
}

/// A diff line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Line type
    pub ty: DiffLineType,
    /// Content
    pub content: String,
}

/// Type of diff line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
}
