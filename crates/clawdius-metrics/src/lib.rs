//! Prometheus-compatible metrics for Clawdius (REQ-009).
//!
//! This crate preserves the historical Clawdius metrics seam — record-time
//! counters, histograms, and gauges rendered as Prometheus text — while the
//! engine is delegated to the estate's `metrics-kit` crate (lock-free
//! cache-line-padded atomics, cardinality guards, and spec-correct
//! exposition escaping). See <https://github.com/WyattAu/metrics-kit>.
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
//!
//! # Cardinality budget
//!
//! Every registry — including the process-wide [`registry`] handle that the
//! gateway scrapes at `/metrics` — enforces a budget of [`MAX_SERIES`]
//! series via `metrics_kit::Registry::with_max_series`. The first
//! observation of a unique (metric name, label set) pair lazily registers
//! the series; once the budget is exhausted, further *new* series are
//! dropped (the sample is discarded) instead of growing the exposition
//! without bound. Updates to already-registered series are never dropped.
//!
//! # Exposition compatibility
//!
//! Rendering is delegated to `metrics_kit::Registry::render()` (Prometheus
//! text exposition format 0.0.4). Relative to the previous hand-rolled
//! formatter:
//!
//! - `# HELP` lines are emitted before each `# TYPE` line.
//! - Label values and help strings are escaped per the exposition spec.
//! - Histogram bucket bounds render as a second brace group after the
//!   series labels (e.g. `x_bucket{a="b"}{le="1"} 3`) instead of being
//!   merged into the sorted label set.
//! - Integral floats render without a trailing `.0` (bucket bound `1.0`
//!   renders as `le="1"`).

use std::collections::{BTreeMap, HashMap};
use std::sync::{OnceLock, PoisonError, RwLock};
use std::time::Duration;

use metrics_kit::{Counter, Gauge, Histogram, Registry};

/// Cardinality budget for every Clawdius registry.
///
/// Maximum series admitted before new series are dropped. Sized for the
/// gateway's dynamic label values while protecting the scrape target from
/// cardinality growth.
pub const MAX_SERIES: usize = 8192;

/// Default histogram bucket upper bounds, matching the Prometheus
/// `DefBuckets` (`0.005` .. `10.0`).
pub const DEFAULT_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Global metrics registry handle.
static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

/// Access the global [`MetricsRegistry`], initializing it on first use.
///
/// The global registry carries the [`MAX_SERIES`] cardinality budget and
/// backs [`render_metrics`], which the gateway serves at `/metrics`.
pub fn registry() -> &'static MetricsRegistry {
    REGISTRY.get_or_init(MetricsRegistry::new)
}

/// A Prometheus-compatible metrics registry.
///
/// The public API is the historical Clawdius metrics seam: record-time
/// `(name, labels)` updates plus text rendering via [`MetricsRegistry::render`].
/// Internally, each unique (name, label set) pair is lazily registered with
/// a `metrics_kit::Registry` on first observation; subsequent updates hit
/// the returned lock-free handle and never take a write lock.
pub struct MetricsRegistry {
    /// Engine registry: owns the series and renders the exposition.
    inner: Registry,
    /// Lazily registered counter handles keyed by [`metric_key`].
    counters: RwLock<HashMap<String, Counter>>,
    /// Lazily registered histogram handles keyed by [`metric_key`].
    histograms: RwLock<HashMap<String, Histogram>>,
    /// Lazily registered gauge handles keyed by [`metric_key`].
    gauges: RwLock<HashMap<String, Gauge>>,
}

impl MetricsRegistry {
    /// Create a new empty registry with the [`MAX_SERIES`] budget.
    fn new() -> Self {
        Self::with_engine(Registry::with_max_series(MAX_SERIES))
    }

    /// Wrap an engine registry (test seam for small cardinality budgets).
    fn with_engine(inner: Registry) -> Self {
        Self {
            inner,
            counters: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
        }
    }

    /// Add `value` to the counter identified by `name` + `labels`,
    /// lazily registering the series on first observation.
    ///
    /// If registration fails (cardinality budget exhausted, or an invalid
    /// metric or label name), the sample is dropped.
    pub fn add_counter(&self, name: &str, labels: &BTreeMap<String, String>, value: u64) {
        if let Some(counter) = self.counter_handle(name, labels) {
            counter.add(value);
        }
    }

    /// Increment a counter by one.
    pub fn increment_counter(&self, name: &str, labels: &BTreeMap<String, String>) {
        self.add_counter(name, labels, 1);
    }

    /// Record a single observation `value` into a histogram using
    /// [`DEFAULT_BUCKETS`].
    pub fn observe_histogram(&self, name: &str, labels: &BTreeMap<String, String>, value: f64) {
        self.observe_histogram_with_buckets(name, labels, value, DEFAULT_BUCKETS);
    }

    /// Record an observation into a histogram with custom bucket bounds.
    ///
    /// The bucket bounds of a series are fixed by its first observation;
    /// later observations with different bounds reuse the registered
    /// series (historical behavior, preserved).
    pub fn observe_histogram_with_buckets(
        &self,
        name: &str,
        labels: &BTreeMap<String, String>,
        value: f64,
        buckets: &[f64],
    ) {
        if let Some(histogram) = self.histogram_handle(name, labels, buckets) {
            histogram.observe(value);
        }
    }

    /// Set a gauge to `value`.
    pub fn set_gauge(&self, name: &str, labels: &BTreeMap<String, String>, value: f64) {
        if let Some(gauge) = self.gauge_handle(name, labels) {
            gauge.set(value);
        }
    }

    /// Render the entire registry in Prometheus text exposition format.
    pub fn render(&self) -> String {
        self.inner.render()
    }

    /// Number of registered series (budget monitoring and tests).
    pub fn series_count(&self) -> usize {
        self.inner.series_count()
    }

    /// Look up or lazily register the counter for `(name, labels)`.
    fn counter_handle(&self, name: &str, labels: &BTreeMap<String, String>) -> Option<Counter> {
        let key = metric_key(name, labels);
        if let Some(handle) = self
            .counters
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Some(handle.clone());
        }
        let mut handles = self
            .counters
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = handles.get(&key) {
            let handle = handle.clone();
            drop(handles);
            return Some(handle);
        }
        let pairs: Vec<(&str, &str)> = labels
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        // Budget exhausted or invalid identifiers: drop the sample.
        let registered = self
            .inner
            .counter(name, help_for(name), &pairs)
            .ok()
            .inspect(|handle| {
                handles.insert(key, handle.clone());
            });
        drop(handles);
        registered
    }

    /// Look up or lazily register the histogram for `(name, labels)`,
    /// fixing `buckets` on first registration.
    fn histogram_handle(
        &self,
        name: &str,
        labels: &BTreeMap<String, String>,
        buckets: &[f64],
    ) -> Option<Histogram> {
        let key = metric_key(name, labels);
        if let Some(handle) = self
            .histograms
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Some(handle.clone());
        }
        let mut handles = self
            .histograms
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = handles.get(&key) {
            let handle = handle.clone();
            drop(handles);
            return Some(handle);
        }
        let pairs: Vec<(&str, &str)> = labels
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        // Budget exhausted or invalid identifiers/buckets: drop the sample.
        let registered = self
            .inner
            .histogram_with_buckets(name, help_for(name), &pairs, buckets.to_vec())
            .ok()
            .inspect(|handle| {
                handles.insert(key, handle.clone());
            });
        drop(handles);
        registered
    }

    /// Look up or lazily register the gauge for `(name, labels)`.
    fn gauge_handle(&self, name: &str, labels: &BTreeMap<String, String>) -> Option<Gauge> {
        let key = metric_key(name, labels);
        if let Some(handle) = self
            .gauges
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Some(handle.clone());
        }
        let mut handles = self.gauges.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = handles.get(&key) {
            let handle = handle.clone();
            drop(handles);
            return Some(handle);
        }
        let pairs: Vec<(&str, &str)> = labels
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        // Budget exhausted or invalid identifiers: drop the sample.
        let registered = self
            .inner
            .gauge(name, help_for(name), &pairs)
            .ok()
            .inspect(|handle| {
                handles.insert(key, handle.clone());
            });
        drop(handles);
        registered
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a label map from `(&str, &str)` pairs.
#[must_use]
pub fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Stable memoization key for a `(name, labels)` pair (historical format).
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

/// Help text for the known Clawdius metric families; unknown families fall
/// back to their own name (HELP is a new exposition line and optional for
/// scrapers).
fn help_for(name: &str) -> &str {
    match name {
        "clawdius_llm_requests_total" => "Total LLM requests.",
        "clawdius_llm_request_duration_seconds" => "LLM request duration in seconds.",
        "clawdius_llm_tokens_total" => "Total LLM tokens processed.",
        "clawdius_tool_executions_total" => "Total tool executions.",
        "clawdius_tool_duration_seconds" => "Tool execution duration in seconds.",
        "clawdius_sandbox_executions_total" => "Total sandbox executions.",
        "clawdius_sandbox_duration_seconds" => "Sandbox execution duration in seconds.",
        "clawdius_active_sessions" => "Currently active sessions.",
        other => other,
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
        &labels(&[("provider", provider), ("model", model), ("status", status)]),
    );

    r.observe_histogram(
        "clawdius_llm_request_duration_seconds",
        &labels(&[("provider", provider), ("model", model)]),
        duration.as_secs_f64(),
    );

    r.add_counter(
        "clawdius_llm_tokens_total",
        &labels(&[("type", "prompt")]),
        u64::from(prompt_tokens),
    );
    r.add_counter(
        "clawdius_llm_tokens_total",
        &labels(&[("type", "completion")]),
        u64::from(completion_tokens),
    );
}

/// Record metrics for a tool execution.
pub fn record_tool_execution(tool: &str, duration: Duration, success: bool) {
    let r = registry();
    let status = if success { "success" } else { "error" };
    r.increment_counter(
        "clawdius_tool_executions_total",
        &labels(&[("tool", tool), ("status", status)]),
    );
    r.observe_histogram(
        "clawdius_tool_duration_seconds",
        &labels(&[("tool", tool)]),
        duration.as_secs_f64(),
    );
}

/// Record the active session count gauge.
pub fn record_session_count(count: usize) {
    #[allow(clippy::cast_precision_loss)]
    // session count: precision beyond 2^52 is irrelevant for a gauge
    registry().set_gauge("clawdius_active_sessions", &BTreeMap::new(), count as f64);
}

/// Record metrics for a sandbox execution.
pub fn record_sandbox_execution(backend: &str, duration: Duration, success: bool) {
    let r = registry();
    let status = if success { "success" } else { "error" };
    r.increment_counter(
        "clawdius_sandbox_executions_total",
        &labels(&[("backend", backend), ("status", status)]),
    );
    r.observe_histogram(
        "clawdius_sandbox_duration_seconds",
        &labels(&[("backend", backend)]),
        duration.as_secs_f64(),
    );
}

/// Render the Prometheus metrics text for the `/metrics` endpoint.
#[must_use]
pub fn render_metrics() -> String {
    registry().render()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::items_after_statements
    )]
    use super::*;

    #[test]
    fn renders_valid_prometheus_text() {
        // Use a fresh local registry so the test is deterministic.
        let reg = MetricsRegistry::default();
        reg.increment_counter(
            "clawdius_llm_requests_total",
            &labels(&[
                ("provider", "openai"),
                ("model", "gpt-4"),
                ("status", "success"),
            ]),
        );
        reg.add_counter(
            "clawdius_llm_tokens_total",
            &labels(&[("type", "prompt")]),
            128,
        );
        reg.observe_histogram(
            "clawdius_llm_request_duration_seconds",
            &labels(&[("provider", "openai"), ("model", "gpt-4")]),
            0.3,
        );
        reg.set_gauge("clawdius_active_sessions", &BTreeMap::new(), 4.0);

        let out = reg.render();
        assert!(out.contains("# TYPE clawdius_llm_requests_total counter"));
        assert!(out.contains(
            "clawdius_llm_requests_total{model=\"gpt-4\",provider=\"openai\",status=\"success\"} 1"
        ));
        assert!(out.contains("clawdius_llm_tokens_total{type=\"prompt\"} 128"));
        assert!(out.contains("# TYPE clawdius_active_sessions gauge"));
        assert!(out.contains("clawdius_active_sessions 4"));
        assert!(out.contains("# TYPE clawdius_llm_request_duration_seconds histogram"));
        // Exposition is now rendered by metrics-kit: the `le` bucket label is
        // emitted as its own brace group after the series labels (it used to
        // be merged into the sorted label set).
        assert!(out.contains("clawdius_llm_request_duration_seconds_bucket{model=\"gpt-4\",provider=\"openai\"}{le=\"0.5\"} 1"));
        assert!(out.contains("clawdius_llm_request_duration_seconds_bucket{model=\"gpt-4\",provider=\"openai\"}{le=\"+Inf\"} 1"));
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

    /// Concurrent registration (unique label sets per thread) interleaved
    /// with concurrent renders must be lossless: every series survives and
    /// every update is accounted for.
    #[test]
    fn concurrent_registration_and_render_is_lossless() {
        use std::sync::Arc;

        const THREADS: u64 = 8;
        const ITERATIONS: u64 = 2_000;

        let reg = Arc::new(MetricsRegistry::default());

        let renderer = {
            let reg = Arc::clone(&reg);
            std::thread::spawn(move || {
                for _ in 0..256 {
                    // Empty until the first registration lands; the point is
                    // to interleave renders with concurrent registration.
                    let _ = reg.render();
                    std::thread::yield_now();
                }
            })
        };

        let workers: Vec<_> = (0..THREADS)
            .map(|t| {
                let reg = Arc::clone(&reg);
                std::thread::spawn(move || {
                    let tag = t.to_string();
                    for _ in 0..ITERATIONS {
                        reg.increment_counter(
                            "concurrent_total",
                            &labels(&[("thread", tag.as_str())]),
                        );
                        reg.observe_histogram(
                            "concurrent_duration_seconds",
                            &labels(&[("thread", tag.as_str())]),
                            0.5,
                        );
                        reg.set_gauge("concurrent_active", &BTreeMap::new(), t as f64);
                    }
                })
            })
            .collect();

        for worker in workers {
            worker.join().expect("worker thread should not panic");
        }
        renderer.join().expect("renderer thread should not panic");

        let out = reg.render();
        for t in 0..THREADS {
            assert!(
                out.contains(&format!("concurrent_total{{thread=\"{t}\"}} {ITERATIONS}")),
                "lossless counter expected for thread {t}"
            );
        }
        assert_eq!(
            reg.series_count(),
            (THREADS * 2 + 1) as usize,
            "8 counter + 8 histogram series plus one gauge"
        );
    }

    #[test]
    fn custom_buckets_map_to_histogram_with_buckets() {
        let reg = MetricsRegistry::default();
        let buckets = [1.0, 2.0];
        reg.observe_histogram_with_buckets(
            "custom_duration_seconds",
            &BTreeMap::new(),
            0.5,
            &buckets,
        );
        reg.observe_histogram_with_buckets(
            "custom_duration_seconds",
            &BTreeMap::new(),
            1.5,
            &buckets,
        );
        reg.observe_histogram_with_buckets(
            "custom_duration_seconds",
            &BTreeMap::new(),
            5.0,
            &buckets,
        );

        let out = reg.render();
        // metrics-kit renders integral floats without a trailing `.0`
        // (`le="1"`, previously `le="1.0"`) — both parse identically.
        assert!(out.contains(r#"custom_duration_seconds_bucket{le="1"} 1"#));
        assert!(out.contains(r#"custom_duration_seconds_bucket{le="2"} 2"#));
        assert!(out.contains(r#"custom_duration_seconds_bucket{le="+Inf"} 3"#));
        assert!(out.contains("custom_duration_seconds_sum 7"));
        assert!(out.contains("custom_duration_seconds_count 3"));
    }

    #[test]
    fn label_values_are_escaped_per_spec() {
        let reg = MetricsRegistry::default();
        reg.set_gauge("escaped_demo", &labels(&[("path", "a\"b\\c\nd")]), 1.0);

        let out = reg.render();
        // The hand-rolled formatter emitted raw bytes; metrics-kit escapes
        // quotes, backslashes, and newlines per the exposition spec.
        assert!(out.contains(r#"escaped_demo{path="a\"b\\c\nd"} 1"#));
    }

    #[test]
    fn cardinality_budget_drops_new_series_only() {
        let reg = MetricsRegistry::with_engine(Registry::with_max_series(2));
        reg.increment_counter("budget_total", &labels(&[("n", "1")]));
        reg.increment_counter("budget_total", &labels(&[("n", "2")]));
        // Third unique series exceeds the budget and is dropped.
        reg.increment_counter("budget_total", &labels(&[("n", "3")]));
        // Updates to admitted series are never dropped.
        reg.increment_counter("budget_total", &labels(&[("n", "1")]));

        let out = reg.render();
        assert!(out.contains(r#"budget_total{n="1"} 2"#));
        assert!(out.contains(r#"budget_total{n="2"} 1"#));
        assert!(!out.contains(r#"n="3""#));
    }
}
