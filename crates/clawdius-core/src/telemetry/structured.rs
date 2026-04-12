//! Structured Telemetry for SaaS
//!
//! Provides JSON logging, session-scoped tracing, and time-travel debugging
//! by integrating with the `tracing` ecosystem.
//!
//! # Architecture
//!
//! ```text
//!   Agent Action ──→ tracing::info!(session_id, task_id, "message")
//!                       │
//!                  TelemetryLayer
//!                       │
//!              ┌────────┼────────┐
//!              ▼        ▼        ▼
//!           Console   JSON File  External Collector
//!           (dev)     (local)   (Axiom/Datadog/Loki)
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use clawdius_core::telemetry::structured::TelemetryLayer;
//!
//! let layer = TelemetryLayer::new("my-app".to_string())
//!     .with_json_output("/var/log/clawdius")
//!     .with_external_endpoint("https://axiom.example.com/logs");
//!
//! layer.init()?;
//! ```

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

/// A session-scoped telemetry event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Session ID
    pub session_id: String,
    /// Task ID (if applicable)
    pub task_id: Option<String>,
    /// Tenant ID
    pub tenant_id: Option<String>,
    /// Worker ID
    pub worker_id: Option<String>,
    /// Event level (trace/debug/info/warn/error)
    pub level: String,
    /// Component that generated the event
    pub component: String,
    /// Event message
    pub message: String,
    /// Structured fields
    pub fields: HashMap<String, serde_json::Value>,
    /// Duration in milliseconds (if applicable)
    pub duration_ms: Option<u64>,
}

impl TelemetryEvent {
    /// Creates a new telemetry event.
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        level: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.into(),
            task_id: None,
            tenant_id: None,
            worker_id: None,
            level: level.into(),
            component: String::new(),
            message: message.into(),
            fields: HashMap::new(),
            duration_ms: None,
        }
    }

    /// Sets the task ID.
    #[must_use]
    pub fn with_task(mut self, id: impl Into<String>) -> Self {
        self.task_id = Some(id.into());
        self
    }

    /// Sets the tenant ID.
    #[must_use]
    pub fn with_tenant(mut self, id: impl Into<String>) -> Self {
        self.tenant_id = Some(id.into());
        self
    }

    /// Sets the worker ID.
    #[must_use]
    pub fn with_worker(mut self, id: impl Into<String>) -> Self {
        self.worker_id = Some(id.into());
        self
    }

    /// Sets the component.
    #[must_use]
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = component.into();
        self
    }

    /// Adds a structured field.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Sets the duration.
    #[must_use]
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Converts to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| crate::error::Error::Serialization(e))
    }
}

/// Configuration for the structured telemetry layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredTelemetryConfig {
    /// Application name (included in all logs)
    pub app_name: String,
    /// Log output format
    pub format: LogFormat,
    /// Log level (default: info)
    pub level: String,
    /// Directory for JSON log files (None = no file output)
    pub log_dir: Option<PathBuf>,
    /// External log collector endpoint (Axiom, Datadog, Loki)
    pub external_endpoint: Option<String>,
    /// External collector API key
    pub external_api_key: Option<String>,
    /// Maximum log file size in bytes before rotation
    pub max_file_size: u64,
    /// Maximum number of rotated log files to keep
    pub max_files: usize,
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogFormat {
    /// Human-readable text format (dev)
    #[default]
    Text,
    /// JSON format (production)
    Json,
    /// Compact JSON (one-line per event)
    CompactJson,
}

impl Default for StructuredTelemetryConfig {
    fn default() -> Self {
        Self {
            app_name: "clawdius".to_string(),
            format: LogFormat::Text,
            level: "info".to_string(),
            log_dir: None,
            external_endpoint: None,
            external_api_key: None,
            max_file_size: 100 * 1024 * 1024, // 100 MB
            max_files: 5,
        }
    }
}

/// Manages structured telemetry for the application.
pub struct TelemetryLayer {
    /// Configuration
    config: StructuredTelemetryConfig,
    /// Active session context
    session_context: RwLock<SessionContext>,
    /// Whether the layer has been initialized
    initialized: std::sync::atomic::AtomicBool,
}

/// Session context propagated through all telemetry events.
#[derive(Debug, Clone, Default)]
struct SessionContext {
    /// Current session ID
    session_id: Option<String>,
    /// Current task ID
    task_id: Option<String>,
    /// Current tenant ID
    tenant_id: Option<String>,
    /// Current worker ID
    worker_id: Option<String>,
}

impl TelemetryLayer {
    /// Creates a new telemetry layer.
    #[must_use]
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            config: StructuredTelemetryConfig {
                app_name: app_name.into(),
                ..Default::default()
            },
            session_context: RwLock::new(SessionContext::default()),
            initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Sets the log output format.
    #[must_use]
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.config.format = format;
        self
    }

    /// Enables JSON file output to the given directory.
    #[must_use]
    pub fn with_json_output(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.log_dir = Some(dir.into());
        self.config.format = LogFormat::Json;
        self
    }

    /// Sets an external log collector endpoint.
    #[must_use]
    pub fn with_external_endpoint(mut self, url: impl Into<String>) -> Self {
        self.config.external_endpoint = Some(url.into());
        self
    }

    /// Sets an external log collector API key.
    #[must_use]
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.config.external_api_key = Some(key.into());
        self
    }

    /// Sets the log level.
    #[must_use]
    pub fn with_level(mut self, level: impl Into<String>) -> Self {
        self.config.level = level.into();
        self
    }

    /// Initializes the telemetry layer.
    ///
    /// Sets up `tracing_subscriber` with the configured format and filters.
    /// Should be called once at application startup.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub fn init(&self) -> Result<()> {
        if self
            .initialized
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&self.config.level));

        match self.config.format {
            LogFormat::Text => {
                tracing_subscriber::fmt()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_env_filter(filter)
                    .try_init()
                    .map_err(|e| {
                        crate::error::Error::Internal(format!("Failed to init telemetry: {}", e))
                    })?;
            },
            LogFormat::Json => {
                if let Some(ref dir) = self.config.log_dir {
                    std::fs::create_dir_all(dir).map_err(|e| {
                        crate::error::Error::Internal(format!("Failed to create log dir: {}", e))
                    })?;

                    let file_appender = tracing_appender::rolling::Builder::new()
                        .rotation(tracing_appender::rolling::Rotation::DAILY)
                        .max_log_files(self.config.max_files)
                        .filename_prefix("clawdius")
                        .filename_suffix("log")
                        .build(dir)
                        .map_err(|e| {
                            crate::error::Error::Internal(format!(
                                "Failed to build log appender: {}",
                                e
                            ))
                        })?;

                    tracing_subscriber::fmt()
                        .with_target(true)
                        .json()
                        .with_writer(file_appender)
                        .with_env_filter(filter)
                        .try_init()
                        .map_err(|e| {
                            crate::error::Error::Internal(format!(
                                "Failed to init telemetry: {}",
                                e
                            ))
                        })?;
                } else {
                    tracing_subscriber::fmt()
                        .with_target(true)
                        .json()
                        .with_env_filter(filter)
                        .try_init()
                        .map_err(|e| {
                            crate::error::Error::Internal(format!(
                                "Failed to init telemetry: {}",
                                e
                            ))
                        })?;
                }
            },
            LogFormat::CompactJson => {
                tracing_subscriber::fmt()
                    .with_target(false)
                    .json()
                    .compact()
                    .with_env_filter(filter)
                    .try_init()
                    .map_err(|e| {
                        crate::error::Error::Internal(format!("Failed to init telemetry: {}", e))
                    })?;
            },
        }

        tracing::info!(
            app_name = %self.config.app_name,
            format = ?self.config.format,
            "Telemetry initialized"
        );

        Ok(())
    }

    /// Sets the session context for subsequent log entries.
    ///
    /// This is called by the orchestrator when a task starts processing.
    pub async fn set_session_context(
        &self,
        session_id: impl Into<String>,
        task_id: Option<String>,
        tenant_id: Option<String>,
        worker_id: Option<String>,
    ) {
        let mut ctx = self.session_context.write().await;
        ctx.session_id = Some(session_id.into());
        ctx.task_id = task_id;
        ctx.tenant_id = tenant_id;
        ctx.worker_id = worker_id;
    }

    /// Clears the session context.
    pub async fn clear_session_context(&self) {
        let mut ctx = self.session_context.write().await;
        *ctx = SessionContext::default();
    }

    /// Returns the current session context.
    pub async fn session_context(&self) -> SessionContext {
        self.session_context.read().await.clone()
    }

    /// Creates a telemetry event from the current session context.
    pub async fn make_event(
        &self,
        level: impl Into<String>,
        message: impl Into<String>,
    ) -> TelemetryEvent {
        let ctx = self.session_context.read().await;
        TelemetryEvent::new(
            ctx.session_id.as_deref().unwrap_or("unknown"),
            level,
            message,
        )
        .with_task(ctx.task_id.as_deref().unwrap_or(""))
        .with_tenant(ctx.tenant_id.as_deref().unwrap_or(""))
        .with_worker(ctx.worker_id.as_deref().unwrap_or(""))
    }

    /// Logs a session-scoped info event.
    pub async fn info(&self, component: &str, message: &str) {
        let event = self
            .make_event("info", message)
            .await
            .with_component(component);
        if let Ok(json) = event.to_json() {
            tracing::info!(
                session_id = %event.session_id,
                component = component,
                event = %json,
                "{}", message
            );
        } else {
            tracing::info!(
                session_id = %event.session_id,
                component = component,
                "{}", message
            );
        }
    }

    /// Logs a session-scoped error event.
    pub async fn error(&self, component: &str, message: &str) {
        let event = self
            .make_event("error", message)
            .await
            .with_component(component);
        if let Ok(json) = event.to_json() {
            tracing::error!(
                session_id = %event.session_id,
                component = component,
                event = %json,
                "{}", message
            );
        } else {
            tracing::error!(
                session_id = %event.session_id,
                component = component,
                "{}", message
            );
        }
    }
}

/// Timeline checkpoint for time-travel debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCheckpoint {
    /// Checkpoint ID
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Task ID
    pub task_id: String,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Step description
    pub step: String,
    /// Agent that created this checkpoint
    pub agent: String,
    /// Files changed in this step
    pub files_changed: Vec<String>,
    /// Telemetry events up to this checkpoint
    pub events: Vec<TelemetryEvent>,
    /// Whether this checkpoint represents a successful state
    pub success: bool,
}

/// Exports session timelines for time-travel debugging.
///
/// Each session's timeline is a sequence of checkpoints that can be
/// replayed in the web dashboard.
pub struct TimelineExporter {
    /// Storage directory for timeline data
    storage_dir: PathBuf,
}

impl TimelineExporter {
    /// Creates a new timeline exporter.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage directory cannot be created.
    pub fn new(storage_dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = storage_dir.into();
        std::fs::create_dir_all(&dir).map_err(|e| {
            crate::error::Error::Internal(format!("Failed to create timeline dir: {}", e))
        })?;
        Ok(Self { storage_dir: dir })
    }

    /// Exports a timeline checkpoint to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn export_checkpoint(&self, checkpoint: &TimelineCheckpoint) -> Result<()> {
        let filename = format!(
            "{}_{}_{}.json",
            checkpoint.session_id, checkpoint.task_id, checkpoint.id
        );
        let path = self.storage_dir.join(filename);
        let json =
            serde_json::to_string_pretty(checkpoint).map_err(crate::error::Error::Serialization)?;
        std::fs::write(&path, json).map_err(|e| {
            crate::error::Error::Internal(format!("Failed to write checkpoint: {}", e))
        })?;
        Ok(())
    }

    /// Loads a timeline checkpoint from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or parsing fails.
    pub fn load_checkpoint(
        &self,
        session_id: &str,
        task_id: &str,
        checkpoint_id: &str,
    ) -> Result<TimelineCheckpoint> {
        let filename = format!("{}_{}_{}.json", session_id, task_id, checkpoint_id);
        let path = self.storage_dir.join(filename);
        let json = std::fs::read_to_string(&path).map_err(|e| {
            crate::error::Error::Internal(format!("Failed to read checkpoint: {}", e))
        })?;
        serde_json::from_str(&json).map_err(|e| crate::error::Error::ParseError(e.to_string()))
    }

    /// Lists all checkpoints for a session.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    pub fn list_checkpoints(&self, session_id: &str) -> Result<Vec<String>> {
        let pattern = format!("{}_", session_id);
        let mut checkpoints = Vec::new();

        for entry in std::fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&pattern) && name.ends_with(".json") {
                checkpoints.push(name);
            }
        }

        checkpoints.sort();
        Ok(checkpoints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_event_creation() {
        let event = TelemetryEvent::new("session-123", "info", "Task started")
            .with_task("task-456")
            .with_tenant("tenant-789")
            .with_worker("worker-0")
            .with_component("Orchestrator")
            .with_field("duration_ms", serde_json::json!(1234));

        assert_eq!(event.session_id, "session-123");
        assert_eq!(event.task_id.as_deref(), Some("task-456"));
        assert_eq!(event.tenant_id.as_deref(), Some("tenant-789"));
        assert_eq!(event.worker_id.as_deref(), Some("worker-0"));
        assert_eq!(event.component, "Orchestrator");
        assert_eq!(event.fields.get("duration_ms").unwrap(), 1234);
    }

    #[test]
    fn test_telemetry_event_json() {
        let event =
            TelemetryEvent::new("s1", "error", "Something failed").with_component("TestRunner");
        let json = event.to_json().expect("serialize");
        assert!(json.contains("\"session_id\":\"s1\""));
        assert!(json.contains("\"level\":\"error\""));
    }

    #[test]
    fn test_structured_config_default() {
        let config = StructuredTelemetryConfig::default();
        assert_eq!(config.app_name, "clawdius");
        assert_eq!(config.format, LogFormat::Text);
        assert_eq!(config.level, "info");
        assert!(config.log_dir.is_none());
    }

    #[test]
    fn test_telemetry_layer_builder() {
        let layer = TelemetryLayer::new("test-app")
            .with_format(LogFormat::Json)
            .with_level("debug");

        assert_eq!(layer.config.app_name, "test-app");
        assert_eq!(layer.config.format, LogFormat::Json);
        assert_eq!(layer.config.level, "debug");
    }

    #[tokio::test]
    async fn test_session_context() {
        let layer = TelemetryLayer::new("test");

        layer
            .set_session_context(
                "session-1",
                Some("task-1".to_string()),
                Some("tenant-1".to_string()),
                Some("worker-0".to_string()),
            )
            .await;

        let ctx = layer.session_context().await;
        assert_eq!(ctx.session_id.as_deref(), Some("session-1"));
        assert_eq!(ctx.task_id.as_deref(), Some("task-1"));
        assert_eq!(ctx.tenant_id.as_deref(), Some("tenant-1"));
        assert_eq!(ctx.worker_id.as_deref(), Some("worker-0"));

        layer.clear_session_context().await;
        let ctx = layer.session_context().await;
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn test_timeline_exporter() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exporter = TimelineExporter::new(dir.path()).expect("exporter");

        let checkpoint = TimelineCheckpoint {
            id: "cp-1".to_string(),
            session_id: "s1".to_string(),
            task_id: "t1".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            step: "Initial plan".to_string(),
            agent: "PlannerAgent".to_string(),
            files_changed: vec!["src/main.rs".to_string()],
            events: vec![],
            success: true,
        };

        exporter.export_checkpoint(&checkpoint).expect("export");

        let loaded = exporter.load_checkpoint("s1", "t1", "cp-1").expect("load");
        assert_eq!(loaded.id, "cp-1");
        assert_eq!(loaded.step, "Initial plan");
        assert!(loaded.success);

        let list = exporter.list_checkpoints("s1").expect("list");
        assert_eq!(list.len(), 1);
    }
}
