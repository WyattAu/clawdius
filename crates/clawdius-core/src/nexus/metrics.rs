//! Prometheus-style metrics collection for Nexus FSM
//!
//! Provides metrics for phase transitions, gate evaluations, artifact operations,
//! and system performance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::config::MetricsConfig;
use super::PhaseId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub upper_bound: f64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    pub buckets: Vec<HistogramBucket>,
    pub sum: f64,
    pub count: u64,
}

impl Histogram {
    #[must_use]
    pub fn new(bounds: &[f64]) -> Self {
        let buckets = bounds
            .iter()
            .map(|&b| HistogramBucket {
                upper_bound: b,
                count: 0,
            })
            .collect();
        Self {
            buckets,
            sum: 0.0,
            count: 0,
        }
    }

    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
        for bucket in &mut self.buckets {
            if value <= bucket.upper_bound {
                bucket.count += 1;
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_by(&self, delta: u64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct Gauge {
    value: std::sync::atomic::AtomicI64,
    multiplier: f64,
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

impl Gauge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: std::sync::atomic::AtomicI64::new(0),
            multiplier: 1000.0,
        }
    }

    pub fn set(&self, value: f64) {
        self.value
            .store((value * self.multiplier) as i64, Ordering::Relaxed);
    }

    pub fn inc(&self) {
        self.value.fetch_add(1000, Ordering::Relaxed);
    }

    pub fn dec(&self) {
        self.value.fetch_sub(1000, Ordering::Relaxed);
    }

    pub fn get(&self) -> f64 {
        self.value.load(Ordering::Relaxed) as f64 / self.multiplier
    }
}

#[derive(Debug)]
pub struct Timer {
    start: Instant,
    histogram: Arc<parking_lot::Mutex<Histogram>>,
}

impl Timer {
    pub fn new(histogram: Arc<parking_lot::Mutex<Histogram>>) -> Self {
        Self {
            start: Instant::now(),
            histogram,
        }
    }

    #[must_use]
    pub fn stop(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let millis = elapsed.as_secs_f64() * 1000.0;
        self.histogram.lock().observe(millis);
        elapsed
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Debug)]
pub struct MetricsRegistry {
    prefix: String,
    counters: Arc<parking_lot::Mutex<HashMap<String, Arc<Counter>>>>,
    gauges: Arc<parking_lot::Mutex<HashMap<String, Arc<Gauge>>>>,
    histograms: Arc<parking_lot::Mutex<HashMap<String, Arc<parking_lot::Mutex<Histogram>>>>>,
    start_time: Instant,
}

impl MetricsRegistry {
    #[must_use]
    pub fn new(config: &MetricsConfig) -> Self {
        Self {
            prefix: config.prefix.clone(),
            counters: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            gauges: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            histograms: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            counters: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            gauges: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            histograms: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    fn prefixed_name(&self, name: &str) -> String {
        format!("{}_{}", self.prefix, name)
    }

    #[must_use]
    pub fn counter(&self, name: &str) -> Arc<Counter> {
        let full_name = self.prefixed_name(name);
        let mut counters = self.counters.lock();
        counters
            .entry(full_name.clone())
            .or_insert_with(|| Arc::new(Counter::new()))
            .clone()
    }

    #[must_use]
    pub fn gauge(&self, name: &str) -> Arc<Gauge> {
        let full_name = self.prefixed_name(name);
        let mut gauges = self.gauges.lock();
        gauges
            .entry(full_name.clone())
            .or_insert_with(|| Arc::new(Gauge::new()))
            .clone()
    }

    #[must_use]
    pub fn histogram(&self, name: &str) -> Arc<parking_lot::Mutex<Histogram>> {
        let full_name = self.prefixed_name(name);
        let mut histograms = self.histograms.lock();
        histograms
            .entry(full_name.clone())
            .or_insert_with(|| {
                Arc::new(parking_lot::Mutex::new(Histogram::new(&[
                    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
                ])))
            })
            .clone()
    }

    #[must_use]
    pub fn timer(&self, name: &str) -> Timer {
        Timer::new(self.histogram(name))
    }

    #[must_use]
    pub fn gather(&self) -> MetricsSnapshot {
        let counters: HashMap<String, u64> = self
            .counters
            .lock()
            .iter()
            .map(|(k, v)| (k.clone(), v.get()))
            .collect();

        let gauges: HashMap<String, f64> = self
            .gauges
            .lock()
            .iter()
            .map(|(k, v)| (k.clone(), v.get()))
            .collect();

        let histograms: HashMap<String, Histogram> = self
            .histograms
            .lock()
            .iter()
            .map(|(k, v)| (k.clone(), v.lock().clone()))
            .collect();

        MetricsSnapshot {
            counters,
            gauges,
            histograms,
            uptime_secs: self.start_time.elapsed().as_secs(),
            collected_at: Utc::now(),
        }
    }

    #[must_use]
    pub fn export_prometheus(&self) -> String {
        let snapshot = self.gather();
        let mut output = String::new();

        for (name, value) in &snapshot.counters {
            output.push_str(&format!("{name} {value}\n"));
        }

        for (name, value) in &snapshot.gauges {
            output.push_str(&format!("{name} {value:.3}\n"));
        }

        for (name, hist) in &snapshot.histograms {
            for bucket in &hist.buckets {
                output.push_str(&format!(
                    "{}_bucket{{le=\"{}\"}} {}\n",
                    name, bucket.upper_bound, bucket.count
                ));
            }
            output.push_str(&format!("{}_sum {:.3}\n", name, hist.sum));
            output.push_str(&format!("{}_count {}\n", name, hist.count));
        }

        output
    }

    pub fn reset(&self) {
        for counter in self.counters.lock().values() {
            counter.reset();
        }
        for gauge in self.gauges.lock().values() {
            gauge.set(0.0);
        }
        self.histograms.lock().clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, f64>,
    pub histograms: HashMap<String, Histogram>,
    pub uptime_secs: u64,
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricType {
    PhaseTransitionCount,
    PhaseTransitionDurationMs,
    GateEvaluationCount,
    GatePassedCount,
    GateFailedCount,
    GateEvaluationDurationMs,
    ArtifactCreatedCount,
    ArtifactRetrievedCount,
    ArtifactCacheHits,
    ArtifactCacheMisses,
    ErrorCount,
    RetryCount,
    CircuitBreakerTrips,
    EventBusPublishCount,
    EventBusSubscriberCount,
}

impl MetricType {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            MetricType::PhaseTransitionCount => "phase_transitions_total",
            MetricType::PhaseTransitionDurationMs => "phase_transition_duration_ms",
            MetricType::GateEvaluationCount => "gate_evaluations_total",
            MetricType::GatePassedCount => "gates_passed_total",
            MetricType::GateFailedCount => "gates_failed_total",
            MetricType::GateEvaluationDurationMs => "gate_evaluation_duration_ms",
            MetricType::ArtifactCreatedCount => "artifacts_created_total",
            MetricType::ArtifactRetrievedCount => "artifacts_retrieved_total",
            MetricType::ArtifactCacheHits => "artifact_cache_hits_total",
            MetricType::ArtifactCacheMisses => "artifact_cache_misses_total",
            MetricType::ErrorCount => "errors_total",
            MetricType::RetryCount => "retries_total",
            MetricType::CircuitBreakerTrips => "circuit_breaker_trips_total",
            MetricType::EventBusPublishCount => "events_published_total",
            MetricType::EventBusSubscriberCount => "event_subscribers",
        }
    }
}

#[derive(Debug)]
pub struct NexusMetrics {
    registry: Arc<MetricsRegistry>,
    pub phase_transitions: Arc<Counter>,
    pub phase_duration: Arc<parking_lot::Mutex<Histogram>>,
    pub gate_evaluations: Arc<Counter>,
    pub gates_passed: Arc<Counter>,
    pub gates_failed: Arc<Counter>,
    pub gate_duration: Arc<parking_lot::Mutex<Histogram>>,
    pub artifacts_created: Arc<Counter>,
    pub artifacts_retrieved: Arc<Counter>,
    pub cache_hits: Arc<Counter>,
    pub cache_misses: Arc<Counter>,
    pub errors: Arc<Counter>,
    pub retries: Arc<Counter>,
    pub circuit_trips: Arc<Counter>,
}

impl NexusMetrics {
    #[must_use]
    pub fn new(config: &MetricsConfig) -> Self {
        let registry = Arc::new(MetricsRegistry::new(config));

        Self {
            registry: registry.clone(),
            phase_transitions: registry.counter(MetricType::PhaseTransitionCount.name()),
            phase_duration: registry.histogram(MetricType::PhaseTransitionDurationMs.name()),
            gate_evaluations: registry.counter(MetricType::GateEvaluationCount.name()),
            gates_passed: registry.counter(MetricType::GatePassedCount.name()),
            gates_failed: registry.counter(MetricType::GateFailedCount.name()),
            gate_duration: registry.histogram(MetricType::GateEvaluationDurationMs.name()),
            artifacts_created: registry.counter(MetricType::ArtifactCreatedCount.name()),
            artifacts_retrieved: registry.counter(MetricType::ArtifactRetrievedCount.name()),
            cache_hits: registry.counter(MetricType::ArtifactCacheHits.name()),
            cache_misses: registry.counter(MetricType::ArtifactCacheMisses.name()),
            errors: registry.counter(MetricType::ErrorCount.name()),
            retries: registry.counter(MetricType::RetryCount.name()),
            circuit_trips: registry.counter(MetricType::CircuitBreakerTrips.name()),
        }
    }

    #[must_use]
    pub fn registry(&self) -> &Arc<MetricsRegistry> {
        &self.registry
    }

    pub fn record_phase_transition(&self, _from: PhaseId, _to: PhaseId, duration_ms: u64) {
        self.phase_transitions.inc();
        self.phase_duration.lock().observe(duration_ms as f64);
    }

    pub fn record_gate_evaluation(&self, _gate_id: &str, passed: bool, duration_ms: u64) {
        self.gate_evaluations.inc();
        if passed {
            self.gates_passed.inc();
        } else {
            self.gates_failed.inc();
        }
        self.gate_duration.lock().observe(duration_ms as f64);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.inc();
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.inc();
    }

    pub fn record_error(&self) {
        self.errors.inc();
    }

    pub fn record_retry(&self) {
        self.retries.inc();
    }

    pub fn record_circuit_trip(&self) {
        self.circuit_trips.inc();
    }

    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        self.registry.gather()
    }

    #[must_use]
    pub fn prometheus_export(&self) -> String {
        self.registry.export_prometheus()
    }

    #[must_use]
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.get() as f64;
        let misses = self.cache_misses.get() as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

impl Default for NexusMetrics {
    fn default() -> Self {
        Self::new(&MetricsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_operations() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);
        counter.inc();
        assert_eq!(counter.get(), 1);
        counter.inc_by(5);
        assert_eq!(counter.get(), 6);
        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge_operations() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0.0);
        gauge.set(42.5);
        assert!((gauge.get() - 42.5).abs() < 0.001);
        gauge.inc();
        assert!((gauge.get() - 43.5).abs() < 0.001);
        gauge.dec();
        assert!((gauge.get() - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_histogram() {
        let mut hist = Histogram::new(&[1.0, 5.0, 10.0]);
        hist.observe(0.5);
        hist.observe(3.0);
        hist.observe(7.0);
        hist.observe(15.0);

        assert_eq!(hist.count, 4);
        assert!((hist.sum - 25.5).abs() < 0.001);
        assert_eq!(hist.buckets[0].count, 1);
        assert_eq!(hist.buckets[1].count, 2);
        assert_eq!(hist.buckets[2].count, 3);
    }

    #[test]
    fn test_metrics_registry() {
        let registry = MetricsRegistry::with_prefix("test");

        let counter = registry.counter("requests");
        counter.inc();
        counter.inc();

        let gauge = registry.gauge("temperature");
        gauge.set(25.5);

        let snapshot = registry.gather();
        assert_eq!(snapshot.counters.get("test_requests"), Some(&2));
        assert!(
            (snapshot
                .gauges
                .get("test_temperature")
                .copied()
                .unwrap_or(0.0)
                - 25.5)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn test_nexus_metrics() {
        let metrics = NexusMetrics::default();

        metrics.record_phase_transition(PhaseId(0), PhaseId(1), 150);
        assert_eq!(metrics.phase_transitions.get(), 1);

        metrics.record_gate_evaluation("test_gate", true, 25);
        assert_eq!(metrics.gate_evaluations.get(), 1);
        assert_eq!(metrics.gates_passed.get(), 1);

        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        assert!((metrics.cache_hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_prometheus_export() {
        let registry = MetricsRegistry::with_prefix("nexus");
        let counter = registry.counter("operations");
        counter.inc_by(42);

        let export = registry.export_prometheus();
        assert!(export.contains("nexus_operations 42"));
    }

    #[test]
    fn test_timer() {
        let registry = MetricsRegistry::with_prefix("test");
        let histogram = registry.histogram("duration");

        {
            let _timer = Timer::new(histogram.clone());
            std::thread::sleep(Duration::from_millis(10));
        }

        let hist = histogram.lock();
        assert!(hist.count >= 1);
        assert!(hist.sum > 0.0);
    }
}
