//! Checkpoint system for workspace snapshots

mod diff;
mod manager;
mod snapshot;

pub use diff::{Diff, DiffHunk, DiffLine, DiffLineType};
pub use manager::{
    Checkpoint, CheckpointDiff, CheckpointManager, CheckpointSummary, FileChange, Timeline,
};
pub use snapshot::{FileSnapshot, Snapshot, SnapshotManager};
