//! Telemetry and observability for Clawdius
//!
//! This module provides crash reporting, error tracking, metrics, and observability features.

// Unwrap purge batch 4: telemetry module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod crash;
mod metrics;
pub mod structured;

pub use crash::CrashReporter;
pub use metrics::{
    metrics, ErrorMetrics, ErrorRecord, LegacyMetricsSnapshot, LlmMetrics, Metrics,
    MetricsDashboard, MetricsSnapshot, PerformanceMetrics, SessionMetrics, ToolMetrics,
};
pub use structured::{
    LogFormat, StructuredTelemetryConfig, TelemetryEvent, TelemetryLayer, TimelineCheckpoint,
    TimelineExporter,
};

/// Telemetry configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TelemetryConfig {
    /// Enable crash reporting
    #[serde(default)]
    pub crash_reporting: bool,
    /// Sentry DSN (can also be set via `SENTRY_DSN` env var)
    #[serde(default)]
    pub sentry_dsn: Option<String>,
    /// Enable metrics collection
    #[serde(default)]
    pub metrics_enabled: bool,
    /// Enable performance monitoring
    #[serde(default)]
    pub performance_monitoring: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_telemetry_config() {
        let config = TelemetryConfig::default();
        assert!(!config.crash_reporting);
        assert!(config.sentry_dsn.is_none());
    }
}
