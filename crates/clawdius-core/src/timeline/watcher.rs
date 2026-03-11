//! File watching for automatic checkpointing
//!
//! Monitors file changes and creates automatic checkpoints.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

use crate::error::{Error, Result};

/// File watcher configuration
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce interval (don't create checkpoint more often than this)
    pub debounce_interval: Duration,

    /// File patterns to ignore
    pub ignore_patterns: Vec<String>,

    /// Maximum checkpoints per hour
    pub max_checkpoints_per_hour: usize,

    /// Auto-checkpoint on file save
    pub auto_checkpoint: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_interval: Duration::from_secs(30),
            ignore_patterns: vec![
                ".git/".to_string(),
                "target/".to_string(),
                "node_modules/".to_string(),
                ".clawdius/".to_string(),
                "*.swp".to_string(),
                "*.swo".to_string(),
                "*~".to_string(),
                "*.lock".to_string(),
            ],
            max_checkpoints_per_hour: 120,
            auto_checkpoint: true,
        }
    }
}

/// File change event
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub paths: Vec<PathBuf>,
    pub kind: ChangeKind,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
    Any,
}

impl From<EventKind> for ChangeKind {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::Create(_) => ChangeKind::Created,
            EventKind::Modify(_) => ChangeKind::Modified,
            EventKind::Remove(_) => ChangeKind::Deleted,
            _ => ChangeKind::Any,
        }
    }
}

/// File watcher for timeline auto-checkpointing
pub struct FileWatcher {
    config: WatcherConfig,
    workspace_root: PathBuf,
    last_checkpoint: Arc<RwLock<Option<Instant>>>,
    checkpoint_count: Arc<RwLock<usize>>,
    hour_start: Arc<RwLock<Option<Instant>>>,
    shutdown: Arc<RwLock<bool>>,
}

impl FileWatcher {
    /// Create new file watcher
    pub fn new(workspace_root: PathBuf, config: WatcherConfig) -> Self {
        Self {
            config,
            workspace_root,
            last_checkpoint: Arc::new(RwLock::new(None)),
            checkpoint_count: Arc::new(RwLock::new(0)),
            hour_start: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start watching a directory with a callback for auto-checkpointing
    pub async fn watch<F, Fut>(&self, callback: F) -> Result<()>
    where
        F: Fn(Vec<PathBuf>, ChangeKind) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let (tx, mut rx) = mpsc::channel::<FileChangeEvent>(100);

        let workspace_root = self.workspace_root.clone();
        let ignore_patterns = self.config.ignore_patterns.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if !event.paths.is_empty() {
                        let change = FileChangeEvent {
                            paths: event.paths.clone(),
                            kind: ChangeKind::from(event.kind),
                            timestamp: Instant::now(),
                        };

                        if !Self::should_ignore_paths(
                            &event.paths,
                            &ignore_patterns,
                            &workspace_root,
                        ) {
                            if let Err(e) = tx.blocking_send(change) {
                                tracing::error!("Failed to send file change event: {}", e);
                            }
                        }
                    }
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        watcher
            .watch(&self.workspace_root, RecursiveMode::Recursive)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let callback = Arc::new(callback);
        let last_checkpoint = Arc::clone(&self.last_checkpoint);
        let checkpoint_count = Arc::clone(&self.checkpoint_count);
        let hour_start = Arc::clone(&self.hour_start);
        let debounce_interval = self.config.debounce_interval;
        let max_checkpoints_per_hour = self.config.max_checkpoints_per_hour;
        let auto_checkpoint = self.config.auto_checkpoint;
        let shutdown = Arc::clone(&self.shutdown);

        tokio::spawn(async move {
            while !*shutdown.read().await {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        if auto_checkpoint && Self::should_create_checkpoint_internal(
                            &last_checkpoint,
                            &checkpoint_count,
                            &hour_start,
                            debounce_interval,
                            max_checkpoints_per_hour,
                        ).await {
                            let relevant_paths: Vec<PathBuf> = event.paths;

                            if !relevant_paths.is_empty() {
                                match callback(relevant_paths.clone(), event.kind).await {
                                    Ok(()) => {
                                        *last_checkpoint.write().await = Some(Instant::now());
                                        *checkpoint_count.write().await += 1;
                                    }
                                    Err(e) => {
                                        tracing::error!("Auto-checkpoint callback error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check for shutdown periodically
                    }
                }
            }
        });

        Ok(())
    }

    /// Check if paths should be ignored
    fn should_ignore_paths(paths: &[PathBuf], patterns: &[String], workspace_root: &Path) -> bool {
        for path in paths {
            if Self::should_ignore_path(path, patterns, workspace_root) {
                return true;
            }
        }
        false
    }

    /// Check if a single path should be ignored
    fn should_ignore_path(path: &Path, patterns: &[String], workspace_root: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let relative_path = path.strip_prefix(workspace_root).unwrap_or(path);
        let relative_str = relative_path.to_string_lossy();

        for pattern in patterns {
            if pattern.ends_with('/') {
                let dir_pattern = &pattern[..pattern.len() - 1];
                if path_str.contains(dir_pattern) || relative_str.starts_with(dir_pattern) {
                    return true;
                }
            } else if pattern.starts_with('*') {
                let suffix = &pattern[1..];
                if path_str.ends_with(suffix) || relative_str.ends_with(suffix) {
                    return true;
                }
            } else if path_str.contains(pattern) || relative_str.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Internal check if checkpoint should be created
    async fn should_create_checkpoint_internal(
        last_checkpoint: &Arc<RwLock<Option<Instant>>>,
        checkpoint_count: &Arc<RwLock<usize>>,
        hour_start: &Arc<RwLock<Option<Instant>>>,
        debounce_interval: Duration,
        max_checkpoints_per_hour: usize,
    ) -> bool {
        let last = last_checkpoint.read().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < debounce_interval {
                return false;
            }
        }

        let mut hour = hour_start.write().await;
        let now = Instant::now();

        if let Some(start) = *hour {
            if now.duration_since(start) >= Duration::from_secs(3600) {
                *hour = Some(now);
                *checkpoint_count.write().await = 0;
            }
        } else {
            *hour = Some(now);
        }

        let count = *checkpoint_count.read().await;
        if count >= max_checkpoints_per_hour {
            return false;
        }

        true
    }

    /// Check if path should be ignored (public for testing)
    pub fn should_ignore(&self, path: &Path) -> bool {
        Self::should_ignore_path(path, &self.config.ignore_patterns, &self.workspace_root)
    }

    /// Stop watching
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Check if watcher is running
    pub async fn is_running(&self) -> bool {
        !*self.shutdown.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_interval, Duration::from_secs(30));
        assert!(config.auto_checkpoint);
        assert!(config.max_checkpoints_per_hour > 0);
    }

    #[test]
    fn test_should_ignore_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(workspace_root.clone(), config);

        assert!(watcher.should_ignore(&workspace_root.join(".git/config")));
        assert!(watcher.should_ignore(&workspace_root.join("target/debug/test")));
        assert!(watcher.should_ignore(&workspace_root.join("test.swp")));
        assert!(watcher.should_ignore(&workspace_root.join("node_modules/package")));
        assert!(watcher.should_ignore(&workspace_root.join("Cargo.lock")));
        assert!(!watcher.should_ignore(&workspace_root.join("src/main.rs")));
        assert!(!watcher.should_ignore(&workspace_root.join("lib/test.rs")));
    }

    #[test]
    fn test_change_kind_from_event_kind() {
        use notify::EventKind as NotifyEventKind;

        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Create(notify::event::CreateKind::Any)),
            ChangeKind::Created
        ));
        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Modify(notify::event::ModifyKind::Any)),
            ChangeKind::Modified
        ));
        assert!(matches!(
            ChangeKind::from(NotifyEventKind::Remove(notify::event::RemoveKind::Any)),
            ChangeKind::Deleted
        ));
    }

    #[tokio::test]
    async fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        assert!(!*watcher.shutdown.read().await);
        assert!(watcher.last_checkpoint.read().await.is_none());
    }

    #[tokio::test]
    async fn test_stop_watching() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(temp_dir.path().to_path_buf(), config);

        assert!(!*watcher.shutdown.read().await);
        watcher.stop().await;
        assert!(*watcher.shutdown.read().await);
    }
}
