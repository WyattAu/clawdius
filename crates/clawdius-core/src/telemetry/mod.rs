//! Telemetry and observability for Clawdius
//!
//! This module provides crash reporting, error tracking, metrics, and observability features.

mod crash;
mod metrics;

pub use crash::CrashReporter;
pub use metrics::{
    metrics, ErrorMetrics, ErrorRecord, LegacyMetricsSnapshot, LlmMetrics, Metrics,
    MetricsDashboard, MetricsSnapshot, PerformanceMetrics, SessionMetrics, ToolMetrics,
};

/// Telemetry configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TelemetryConfig {
    /// Enable crash reporting
    #[serde(default)]
    pub crash_reporting: bool,
    /// Sentry DSN (can also be set via SENTRY_DSN env var)
    #[serde(default)]
    pub sentry_dsn: Option<String>,
    /// Enable metrics collection
    #[serde(default)]
    pub metrics_enabled: bool,
    /// Enable performance monitoring
    #[serde(default)]
    pub performance_monitoring: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            crash_reporting: false,
            sentry_dsn: None,
            metrics_enabled: false,
            performance_monitoring: false,
        }
    }
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
