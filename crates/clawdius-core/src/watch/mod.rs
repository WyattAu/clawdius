//! File watching and IDE integration
//!
//! Provides real-time file watching capabilities for IDE integration.
//! Monitors file changes and triggers context updates, diagnostics, and completions.
//!
//! # Features
//!
//! - Real-time file change detection
//! - Debounced events for performance
//! - Pattern-based filtering (ignore .git, target, etc.)
//! - Integration with context system for live updates
//! - IDE-agnostic event stream

// Unwrap purge batch 4: watch module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod debounce;
pub mod handlers;
mod watcher;

pub use debounce::{DebounceConfig, DebouncedEvent};
pub use handlers::{ContextUpdateHandler, DiagnosticHandler, WatchHandler};
pub use watcher::{FileWatcher, WatchConfig, WatchEvent};
