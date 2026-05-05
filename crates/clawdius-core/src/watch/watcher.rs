//! File watcher with debouncing and pattern filtering
//!
//! Uses the `notify` crate for cross-platform file system watching.
//! Events are debounced to coalesce rapid changes (e.g., save-with-backup).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecursiveMode, Watcher as NotifyWatcher};
use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::debounce::{DebouncedEvent, DebouncedEventKind, EventDebouncer};

/// File watcher error
#[derive(Debug, Error)]
pub enum WatchError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Notify error
    #[error("Watch error: {0}")]
    Notify(#[from] notify::Error),
    /// Path not found
    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),
    /// Channel send error
    #[error("Channel error: {0}")]
    Channel(String),
}

/// Watch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Paths to watch
    pub paths: Vec<PathBuf>,
    /// File patterns to include (glob patterns)
    pub include_patterns: Vec<String>,
    /// File patterns to exclude (glob patterns)
    pub exclude_patterns: Vec<String>,
    /// Debounce interval in milliseconds
    pub debounce_ms: u64,
    /// Watch recursively
    pub recursive: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from(".")],
            include_patterns: vec!["**/*.rs".into(), "**/*.toml".into()],
            exclude_patterns: vec![
                "**/target/**".into(),
                "**/.git/**".into(),
                "**/node_modules/**".into(),
                "**/.clawdius/**".into(),
            ],
            debounce_ms: 100,
            recursive: true,
        }
    }
}

impl WatchConfig {
    /// Create a new watch config for a single path
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            paths: vec![path.into()],
            ..Default::default()
        }
    }

    /// Add an include pattern
    #[must_use]
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add an exclude pattern
    #[must_use]
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Set debounce interval
    #[must_use]
    pub fn debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Check if a path should be watched
    #[must_use]
    pub fn should_watch(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclude patterns first
        for pattern in &self.exclude_patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return false;
            }
        }

        // Check include patterns
        for pattern in &self.include_patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return true;
            }
        }

        false
    }
}

/// Watch event type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchEvent {
    /// File created
    Created {
        /// Path to the file
        path: PathBuf,
    },
    /// File modified
    Modified {
        /// Path to the file
        path: PathBuf,
    },
    /// File deleted
    Deleted {
        /// Path to the file
        path: PathBuf,
    },
    /// File renamed
    Renamed {
        /// Old path
        from: PathBuf,
        /// New path
        to: PathBuf,
    },
}

impl WatchEvent {
    /// Get the primary path for this event
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Created { path } | Self::Modified { path } | Self::Deleted { path } => path,
            Self::Renamed { to, .. } => to,
        }
    }

    /// Get a short label for the event type
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Created { .. } => "CREATED",
            Self::Modified { .. } => "MODIFIED",
            Self::Deleted { .. } => "DELETED",
            Self::Renamed { .. } => "RENAMED",
        }
    }
}

/// File watcher
///
/// Wraps `notify::RecommendedWatcher` and provides:
/// - Pattern-based filtering (include/exclude globs)
/// - Debounced events via internal `EventDebouncer`
/// - Channel-based event delivery
pub struct FileWatcher {
    config: WatchConfig,
    watcher: Option<RecommendedWatcher>,
    /// Maps paths to their last known state to detect renames
    known_paths: HashMap<PathBuf, Instant>,
}

impl FileWatcher {
    /// Create a new file watcher (does not start watching).
    pub fn new(config: WatchConfig) -> Result<Self, WatchError> {
        Ok(Self {
            config,
            watcher: None,
            known_paths: HashMap::new(),
        })
    }

    /// Start watching and return a receiver for debounced events.
    ///
    /// The returned `std::sync::mpsc::Receiver` yields batches of
    /// `WatchEvent` after debouncing. The caller should poll in a loop
    /// (e.g., `recv()` or `recv_timeout()`).
    ///
    /// Returns `(watcher, receiver)`. The watcher must be kept alive;
    /// dropping it stops watching.
    pub fn start_with_channel(
        config: WatchConfig,
    ) -> Result<(Self, std_mpsc::Receiver<Vec<WatchEvent>>), WatchError> {
        let (out_tx, out_rx) = std_mpsc::channel::<Vec<WatchEvent>>();

        // Shared state: debouncer + flush timer, protected by Mutex for the notify callback
        let state = Arc::new(Mutex::new(DebounceState::new(config.clone())));

        let state_for_callback = Arc::clone(&state);
        let out_tx_for_callback = out_tx.clone();

        // The notify callback runs on a background thread managed by notify.
        // It accumulates events into the debouncer and sends batches via out_tx.
        let callback = move |res: Result<Event, notify::Error>| {
            let Ok(event) = res else {
                return;
            };

            let mut state = state_for_callback.lock().unwrap_or_else(|e| e.into_inner());

            for path in event.paths {
                if !state.config.should_watch(&path) {
                    continue;
                }

                let kind = match event.kind {
                    EventKind::Create(_) => DebouncedEventKind::Created,
                    EventKind::Modify(_) => DebouncedEventKind::Modified,
                    EventKind::Remove(_) => DebouncedEventKind::Deleted,
                    _ => continue,
                };

                state.debouncer.add(path, kind);
            }

            // Flush if we've accumulated enough events
            if state.debouncer.should_flush() {
                let batch = flush_to_watch_events(&mut state.debouncer);
                if !batch.is_empty() {
                    // Ignore send error — receiver may have been dropped
                    let _ = out_tx_for_callback.send(batch);
                }
            }
        };

        // Create the notify watcher
        let mut watcher = notify::recommended_watcher(callback)?;

        // Register all configured paths
        for path in &config.paths {
            let canonical = if path.exists() {
                path.canonicalize()
                    .map_err(|_| WatchError::PathNotFound(path.clone()))?
            } else {
                return Err(WatchError::PathNotFound(path.clone()));
            };

            let mode = if config.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            watcher.watch(&canonical, mode)?;
            tracing::debug!(path = ?canonical, "Watching path");
        }

        // Spawn a timer thread that periodically flushes the debouncer.
        // This ensures events are delivered even if no new events arrive.
        let debounce_ms = config.debounce_ms;
        let state_for_timer = Arc::clone(&state);
        let out_tx_for_timer = out_tx;

        std::thread::Builder::new()
            .name("clawdius-watch-flush".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(debounce_ms));

                    let mut state = match state_for_timer.lock() {
                        Ok(s) => s,
                        Err(e) => e.into_inner(),
                    };

                    if state.debouncer.pending_count() > 0 {
                        let batch = flush_to_watch_events(&mut state.debouncer);
                        if !batch.is_empty() {
                            if out_tx_for_timer.send(batch).is_err() {
                                // Receiver dropped — stop the timer
                                return;
                            }
                        }
                    }
                }
            })
            .map_err(|e| WatchError::Channel(e.to_string()))?;

        Ok((
            Self {
                config,
                watcher: Some(watcher),
                known_paths: HashMap::new(),
            },
            out_rx,
        ))
    }

    /// Stop watching
    pub fn stop(&mut self) {
        if let Some(mut w) = self.watcher.take() {
            for path in &self.config.paths {
                let _ = w.unwatch(path);
            }
        }
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &WatchConfig {
        &self.config
    }

    /// Get the number of currently tracked paths
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.known_paths.len()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Internal state shared between the notify callback and the flush timer.
struct DebounceState {
    config: WatchConfig,
    debouncer: EventDebouncer,
}

impl DebounceState {
    fn new(config: WatchConfig) -> Self {
        Self {
            debouncer: EventDebouncer::new(super::debounce::DebounceConfig {
                min_interval_ms: config.debounce_ms / 2,
                max_wait_ms: config.debounce_ms,
                max_batch_size: 100,
            }),
            config,
        }
    }
}

/// Convert debounced events into `WatchEvent` batch and flush the debouncer.
fn flush_to_watch_events(debouncer: &mut EventDebouncer) -> Vec<WatchEvent> {
    let debounced = debouncer.flush();

    let mut events = Vec::with_capacity(debounced.len());
    for de in debounced {
        let event = match de.kind {
            DebouncedEventKind::Created => WatchEvent::Created { path: de.path },
            DebouncedEventKind::Modified => WatchEvent::Modified { path: de.path },
            DebouncedEventKind::Deleted => WatchEvent::Deleted { path: de.path },
            DebouncedEventKind::Any => {
                // Determine actual state: if path exists now, it was modified; otherwise deleted
                if de.path.exists() {
                    WatchEvent::Modified { path: de.path }
                } else {
                    WatchEvent::Deleted { path: de.path }
                }
            },
        };
        events.push(event);
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_default() {
        let config = WatchConfig::default();
        assert!(!config.paths.is_empty());
        assert!(config.recursive);
    }

    #[test]
    fn test_watch_config_should_watch() {
        let config = WatchConfig::new(".").include("**/*.rs");

        assert!(config.should_watch(Path::new("src/main.rs")));
        assert!(!config.should_watch(Path::new("target/debug/main")));
    }

    #[test]
    fn test_watch_config_exclude() {
        let config = WatchConfig::new(".").exclude("**/target/**");

        assert!(!config.should_watch(Path::new("target/debug/main.rs")));
        assert!(!config.should_watch(Path::new("crates/foo/target/debug/lib.rs")));
    }

    #[test]
    fn test_watch_event_path() {
        let created = WatchEvent::Created {
            path: PathBuf::from("/tmp/foo.rs"),
        };
        assert_eq!(created.path(), Path::new("/tmp/foo.rs"));

        let renamed = WatchEvent::Renamed {
            from: PathBuf::from("/tmp/old.rs"),
            to: PathBuf::from("/tmp/new.rs"),
        };
        assert_eq!(renamed.path(), Path::new("/tmp/new.rs"));
    }

    #[test]
    fn test_watch_event_label() {
        assert_eq!(
            WatchEvent::Created {
                path: PathBuf::from("a.rs")
            }
            .label(),
            "CREATED"
        );
        assert_eq!(
            WatchEvent::Modified {
                path: PathBuf::from("a.rs")
            }
            .label(),
            "MODIFIED"
        );
        assert_eq!(
            WatchEvent::Deleted {
                path: PathBuf::from("a.rs")
            }
            .label(),
            "DELETED"
        );
        assert_eq!(
            WatchEvent::Renamed {
                from: PathBuf::from("a.rs"),
                to: PathBuf::from("b.rs")
            }
            .label(),
            "RENAMED"
        );
    }

    #[test]
    fn test_flush_to_watch_events() {
        let mut debouncer = EventDebouncer::default();
        debouncer.add(
            PathBuf::from("existing.rs"),
            DebouncedEventKind::Modified,
        );

        let events = flush_to_watch_events(&mut debouncer);
        assert_eq!(events.len(), 1);
    }
}
