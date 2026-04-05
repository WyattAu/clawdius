//! HTTP Metrics
//!
//! Lightweight in-memory metrics for the Clawdius HTTP server.
//! Tracks request counts, latency histograms, and active connections.
//! Exposes a `/metrics` endpoint in Prometheus text exposition format.
//!
//! # Metrics
//!
//! | Name | Type | Labels |
//! |------|------|--------|
//! | `clawdius_http_requests_total` | counter | `method`, `route`, `status` |
//! | `clawdius_http_request_duration_ms` | histogram | `method`, `route` |
//! | `clawdius_http_active_requests` | gauge | — |

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicI64, AtomicU64, Ordering},
    Mutex,
};

use axum::extract::State as AxumState;
use axum::http::StatusCode;
use axum::response::IntoResponse;

// ---------------------------------------------------------------------------
// Metric name constants
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub const AUDIT_EVENTS_TOTAL: &str = "clawdius_messaging_audit_events_total";
#[allow(dead_code)]
pub const RETRY_DEAD_LETTER_TOTAL: &str = "clawdius_messaging_retry_dead_letter_total";
#[allow(dead_code)]
pub const PII_REDACTIONS_TOTAL: &str = "clawdius_messaging_pii_redactions_total";
#[allow(dead_code)]
pub const ACTIVE_SESSIONS: &str = "clawdius_messaging_active_sessions";
#[allow(dead_code)]
pub const TENANTS_TOTAL: &str = "clawdius_messaging_tenants_total";
#[allow(dead_code)]
pub const RETRY_QUEUE_PENDING: &str = "clawdius_messaging_retry_queue_pending";
#[allow(dead_code)]
pub const USAGE_MESSAGES_TOTAL: &str = "clawdius_usage_messages_total";
#[allow(dead_code)]
pub const USAGE_MESSAGE_DURATION_MS: &str = "clawdius_usage_message_duration_ms";
#[allow(dead_code)]
pub const USAGE_ACTIVE_TENANTS: &str = "clawdius_usage_active_tenants";

// ---------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------

/// Fixed-bucket histogram for latency tracking.
#[derive(Debug, Clone)]
pub struct Histogram {
    bounds: Vec<f64>,
    counts: Vec<u64>,
    count: u64,
    sum: f64,
}

impl Histogram {
    /// Create a histogram with standard HTTP latency buckets (ms).
    pub fn new_http() -> Self {
        Self::new(&[
            0.1, 0.5, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
        ])
    }

    pub fn new(bounds: &[f64]) -> Self {
        Self {
            bounds: bounds.to_vec(),
            counts: vec![0; bounds.len()],
            count: 0,
            sum: 0.0,
        }
    }

    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
        for (i, &bound) in self.bounds.iter().enumerate() {
            if value <= bound {
                self.counts[i] += 1;
            }
        }
    }

    #[allow(dead_code)]
    pub fn count(&self) -> u64 {
        self.count
    }

    #[allow(dead_code)]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// Write Prometheus histogram exposition lines.
    pub fn export_prometheus(&self, name: &str, labels: &str) -> String {
        let mut out = String::new();
        for (i, &bound) in self.bounds.iter().enumerate() {
            out.push_str(&format!(
                "{}_bucket{{{labels},le=\"{:.1}\"}} {}\n",
                name, bound, self.counts[i]
            ));
        }
        out.push_str(&format!(
            "{}_bucket{{{labels},le=\"+Inf\"}} {}\n",
            name, self.count
        ));
        out.push_str(&format!("{}_sum{{{labels}}} {:.3}\n", name, self.sum));
        out.push_str(&format!("{}_count{{{labels}}} {}\n", name, self.count));
        out
    }
}

// ---------------------------------------------------------------------------
// Metrics Store
// ---------------------------------------------------------------------------

/// Thread-safe metrics store keyed by label sets.
#[derive(Debug, Clone)]
pub struct MetricsStore {
    inner: std::sync::Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    request_counts: Mutex<HashMap<String, u64>>,
    request_durations: Mutex<HashMap<String, Histogram>>,
    active_requests: AtomicI64,
    messaging_counters: Mutex<HashMap<String, AtomicU64>>,
    messaging_gauges: Mutex<HashMap<String, AtomicI64>>,
}

impl MetricsStore {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(MetricsInner {
                request_counts: Mutex::new(HashMap::new()),
                request_durations: Mutex::new(HashMap::new()),
                active_requests: AtomicI64::new(0),
                messaging_counters: Mutex::new(HashMap::new()),
                messaging_gauges: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Record a completed request.
    #[allow(dead_code)]
    pub fn record_request(&self, method: &str, route: &str, status: u16, duration_ms: f64) {
        let count_key = format!(
            "clawdius_http_requests_total{{method=\"{}\",route=\"{}\",status=\"{}\"}}",
            method, route, status
        );
        {
            let mut counts = self
                .inner
                .request_counts
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *counts.entry(count_key).or_insert(0) += 1;
        }

        let hist_key = format!(
            "clawdius_http_request_duration_ms{{method=\"{}\",route=\"{}\"}}",
            method, route
        );
        {
            let mut histograms = self
                .inner
                .request_durations
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            histograms
                .entry(hist_key)
                .or_insert_with(Histogram::new_http)
                .observe(duration_ms);
        }
    }

    #[allow(dead_code)]
    pub fn active_inc(&self) {
        self.inner
            .active_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn active_dec(&self) {
        self.inner.active_requests.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn messaging_counter_inc(&self, name: &str, labels: &str) {
        let key = format!("{}{{{}}}", name, labels);
        let mut counters = self
            .inner
            .messaging_counters
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        counters
            .entry(key)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn messaging_gauge_set(&self, name: &str, value: i64) {
        let mut gauges = self
            .inner
            .messaging_gauges
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        gauges
            .entry(name.to_string())
            .or_insert_with(|| AtomicI64::new(0))
            .store(value, Ordering::Relaxed);
    }

    /// Record a histogram observation for a messaging metric.
    pub fn messaging_histogram_observe(&self, name: &str, labels: &str, value_ms: f64) {
        let hist_key = format!("{}{{{}}}", name, labels);
        let mut histograms = self
            .inner
            .request_durations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        histograms
            .entry(hist_key)
            .or_insert_with(Histogram::new_http)
            .observe(value_ms);
    }

    /// Export all metrics in Prometheus text exposition format.
    pub fn export_prometheus(&self) -> String {
        let mut out = String::new();

        {
            let counts = self
                .inner
                .request_counts
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (key, value) in counts.iter() {
                out.push_str(&format!("{} {}\n", key, value));
            }
        }

        {
            let histograms = self
                .inner
                .request_durations
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (key, hist) in histograms.iter() {
                let open = '{';
                if let Some(brace_pos) = key.find(open) {
                    let name = &key[..brace_pos];
                    let labels = &key[brace_pos + 1..key.len() - 1];
                    out.push_str(&hist.export_prometheus(name, labels));
                }
            }
        }

        let active = self.inner.active_requests.load(Ordering::Relaxed);
        out.push_str(&format!("clawdius_http_active_requests {}\n", active));

        {
            let counters = self
                .inner
                .messaging_counters
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (key, atomic) in counters.iter() {
                out.push_str(&format!("{} {}\n", key, atomic.load(Ordering::Relaxed)));
            }
        }

        {
            let gauges = self
                .inner
                .messaging_gauges
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for (key, atomic) in gauges.iter() {
                out.push_str(&format!("{} {}\n", key, atomic.load(Ordering::Relaxed)));
            }
        }

        out
    }
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Shared State
// ---------------------------------------------------------------------------

/// Shared state for the metrics handler.
#[derive(Debug, Clone)]
pub struct HttpMetrics {
    pub store: MetricsStore,
}

// ---------------------------------------------------------------------------
// /metrics Handler
// ---------------------------------------------------------------------------

/// GET `/metrics` — Prometheus scrape endpoint.
#[allow(dead_code)]
pub async fn metrics_handler(AxumState(metrics): AxumState<HttpMetrics>) -> impl IntoResponse {
    let body = metrics.store.export_prometheus();
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

// ---------------------------------------------------------------------------
// Route normalization
// ---------------------------------------------------------------------------

/// Normalize a path for metrics labels.
///
/// - Strips query strings
/// - Replaces UUIDs and long numeric IDs with `:id`
/// - Keeps segments that are ID replacements, caps total at 4 segments
#[allow(dead_code)]
pub fn normalize_route(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path);
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for seg in &mut segments {
        if looks_like_uuid(seg) || (seg.parse::<u64>().is_ok() && seg.len() > 4) {
            *seg = ":id";
        }
    }

    // Truncate to 4 segments max (preserves :id if present in 4th position)
    segments.truncate(4);

    let mut result = String::from("/");
    if let Some(first) = segments.first() {
        result.push_str(first);
        for seg in segments.iter().skip(1) {
            result.push('/');
            result.push_str(seg);
        }
    }
    result
}

#[allow(dead_code)]
fn looks_like_uuid(s: &str) -> bool {
    s.len() == 36
        && s.chars().filter(|&c| c == '-').count() == 4
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_observe_basic() {
        let mut h = Histogram::new_http();
        h.observe(0.3);
        h.observe(5.0);
        h.observe(100.0);
        assert_eq!(h.count(), 3);
        assert!((h.sum() - 105.3).abs() < 0.01);
    }

    #[test]
    fn histogram_export_prometheus() {
        let mut h = Histogram::new(&[1.0, 10.0]);
        h.observe(0.5);
        h.observe(5.0);
        h.observe(15.0);

        assert_eq!(h.count(), 3);

        let out = h.export_prometheus("test_metric", "method=\"GET\"");
        assert!(!out.is_empty(), "export_prometheus returned empty string");
        assert!(out.contains("test_metric_bucket{method=\"GET\",le=\"1.0\"} 1"));
        assert!(out.contains("test_metric_bucket{method=\"GET\",le=\"10.0\"} 2"));
        assert!(out.contains("test_metric_bucket{method=\"GET\",le=\"+Inf\"} 3"));
        assert!(out.contains("test_metric_sum{method=\"GET\"}"));
        assert!(out.contains("test_metric_count{method=\"GET\"} 3"));
    }

    #[test]
    fn metrics_store_records_requests() {
        let store = MetricsStore::new();
        store.record_request("GET", "/health", 200, 0.5);
        store.record_request("POST", "/webhook/telegram", 200, 5.2);
        store.record_request("POST", "/webhook/telegram", 401, 0.1);

        let output = store.export_prometheus();
        assert!(output.contains(
            "clawdius_http_requests_total{method=\"GET\",route=\"/health\",status=\"200\"} 1"
        ));
        assert!(output.contains("clawdius_http_requests_total{method=\"POST\",route=\"/webhook/telegram\",status=\"200\"} 1"));
        assert!(output.contains("clawdius_http_requests_total{method=\"POST\",route=\"/webhook/telegram\",status=\"401\"} 1"));
        assert!(output.contains("clawdius_http_active_requests 0"));
    }

    #[test]
    fn metrics_store_active_gauge() {
        let store = MetricsStore::new();
        store.active_inc();
        store.active_inc();
        store.active_dec();
        assert_eq!(
            store
                .inner
                .active_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        store.active_dec();
        assert_eq!(
            store
                .inner
                .active_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn normalize_route_basic() {
        assert_eq!(normalize_route("/health"), "/health");
        assert_eq!(normalize_route("/api/v1/chat"), "/api/v1/chat");
        assert_eq!(normalize_route("/webhook/telegram"), "/webhook/telegram");
    }

    #[test]
    fn normalize_route_strips_query() {
        assert_eq!(
            normalize_route("/webhook/telegram?secret=abc"),
            "/webhook/telegram"
        );
    }

    #[test]
    fn normalize_route_replaces_uuid() {
        assert_eq!(
            normalize_route("/api/v1/sessions/550e8400-e29b-41d4-a716-446655440000"),
            "/api/v1/sessions/:id"
        );
    }

    #[test]
    fn normalize_route_replaces_numeric_id() {
        assert_eq!(
            normalize_route("/api/v1/sessions/12345"),
            "/api/v1/sessions/:id"
        );
    }

    #[test]
    fn normalize_route_caps_segments() {
        assert_eq!(normalize_route("/a/b/c/d/e"), "/a/b/c/d");
    }

    #[test]
    fn looks_like_uuid_test() {
        assert!(looks_like_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(looks_like_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!looks_like_uuid("not-a-uuid"));
        assert!(!looks_like_uuid("12345"));
        assert!(!looks_like_uuid(""));
    }

    #[test]
    fn export_prometheus_includes_duration_histogram() {
        let store = MetricsStore::new();
        store.record_request("POST", "/webhook/telegram", 200, 0.3);
        store.record_request("POST", "/webhook/telegram", 200, 5.0);

        let output = store.export_prometheus();
        assert!(output.contains("clawdius_http_request_duration_ms_bucket"));
        assert!(output.contains("clawdius_http_request_duration_ms_sum"));
        assert!(output.contains("clawdius_http_request_duration_ms_count{method=\"POST\",route=\"/webhook/telegram\"} 2"));
    }

    #[test]
    fn messaging_counter_inc() {
        let store = MetricsStore::new();
        store.messaging_counter_inc(AUDIT_EVENTS_TOTAL, "category=\"message_received\"");
        store.messaging_counter_inc(AUDIT_EVENTS_TOTAL, "category=\"message_received\"");
        store.messaging_counter_inc(AUDIT_EVENTS_TOTAL, r#"category="command_executed""#);

        let output = store.export_prometheus();
        assert!(output
            .contains("clawdius_messaging_audit_events_total{category=\"message_received\"} 2"));
        assert!(output
            .contains("clawdius_messaging_audit_events_total{category=\"command_executed\"} 1"));
    }

    #[test]
    fn messaging_gauge_set() {
        let store = MetricsStore::new();
        store.messaging_gauge_set(ACTIVE_SESSIONS, 5);
        store.messaging_gauge_set(ACTIVE_SESSIONS, 3);

        let output = store.export_prometheus();
        assert!(output.contains("clawdius_messaging_active_sessions 3"));
    }

    #[test]
    fn messaging_metrics_coexist_with_http_metrics() {
        let store = MetricsStore::new();
        store.record_request("GET", "/health", 200, 1.0);
        store.messaging_counter_inc(PII_REDACTIONS_TOTAL, "source=\"telegram\"");
        store.messaging_gauge_set(TENANTS_TOTAL, 2);
        store.messaging_gauge_set(RETRY_QUEUE_PENDING, 7);

        let output = store.export_prometheus();
        assert!(output.contains("clawdius_http_requests_total"));
        assert!(output.contains("clawdius_messaging_pii_redactions_total{source=\"telegram\"} 1"));
        assert!(output.contains("clawdius_messaging_tenants_total 2"));
        assert!(output.contains("clawdius_messaging_retry_queue_pending 7"));
    }

    #[test]
    fn messaging_histogram_observe() {
        let store = MetricsStore::new();
        store.messaging_histogram_observe(
            USAGE_MESSAGE_DURATION_MS,
            "tenant=\"t1\",platform=\"telegram\",category=\"generate\"",
            5.0,
        );
        store.messaging_histogram_observe(
            USAGE_MESSAGE_DURATION_MS,
            "tenant=\"t1\",platform=\"telegram\",category=\"generate\"",
            50.0,
        );

        let output = store.export_prometheus();
        assert!(output.contains("clawdius_usage_message_duration_ms_bucket"));
        assert!(output.contains("clawdius_usage_message_duration_ms_count{tenant=\"t1\",platform=\"telegram\",category=\"generate\"} 2"));
    }

    #[test]
    fn usage_metrics_full_pipeline() {
        let store = MetricsStore::new();
        store.messaging_counter_inc(
            USAGE_MESSAGES_TOTAL,
            "tenant=\"t1\",platform=\"discord\",category=\"help\",outcome=\"success\"",
        );
        store.messaging_counter_inc(
            USAGE_MESSAGES_TOTAL,
            "tenant=\"t1\",platform=\"discord\",category=\"help\",outcome=\"error\"",
        );
        store.messaging_histogram_observe(
            USAGE_MESSAGE_DURATION_MS,
            "tenant=\"t1\",platform=\"discord\",category=\"help\"",
            12.5,
        );
        store.messaging_gauge_set(USAGE_ACTIVE_TENANTS, 3);

        let output = store.export_prometheus();
        assert!(output.contains("clawdius_usage_messages_total{tenant=\"t1\",platform=\"discord\",category=\"help\",outcome=\"success\"} 1"));
        assert!(output.contains("clawdius_usage_messages_total{tenant=\"t1\",platform=\"discord\",category=\"help\",outcome=\"error\"} 1"));
        assert!(output.contains("clawdius_usage_message_duration_ms_bucket"));
        assert!(output.contains("clawdius_usage_active_tenants 3"));
    }
}
