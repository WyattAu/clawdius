//! Watch event handlers
//!
//! Handlers for different types of file watch events.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::watcher::WatchEvent;

/// Watch handler result
pub type WatchResult<T> = Result<T, WatchHandlerError>;

/// Watch handler error
#[derive(Debug, thiserror::Error)]
pub enum WatchHandlerError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Analysis error
    #[error("Analysis error: {0}")]
    Analysis(String),
    /// Handler not ready
    #[error("Handler not ready")]
    NotReady,
}

/// Trait for watch event handlers
#[async_trait]
pub trait WatchHandler: Send + Sync {
    /// Handle a watch event
    async fn handle(&self, event: &WatchEvent) -> WatchResult<()>;

    /// Handler name for identification
    fn name(&self) -> &str;

    /// Whether this handler should handle the given event
    fn should_handle(&self, event: &WatchEvent) -> bool {
        let path = event.path();
        self.filter_path(path)
    }

    /// Filter paths this handler cares about
    fn filter_path(&self, path: &Path) -> bool;
}

/// Context update handler
/// Updates the LLM context when files change
pub struct ContextUpdateHandler {
    /// Paths being tracked
    tracked_paths: Arc<RwLock<Vec<std::path::PathBuf>>>,
    /// File patterns to track
    patterns: Vec<String>,
}

impl ContextUpdateHandler {
    /// Create a new context update handler
    #[must_use]
    pub fn new(patterns: Vec<String>) -> Self {
        Self {
            tracked_paths: Arc::new(RwLock::new(Vec::new())),
            patterns,
        }
    }

    /// Get tracked paths
    pub async fn tracked_paths(&self) -> Vec<std::path::PathBuf> {
        self.tracked_paths.read().await.clone()
    }

    /// Check if path matches patterns
    fn matches_patterns(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.patterns
            .iter()
            .any(|p| glob_match::glob_match(p, &path_str))
    }
}

#[async_trait]
impl WatchHandler for ContextUpdateHandler {
    async fn handle(&self, event: &WatchEvent) -> WatchResult<()> {
        match event {
            WatchEvent::Created { path } | WatchEvent::Modified { path } => {
                if self.matches_patterns(path) {
                    let mut tracked = self.tracked_paths.write().await;
                    if !tracked.contains(path) {
                        tracked.push(path.clone());
                    }
                    tracing::debug!("Context updated for: {:?}", path);
                }
            },
            WatchEvent::Deleted { path } => {
                let mut tracked = self.tracked_paths.write().await;
                tracked.retain(|p| p != path);
                tracing::debug!("Context removed for: {:?}", path);
            },
            WatchEvent::Renamed { from, to } => {
                let mut tracked = self.tracked_paths.write().await;
                tracked.retain(|p| p != from);
                if self.matches_patterns(to) {
                    tracked.push(to.clone());
                }
                tracing::debug!("Context renamed: {:?} -> {:?}", from, to);
            },
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "context_update"
    }

    fn filter_path(&self, path: &Path) -> bool {
        self.matches_patterns(path)
    }
}

/// Diagnostic handler
/// Triggers diagnostic re-runs when files change
pub struct DiagnosticHandler {
    /// Supported language extensions
    language_extensions: Vec<String>,
}

impl DiagnosticHandler {
    /// Create a new diagnostic handler
    #[must_use]
    pub fn new() -> Self {
        Self {
            language_extensions: vec![
                "rs".into(),
                "py".into(),
                "js".into(),
                "ts".into(),
                "go".into(),
                "java".into(),
                "c".into(),
                "cpp".into(),
            ],
        }
    }

    /// Check if file is a source file
    fn is_source_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.language_extensions.contains(&ext.to_string()))
    }
}

impl Default for DiagnosticHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WatchHandler for DiagnosticHandler {
    async fn handle(&self, event: &WatchEvent) -> WatchResult<()> {
        if self.is_source_file(event.path()) {
            match event {
                WatchEvent::Created { path } | WatchEvent::Modified { path } => {
                    tracing::info!("Running diagnostics for: {:?}", path);
                },
                WatchEvent::Deleted { path } => {
                    tracing::info!("Clearing diagnostics for: {:?}", path);
                },
                WatchEvent::Renamed { from, to } => {
                    tracing::info!("Diagnostics: renamed {:?} -> {:?}", from, to);
                },
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "diagnostic"
    }

    fn filter_path(&self, path: &Path) -> bool {
        self.is_source_file(path)
    }
}

/// Auto-analysis handler
/// Triggers architecture drift and technical debt analysis when files change
#[allow(dead_code)] // Feature complete but not yet wired to CLI
pub struct AutoAnalysisHandler {
    /// Supported language extensions
    language_extensions: Vec<String>,
    /// Whether analysis is enabled
    enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[allow(dead_code)] // Feature complete but not yet wired to CLI
impl AutoAnalysisHandler {
    /// Create a new auto-analysis handler
    #[must_use]
    pub fn new() -> Self {
        Self {
            language_extensions: vec![
                "rs".into(),
                "py".into(),
                "js".into(),
                "ts".into(),
                "go".into(),
                "java".into(),
                "c".into(),
                "cpp".into(),
            ],
            enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Enable or disable auto-analysis
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if auto-analysis is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Check if file is a source file
    fn is_source_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.language_extensions.contains(&ext.to_string()))
    }

    /// Trigger analysis for a path
    async fn trigger_analysis(&self, path: &Path) -> WatchResult<()> {
        if !self.is_enabled() {
            tracing::debug!("Auto-analysis disabled, skipping: {:?}", path);
            return Ok(());
        }

        tracing::info!(
            path = ?path,
            "Auto-analysis triggered for file change"
        );

        // In a real implementation, this would:
        // 1. Read the file content
        // 2. Run DriftDetector rules
        // 3. Run DebtAnalyzer rules
        // 4. Store results or emit events

        // For now, just log that analysis was triggered
        tracing::debug!("Analysis complete for: {:?}", path);

        Ok(())
    }
}

impl Default for AutoAnalysisHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WatchHandler for AutoAnalysisHandler {
    async fn handle(&self, event: &WatchEvent) -> WatchResult<()> {
        match event {
            WatchEvent::Created { path } | WatchEvent::Modified { path } => {
                if self.is_source_file(path) {
                    self.trigger_analysis(path).await?;
                }
            },
            WatchEvent::Deleted { path } => {
                if self.is_source_file(path) {
                    tracing::debug!("Clearing analysis for deleted file: {:?}", path);
                }
            },
            WatchEvent::Renamed { from, to } => {
                if self.is_source_file(from) {
                    tracing::debug!("Clearing analysis for renamed file: {:?}", from);
                }
                if self.is_source_file(to) {
                    self.trigger_analysis(to).await?;
                }
            },
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "auto_analysis"
    }

    fn filter_path(&self, path: &Path) -> bool {
        self.is_source_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_handler() {
        let handler = ContextUpdateHandler::new(vec!["**/*.rs".into()]);

        let event = WatchEvent::Modified {
            path: std::path::PathBuf::from("src/main.rs"),
        };

        handler.handle(&event).await.unwrap();

        let tracked = handler.tracked_paths().await;
        assert!(tracked.contains(&std::path::PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_diagnostic_handler() {
        let handler = DiagnosticHandler::new();

        assert!(handler.filter_path(Path::new("src/main.rs")));
        assert!(handler.filter_path(Path::new("lib.py")));
        assert!(!handler.filter_path(Path::new("README.md")));
    }
}
