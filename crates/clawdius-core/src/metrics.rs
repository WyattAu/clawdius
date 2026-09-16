//! Prometheus-compatible metrics for Clawdius (REQ-009).
//!
//! The registry now lives in the `clawdius-metrics` crate, where it is
//! re-expressed over the estate's `metrics-kit` engine (lock-free counters,
//! histograms, and gauges with a cardinality budget and spec-correct
//! Prometheus text exposition). This module re-exports that crate so the
//! historical `clawdius_core::metrics` paths — used by the web gateway,
//! the agent loop, and the CLI — keep working unchanged.

pub use clawdius_metrics::{
    labels, record_llm_request, record_sandbox_execution, record_session_count,
    record_tool_execution, registry, render_metrics, MetricsRegistry, DEFAULT_BUCKETS,
};
