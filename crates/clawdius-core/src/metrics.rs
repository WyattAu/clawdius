//! Prometheus-compatible metrics for Clawdius (REQ-009).
//!
//! A lightweight, dependency-light metrics registry that renders the
//! Prometheus text exposition format. It exposes counters, histograms,
//! and gauges for LLM requests, tool executions, sandbox runs, session
//! activity, and system health.
//!
//! No external metrics crate is required: the registry is implemented
//! with [`parking_lot`] locks and a global [`std::sync::OnceLock`]
//! handle, and is reachable from both the core library and the web
//! gateway via [`registry`] / [`render_metrics`].
//!
//! # Metric families
//!
//! - `clawdius_llm_requests_total{provider,model,status}` (counter)
//! - `clawdius_llm_request_duration_seconds{provider,model}` (histogram)
//! - `clawdius_llm_tokens_total{type}` (counter)
//! - `clawdius_tool_executions_total{tool,status}` (counter)
//! - `clawdius_tool_duration_seconds{tool}` (histogram)
//! - `clawdius_sandbox_executions_total{backend,status}` (counter)
//! - `clawdius_sandbox_duration_seconds{backend}` (histogram)
//! - `clawdius_active_sessions` (gauge)

use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
use std::time::Duration;

/// Default histogram bucket upper bounds, matching the Prometheus
/// `DefBuckets` (`0.005` .. `10.0`).
pub const DEFAULT_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Global metrics registry handle.
static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

/// Access the global [`MetricsRegistry`], initializing it on first use.
pub fn registry() -> &'static MetricsRegistry {
    REGISTRY.get_or_init(MetricsRegistry::new)
}

/// A Prometheus-compatible metrics registry.
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, CounterEntry>>,
    histograms: RwLock<HashMap<String, HistogramEntry>>,
    gauges: RwLock<HashMap<String, GaugeEntry>>,
}

struct CounterEntry {
    name: String,
    value: u64,
    labels: BTreeMap<String, String>,
}

struct HistogramEntry {
    name: String,
    count: u64,
    sum: f64,
    bounds: Vec<f64>,
    /// Cumulative count of observations falling into each bucket
    /// (aligned with [`HistogramEntry::bounds`]).
    bucket_counts: Vec<u64>,
    labels: BTreeMap<String, String>,
}

struct GaugeEntry {
    name: String,
    value: f64,
    labels: BTreeMap<String, String>,
}

impl MetricsRegistry {
    /// Create a new empty registry.
    fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
        }
    }

    /// Add `value` to the counter identified by `name` + `labels`.
    pub fn add_counter(&self, name: &str, labels: BTreeMap<String, String>, value: u64) {
        let key = metric_key(name, &labels);
        let mut counters = self.counters.write();
        counters
            .entry(key)
            .or_insert_with(|| CounterEntry {
                name: name.to_string(),
                value: 0,
                labels,
            })
            .value += value;
    }

    /// Increment a counter by one.
    pub fn increment_counter(&self, name: &str, labels: BTreeMap<String, String>) {
        self.add_counter(name, labels, 1);
    }

    /// Record a single observation `value` into a histogram.
    pub fn observe_histogram(&self, name: &str, labels: BTreeMap<String, String>, value: f64) {
        self.observe_histogram_with_buckets(name, labels, value, DEFAULT_BUCKETS);
    }

    /// Record an observation into a histogram with custom bucket bounds.
    pub fn observe_histogram_with_buckets(
        &self,
        name: &str,
        labels: BTreeMap<String, String>,
        value: f64,
        buckets: &[f64],
    ) {
        let key = metric_key(name, &labels);
        let mut histograms = self.histograms.write();
        let entry = histograms.entry(key).or_insert_with(|| HistogramEntry {
            name: name.to_string(),
            count: 0,
            sum: 0.0,
            bounds: buckets.to_vec(),
            bucket_counts: vec![0; buckets.len()],
            labels,
        });
        entry.count += 1;
        entry.sum += value;
        for (i, bound) in entry.bounds.iter().enumerate() {
            if value <= *bound {
                entry.bucket_counts[i] += 1;
            }
        }
    }

    /// Set a gauge to `value`.
    pub fn set_gauge(&self, name: &str, labels: BTreeMap<String, String>, value: f64) {
        let key = metric_key(name, &labels);
        self.gauges.write().insert(
            key,
            GaugeEntry {
                name: name.to_string(),
                value,
                labels,
            },
        );
    }

    /// Render the entire registry in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let mut output = String::new();

        // Counters, grouped by metric family name.
        let counters = self.counters.read();
        let mut seen_families: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for entry in counters.values() {
            if seen_families.insert(entry.name.as_str()) {
                output.push_str(&format!("# TYPE {} counter\n", entry.name));
            }
            output.push_str(&format!(
                "{}{} {}\n",
                entry.name,
                format_labels(&entry.labels),
                entry.value
            ));
        }

        // Gauges.
        let gauges = self.gauges.read();
        seen_families.clear();
        for entry in gauges.values() {
            if seen_families.insert(entry.name.as_str()) {
                output.push_str(&format!("# TYPE {} gauge\n", entry.name));
            }
            output.push_str(&format!(
                "{}{} {}\n",
                entry.name,
                format_labels(&entry.labels),
                entry.value
            ));
        }

        // Histograms.
        let histograms = self.histograms.read();
        seen_families.clear();
        for entry in histograms.values() {
            if seen_families.insert(entry.name.as_str()) {
                output.push_str(&format!("# TYPE {} histogram\n", entry.name));
            }
            let label_text = format_labels(&entry.labels);
            // Merge the `le="..."` bucket label into the existing label set.
            for (i, bound) in entry.bounds.iter().enumerate() {
                output.push_str(&format!(
                    "{}_bucket{} {}\n",
                    entry.name,
                    format_labels_with_le(&entry.labels, *bound),
                    entry.bucket_counts[i]
                ));
            }
            output.push_str(&format!(
                "{}_bucket{} {}\n",
                entry.name,
                format_labels_with_le(&entry.labels, f64::INFINITY),
                entry.count
            ));
            output.push_str(&format!("{}_sum{} {}\n", entry.name, label_text, entry.sum));
            output.push_str(&format!(
                "{}_count{} {}\n",
                entry.name, label_text, entry.count
            ));
            let _ = label_text;
        }

        output
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a label map from `(&str, &str)` pairs.
pub fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn metric_key(name: &str, labels: &BTreeMap<String, String>) -> String {
    format!("{}|{}", name, serialize_labels(labels))
}

fn serialize_labels(labels: &BTreeMap<String, String>) -> String {
    labels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_labels(labels: &BTreeMap<String, String>) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let inner = labels
        .iter()
        .map(|(k, v)| format!("{k}=\"{v}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{inner}}}")
}

/// Render labels with an additional `le="..."` bucket label, used for
/// histogram bucket lines. The `le` label is merged into the map so all
/// labels (including `le`) render in canonical alphabetical order.
fn format_labels_with_le(labels: &BTreeMap<String, String>, le: f64) -> String {
    let le_value = if le.is_infinite() {
        "+Inf".to_string()
    } else {
        format_float(le)
    };
    let mut all = labels.clone();
    all.insert("le".to_string(), le_value);
    format_labels(&all)
}

/// Format an `f64` the way Prometheus expects (no trailing exponent where
/// avoidable, integer values without a decimal fraction).
fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{:.1}", v)
    } else {
        format!("{v}")
    }
}

// ---------------------------------------------------------------------------
// Convenience recorders
// ---------------------------------------------------------------------------

/// Record metrics for an LLM request.
#[allow(clippy::too_many_arguments)]
pub fn record_llm_request(
    provider: &str,
    model: &str,
    duration: Duration,
    prompt_tokens: u32,
    completion_tokens: u32,
    success: bool,
) {
    let r = registry();
    let status = if success { "success" } else { "error" };

    r.increment_counter(
        "clawdius_llm_requests_total",
        labels(&[("provider", provider), ("model", model), ("status", status)]),
    );

    r.observe_histogram(
        "clawdius_llm_request_duration_seconds",
        labels(&[("provider", provider), ("model", model)]),
        duration.as_secs_f64(),
    );

    r.add_counter(
        "clawdius_llm_tokens_total",
        labels(&[("type", "prompt")]),
        prompt_tokens as u64,
    );
    r.add_counter(
        "clawdius_llm_tokens_total",
        labels(&[("type", "completion")]),
        completion_tokens as u64,
    );
}

/// Record metrics for a tool execution.
pub fn record_tool_execution(tool: &str, duration: Duration, success: bool) {
    let r = registry();
    let status = if success { "success" } else { "error" };
    r.increment_counter(
        "clawdius_tool_executions_total",
        labels(&[("tool", tool), ("status", status)]),
    );
    r.observe_histogram(
        "clawdius_tool_duration_seconds",
        labels(&[("tool", tool)]),
        duration.as_secs_f64(),
    );
}

/// Record the active session count gauge.
pub fn record_session_count(count: usize) {
    registry().set_gauge("clawdius_active_sessions", BTreeMap::new(), count as f64);
}

/// Record metrics for a sandbox execution.
pub fn record_sandbox_execution(backend: &str, duration: Duration, success: bool) {
    let r = registry();
    let status = if success { "success" } else { "error" };
    r.increment_counter(
        "clawdius_sandbox_executions_total",
        labels(&[("backend", backend), ("status", status)]),
    );
    r.observe_histogram(
        "clawdius_sandbox_duration_seconds",
        labels(&[("backend", backend)]),
        duration.as_secs_f64(),
    );
}

/// Render the Prometheus metrics text for the `/metrics` endpoint.
pub fn render_metrics() -> String {
    registry().render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_valid_prometheus_text() {
        // Use a fresh local registry so the test is deterministic.
        let reg = MetricsRegistry::new();
        reg.increment_counter(
            "clawdius_llm_requests_total",
            labels(&[
                ("provider", "openai"),
                ("model", "gpt-4"),
                ("status", "success"),
            ]),
        );
        reg.add_counter(
            "clawdius_llm_tokens_total",
            labels(&[("type", "prompt")]),
            128,
        );
        reg.observe_histogram(
            "clawdius_llm_request_duration_seconds",
            labels(&[("provider", "openai"), ("model", "gpt-4")]),
            0.3,
        );
        reg.set_gauge("clawdius_active_sessions", BTreeMap::new(), 4.0);

        let out = reg.render();
        assert!(out.contains("# TYPE clawdius_llm_requests_total counter"));
        assert!(out.contains(
            "clawdius_llm_requests_total{model=\"gpt-4\",provider=\"openai\",status=\"success\"} 1"
        ));
        assert!(out.contains("clawdius_llm_tokens_total{type=\"prompt\"} 128"));
        assert!(out.contains("# TYPE clawdius_active_sessions gauge"));
        assert!(out.contains("clawdius_active_sessions 4"));
        assert!(out.contains("# TYPE clawdius_llm_request_duration_seconds histogram"));
        assert!(out.contains("clawdius_llm_request_duration_seconds_bucket{le=\"0.5\",model=\"gpt-4\",provider=\"openai\"} 1"));
        assert!(out.contains("clawdius_llm_request_duration_seconds_bucket{le=\"+Inf\",model=\"gpt-4\",provider=\"openai\"} 1"));
        assert!(out.contains(
            "clawdius_llm_request_duration_seconds_count{model=\"gpt-4\",provider=\"openai\"} 1"
        ));
    }

    #[test]
    fn convenience_functions_update_global_registry() {
        record_llm_request("openai", "gpt-4", Duration::from_millis(250), 10, 20, true);
        record_tool_execution("bash", Duration::from_millis(5), true);
        record_session_count(3);
        record_sandbox_execution("wasm", Duration::from_millis(40), false);

        let out = render_metrics();
        assert!(out.contains("clawdius_llm_tokens_total{type=\"completion\"} 20"));
        assert!(out.contains("clawdius_tool_executions_total{status=\"success\",tool=\"bash\"} 1"));
        assert!(out.contains("clawdius_active_sessions 3"));
        assert!(
            out.contains("clawdius_sandbox_executions_total{backend=\"wasm\",status=\"error\"} 1")
        );
    }
}
