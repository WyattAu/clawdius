//! File watcher with debouncing and pattern filtering

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::{EventKind, Watcher as NotifyWatcher};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}

/// File watcher
pub struct FileWatcher {
    config: WatchConfig,
    watcher: Option<Box<dyn NotifyWatcher>>,
    pending_events: HashSet<PathBuf>,
    last_event_time: Option<Instant>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(config: WatchConfig) -> Result<Self, WatchError> {
        Ok(Self {
            config,
            watcher: None,
            pending_events: HashSet::new(),
            last_event_time: None,
        })
    }

    /// Start watching
    pub fn start(&mut self) -> Result<(), WatchError> {
        // Validate paths
        for path in &self.config.paths {
            if !path.exists() {
                return Err(WatchError::PathNotFound(path.clone()));
            }
        }

        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        self.watcher = None;
    }

    /// Process events with debouncing
    pub fn process_events(&mut self, events: Vec<notify::Event>) -> Vec<WatchEvent> {
        let now = Instant::now();
        let debounce_duration = Duration::from_millis(self.config.debounce_ms);

        // Check if enough time has passed since last event
        if let Some(last_time) = self.last_event_time {
            if now.duration_since(last_time) < debounce_duration {
                // Still debouncing, accumulate paths
                for event in events {
                    for path in event.paths {
                        if self.config.should_watch(&path) {
                            self.pending_events.insert(path);
                        }
                    }
                }
                return Vec::new();
            }
        }

        // Process accumulated events
        let mut watch_events = Vec::new();

        for path in &self.pending_events {
            if path.exists() {
                watch_events.push(WatchEvent::Modified { path: path.clone() });
            } else {
                watch_events.push(WatchEvent::Deleted { path: path.clone() });
            }
        }

        // Process new events
        for event in events {
            for path in event.paths {
                if self.config.should_watch(&path) {
                    let watch_event = match event.kind {
                        EventKind::Create(_) => WatchEvent::Created { path: path.clone() },
                        EventKind::Modify(_) => WatchEvent::Modified { path: path.clone() },
                        EventKind::Remove(_) => WatchEvent::Deleted { path: path.clone() },
                        _ => continue,
                    };
                    watch_events.push(watch_event);
                }
            }
        }

        self.pending_events.clear();
        self.last_event_time = Some(now);

        watch_events
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &WatchConfig {
        &self.config
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
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
}
