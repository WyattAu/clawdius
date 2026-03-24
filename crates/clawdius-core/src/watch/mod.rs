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

mod debounce;
pub mod handlers;
mod watcher;

pub use debounce::{DebounceConfig, DebouncedEvent};
pub use handlers::{ContextUpdateHandler, DiagnosticHandler, WatchHandler};
pub use watcher::{FileWatcher, WatchConfig, WatchEvent};
