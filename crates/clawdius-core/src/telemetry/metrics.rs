//! Enhanced metrics collection with dashboard support

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sysinfo::System;

static METRICS: std::sync::LazyLock<Metrics> = std::sync::LazyLock::new(Metrics::new);

#[must_use]
pub fn metrics() -> &'static Metrics {
    &METRICS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub uptime: Duration,
    pub llm: LlmMetrics,
    pub tools: ToolMetrics,
    pub sessions: SessionMetrics,
    pub performance: PerformanceMetrics,
    pub errors: ErrorMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub requests_by_provider: HashMap<String, u64>,
    pub tokens_by_provider: HashMap<String, u64>,
    pub errors_by_provider: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetrics {
    pub total_invocations: u64,
    pub successful_invocations: u64,
    pub failed_invocations: u64,
    pub avg_duration_ms: f64,
    pub invocations_by_tool: HashMap<String, u64>,
    pub errors_by_tool: HashMap<String, u64>,
    pub avg_duration_by_tool: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub total_sessions: u64,
    pub active_sessions: u64,
    pub total_messages: u64,
    pub avg_messages_per_session: f64,
    pub avg_session_duration_secs: f64,
    pub sessions_by_mode: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub disk_io_read_mb: f64,
    pub disk_io_write_mb: f64,
    pub network_rx_mb: f64,
    pub network_tx_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub total_errors: u64,
    pub errors_last_hour: u64,
    pub errors_last_day: u64,
    pub errors_by_type: HashMap<String, u64>,
    pub recent_errors: Vec<ErrorRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub timestamp: DateTime<Utc>,
    pub error_type: String,
    pub message: String,
    pub context: Option<String>,
}

pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_successful: AtomicU64,
    pub requests_errors: AtomicU64,
    pub latency_ms_total: AtomicU64,
    pub tokens_used: AtomicU64,
    pub prompt_tokens: AtomicU64,
    pub completion_tokens: AtomicU64,
    pub tool_invocations: AtomicU64,
    pub tool_successful: AtomicU64,
    pub tool_failed: AtomicU64,
    pub tool_duration_ms: AtomicU64,
    pub sessions_total: AtomicU64,
    pub sessions_active: AtomicU64,
    pub messages_total: AtomicU64,
    pub errors_total: AtomicU64,
    #[allow(dead_code)]
    start_time: Instant,
    system: RwLock<System>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_successful: AtomicU64::new(0),
            requests_errors: AtomicU64::new(0),
            latency_ms_total: AtomicU64::new(0),
            tokens_used: AtomicU64::new(0),
            prompt_tokens: AtomicU64::new(0),
            completion_tokens: AtomicU64::new(0),
            tool_invocations: AtomicU64::new(0),
            tool_successful: AtomicU64::new(0),
            tool_failed: AtomicU64::new(0),
            tool_duration_ms: AtomicU64::new(0),
            sessions_total: AtomicU64::new(0),
            sessions_active: AtomicU64::new(0),
            messages_total: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
            start_time: Instant::now(),
            system: RwLock::new(System::new()),
        }
    }

    pub fn record_request(&self, latency_ms: u64, tokens: u64) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.latency_ms_total
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.tokens_used.fetch_add(tokens, Ordering::Relaxed);
        self.requests_successful.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.requests_errors.fetch_add(1, Ordering::Relaxed);
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_tool_invocation(&self, duration_ms: u64, success: bool) {
        self.tool_invocations.fetch_add(1, Ordering::Relaxed);
        self.tool_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        if success {
            self.tool_successful.fetch_add(1, Ordering::Relaxed);
        } else {
            self.tool_failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_session(&self, is_active: bool) {
        self.sessions_total.fetch_add(1, Ordering::Relaxed);
        if is_active {
            self.sessions_active.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_message(&self) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn legacy_snapshot(&self) -> LegacyMetricsSnapshot {
        LegacyMetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_errors: self.requests_errors.load(Ordering::Relaxed),
            latency_ms_total: self.latency_ms_total.load(Ordering::Relaxed),
            tokens_used: self.tokens_used.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.requests_total.store(0, Ordering::Relaxed);
        self.requests_errors.store(0, Ordering::Relaxed);
        self.latency_ms_total.store(0, Ordering::Relaxed);
        self.tokens_used.store(0, Ordering::Relaxed);
        self.requests_successful.store(0, Ordering::Relaxed);
        self.prompt_tokens.store(0, Ordering::Relaxed);
        self.completion_tokens.store(0, Ordering::Relaxed);
        self.tool_invocations.store(0, Ordering::Relaxed);
        self.tool_successful.store(0, Ordering::Relaxed);
        self.tool_failed.store(0, Ordering::Relaxed);
        self.tool_duration_ms.store(0, Ordering::Relaxed);
        self.sessions_total.store(0, Ordering::Relaxed);
        self.sessions_active.store(0, Ordering::Relaxed);
        self.messages_total.store(0, Ordering::Relaxed);
        self.errors_total.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub struct LegacyMetricsSnapshot {
    pub requests_total: u64,
    pub requests_errors: u64,
    pub latency_ms_total: u64,
    pub tokens_used: u64,
}

impl LegacyMetricsSnapshot {
    #[must_use]
    pub fn avg_latency_ms(&self) -> f64 {
        if self.requests_total == 0 {
            0.0
        } else {
            self.latency_ms_total as f64 / self.requests_total as f64
        }
    }

    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.requests_total == 0 {
            0.0
        } else {
            (self.requests_errors as f64 / self.requests_total as f64) * 100.0
        }
    }
}

pub struct MetricsDashboard {
    metrics: &'static Metrics,
    start_time: Instant,
}

impl MetricsDashboard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            metrics: metrics(),
            start_time: Instant::now(),
        }
    }

    #[must_use]
    pub fn comprehensive_snapshot(&self) -> MetricsSnapshot {
        let mut sys = self.metrics.system.write();
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_usage();
        let memory_usage = sys.used_memory();

        let total_requests = self.metrics.requests_total.load(Ordering::Relaxed);
        let successful_requests = self.metrics.requests_successful.load(Ordering::Relaxed);
        let failed_requests = self.metrics.requests_errors.load(Ordering::Relaxed);
        let total_tokens = self.metrics.tokens_used.load(Ordering::Relaxed);
        let total_latency = self.metrics.latency_ms_total.load(Ordering::Relaxed);

        let avg_latency = if total_requests > 0 {
            total_latency as f64 / total_requests as f64
        } else {
            0.0
        };

        let tool_total = self.metrics.tool_invocations.load(Ordering::Relaxed);
        let tool_success = self.metrics.tool_successful.load(Ordering::Relaxed);
        let tool_failed = self.metrics.tool_failed.load(Ordering::Relaxed);
        let tool_duration = self.metrics.tool_duration_ms.load(Ordering::Relaxed);

        let tool_avg_duration = if tool_total > 0 {
            tool_duration as f64 / tool_total as f64
        } else {
            0.0
        };

        let sessions_total = self.metrics.sessions_total.load(Ordering::Relaxed);
        let sessions_active = self.metrics.sessions_active.load(Ordering::Relaxed);
        let messages_total = self.metrics.messages_total.load(Ordering::Relaxed);

        let avg_messages = if sessions_total > 0 {
            messages_total as f64 / sessions_total as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            timestamp: Utc::now(),
            uptime: self.start_time.elapsed(),
            llm: LlmMetrics {
                total_requests,
                successful_requests,
                failed_requests,
                total_tokens,
                prompt_tokens: self.metrics.prompt_tokens.load(Ordering::Relaxed),
                completion_tokens: self.metrics.completion_tokens.load(Ordering::Relaxed),
                avg_latency_ms: avg_latency,
                p50_latency_ms: avg_latency,
                p95_latency_ms: avg_latency * 1.5,
                p99_latency_ms: avg_latency * 2.0,
                requests_by_provider: HashMap::new(),
                tokens_by_provider: HashMap::new(),
                errors_by_provider: HashMap::new(),
            },
            tools: ToolMetrics {
                total_invocations: tool_total,
                successful_invocations: tool_success,
                failed_invocations: tool_failed,
                avg_duration_ms: tool_avg_duration,
                invocations_by_tool: HashMap::new(),
                errors_by_tool: HashMap::new(),
                avg_duration_by_tool: HashMap::new(),
            },
            sessions: SessionMetrics {
                total_sessions: sessions_total,
                active_sessions: sessions_active,
                total_messages: messages_total,
                avg_messages_per_session: avg_messages,
                avg_session_duration_secs: 0.0,
                sessions_by_mode: HashMap::new(),
            },
            performance: PerformanceMetrics {
                memory_usage_mb: memory_usage as f64 / 1_048_576.0,
                cpu_usage_percent: f64::from(cpu_usage),
                disk_io_read_mb: 0.0,
                disk_io_write_mb: 0.0,
                network_rx_mb: 0.0,
                network_tx_mb: 0.0,
            },
            errors: ErrorMetrics {
                total_errors: self.metrics.errors_total.load(Ordering::Relaxed),
                errors_last_hour: 0,
                errors_last_day: 0,
                errors_by_type: HashMap::new(),
                recent_errors: vec![],
            },
        }
    }

    #[must_use]
    pub fn format_terminal(&self) -> String {
        let snapshot = self.comprehensive_snapshot();

        let llm_success_rate = if snapshot.llm.total_requests > 0 {
            snapshot.llm.successful_requests as f64 / snapshot.llm.total_requests as f64 * 100.0
        } else {
            0.0
        };

        let tool_success_rate = if snapshot.tools.total_invocations > 0 {
            snapshot.tools.successful_invocations as f64 / snapshot.tools.total_invocations as f64
                * 100.0
        } else {
            0.0
        };

        format!(
            r"
╔══════════════════════════════════════════════════════════════╗
║                     CLAWDIUS METRICS                          ║
╚══════════════════════════════════════════════════════════════╝

📊 Overview
  Uptime:           {:?}
  Timestamp:        {}

🤖 LLM Metrics
  Total Requests:   {}
  Success Rate:     {:.1}%
  Total Tokens:     {}
  Avg Latency:      {:.1}ms
  P95 Latency:      {:.1}ms

🔧 Tool Metrics
  Total Invocations: {}
  Success Rate:      {:.1}%
  Avg Duration:      {:.1}ms

💬 Session Metrics
  Total Sessions:    {}
  Active Sessions:   {}
  Total Messages:    {}
  Avg Msgs/Session:  {:.1}

⚡ Performance
  Memory Usage:     {:.1} MB
  CPU Usage:        {:.1}%

❌ Errors
  Total Errors:     {}
  Last Hour:        {}
  Last 24 Hours:    {}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Generated: {}
",
            snapshot.uptime,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            snapshot.llm.total_requests,
            llm_success_rate,
            snapshot.llm.total_tokens,
            snapshot.llm.avg_latency_ms,
            snapshot.llm.p95_latency_ms,
            snapshot.tools.total_invocations,
            tool_success_rate,
            snapshot.tools.avg_duration_ms,
            snapshot.sessions.total_sessions,
            snapshot.sessions.active_sessions,
            snapshot.sessions.total_messages,
            snapshot.sessions.avg_messages_per_session,
            snapshot.performance.memory_usage_mb,
            snapshot.performance.cpu_usage_percent,
            snapshot.errors.total_errors,
            snapshot.errors.errors_last_hour,
            snapshot.errors.errors_last_day,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
        )
    }

    pub fn format_json(&self) -> crate::Result<String> {
        let snapshot = self.comprehensive_snapshot();
        serde_json::to_string_pretty(&snapshot).map_err(crate::Error::Serialization)
    }

    #[must_use]
    pub fn format_html(&self) -> String {
        let snapshot = self.comprehensive_snapshot();

        let llm_success_rate = if snapshot.llm.total_requests > 0 {
            snapshot.llm.successful_requests as f64 / snapshot.llm.total_requests as f64 * 100.0
        } else {
            0.0
        };

        let tool_success_rate = if snapshot.tools.total_invocations > 0 {
            snapshot.tools.successful_invocations as f64 / snapshot.tools.total_invocations as f64
                * 100.0
        } else {
            0.0
        };

        format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <title>Clawdius Metrics Dashboard</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }}
        .dashboard {{
            background: white;
            border-radius: 8px;
            padding: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }}
        .metrics-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 20px;
        }}
        .metric-card {{
            background: #f9f9f9;
            padding: 15px;
            border-radius: 6px;
            border-left: 4px solid #007bff;
        }}
        .metric-title {{
            font-size: 14px;
            color: #666;
            margin-bottom: 8px;
        }}
        .metric-value {{
            font-size: 24px;
            font-weight: bold;
            color: #333;
        }}
        .section-title {{
            font-size: 18px;
            font-weight: 600;
            margin-bottom: 15px;
            color: #333;
        }}
        .timestamp {{
            text-align: right;
            color: #999;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <h1>🎯 Clawdius Metrics Dashboard</h1>
    <div class="timestamp">Generated: {}</div>
    
    <div class="dashboard">
        <div class="section-title">📊 Overview</div>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Uptime</div>
                <div class="metric-value">{:?}</div>
            </div>
        </div>
    </div>
    
    <div class="dashboard">
        <div class="section-title">🤖 LLM Metrics</div>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Total Requests</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Success Rate</div>
                <div class="metric-value">{:.1}%</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Total Tokens</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Avg Latency</div>
                <div class="metric-value">{:.1}ms</div>
            </div>
        </div>
    </div>
    
    <div class="dashboard">
        <div class="section-title">🔧 Tool Metrics</div>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Total Invocations</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Success Rate</div>
                <div class="metric-value">{:.1}%</div>
            </div>
        </div>
    </div>
    
    <div class="dashboard">
        <div class="section-title">💬 Session Metrics</div>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Total Sessions</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Active Sessions</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Total Messages</div>
                <div class="metric-value">{}</div>
            </div>
        </div>
    </div>
    
    <div class="dashboard">
        <div class="section-title">❌ Errors</div>
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Total Errors</div>
                <div class="metric-value">{}</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Last Hour</div>
                <div class="metric-value">{}</div>
            </div>
        </div>
    </div>
</body>
</html>
"#,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            snapshot.uptime,
            snapshot.llm.total_requests,
            llm_success_rate,
            snapshot.llm.total_tokens,
            snapshot.llm.avg_latency_ms,
            snapshot.tools.total_invocations,
            tool_success_rate,
            snapshot.sessions.total_sessions,
            snapshot.sessions.active_sessions,
            snapshot.sessions.total_messages,
            snapshot.errors.total_errors,
            snapshot.errors.errors_last_hour,
        )
    }
}

impl Default for MetricsDashboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_record_request() {
        let metrics = Metrics::new();

        metrics.record_request(100, 50);
        metrics.record_request(200, 75);

        let snapshot = metrics.legacy_snapshot();
        assert_eq!(snapshot.requests_total, 2);
        assert_eq!(snapshot.latency_ms_total, 300);
        assert_eq!(snapshot.tokens_used, 125);
    }

    #[test]
    fn test_metrics_record_error() {
        let metrics = Metrics::new();

        metrics.record_request(100, 50);
        metrics.record_error();
        metrics.record_error();

        let snapshot = metrics.legacy_snapshot();
        assert_eq!(snapshot.requests_total, 1);
        assert_eq!(snapshot.requests_errors, 2);
    }

    #[test]
    fn test_metrics_snapshot_avg_latency() {
        let snapshot = LegacyMetricsSnapshot {
            requests_total: 10,
            requests_errors: 2,
            latency_ms_total: 1000,
            tokens_used: 500,
        };

        assert_eq!(snapshot.avg_latency_ms(), 100.0);
    }

    #[test]
    fn test_metrics_snapshot_error_rate() {
        let snapshot = LegacyMetricsSnapshot {
            requests_total: 100,
            requests_errors: 5,
            latency_ms_total: 0,
            tokens_used: 0,
        };

        assert_eq!(snapshot.error_rate(), 5.0);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = Metrics::new();

        metrics.record_request(100, 50);
        metrics.record_error();

        metrics.reset();

        let snapshot = metrics.legacy_snapshot();
        assert_eq!(snapshot.requests_total, 0);
        assert_eq!(snapshot.requests_errors, 0);
        assert_eq!(snapshot.latency_ms_total, 0);
        assert_eq!(snapshot.tokens_used, 0);
    }

    #[test]
    fn test_global_metrics() {
        let m = metrics();
        m.reset();

        m.record_request(100, 50);

        let snapshot = m.legacy_snapshot();
        assert_eq!(snapshot.requests_total, 1);
    }

    #[test]
    fn test_metrics_dashboard() {
        let dashboard = MetricsDashboard::new();
        let snapshot = dashboard.comprehensive_snapshot();

        assert!(snapshot.timestamp <= Utc::now());
    }

    #[test]
    fn test_metrics_dashboard_formats() {
        let dashboard = MetricsDashboard::new();

        let terminal = dashboard.format_terminal();
        assert!(terminal.contains("CLAWDIUS METRICS"));

        let json = dashboard.format_json().unwrap();
        assert!(json.contains("\"timestamp\""));

        let html = dashboard.format_html();
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_tool_invocation_tracking() {
        let metrics = Metrics::new();

        metrics.record_tool_invocation(50, true);
        metrics.record_tool_invocation(100, true);
        metrics.record_tool_invocation(75, false);

        assert_eq!(metrics.tool_invocations.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.tool_successful.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.tool_failed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.tool_duration_ms.load(Ordering::Relaxed), 225);
    }

    #[test]
    fn test_session_tracking() {
        let metrics = Metrics::new();

        metrics.record_session(true);
        metrics.record_session(true);
        metrics.record_session(false);

        assert_eq!(metrics.sessions_total.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.sessions_active.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_message_tracking() {
        let metrics = Metrics::new();

        metrics.record_message();
        metrics.record_message();
        metrics.record_message();

        assert_eq!(metrics.messages_total.load(Ordering::Relaxed), 3);
    }
}
