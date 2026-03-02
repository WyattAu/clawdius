# Monitoring Strategy

**Document ID:** MS-CLAWDIUS-011  
**Version:** 1.0.0  
**Phase:** 11 (Continuous Monitoring)  
**Date:** 2026-03-02  
**Status:** APPROVED

---

## 1. Executive Summary

This document defines the comprehensive monitoring strategy for Clawdius production deployments, covering metrics collection, dashboards, and observability architecture.

### 1.1 Monitoring Scope

| Layer | Components | Priority |
|-------|------------|----------|
| Infrastructure | CPU, Memory, Disk, Network | P1 |
| Application | FSM, Sentinel, Brain, Broker | P0 |
| HFT Critical | Ring Buffer, Wallet Guard | P0 |
| Security | Sandbox events, Capability usage | P1 |
| Business | Chat sessions, Refactoring jobs | P2 |

---

## 2. Metrics Architecture

### 2.1 Collection Stack

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MONITORING ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐    │
│  │  Clawdius   │───►│  Prometheus │───►│     Grafana         │    │
│  │  /metrics   │    │  (15s scrape)│    │   Dashboards        │    │
│  └─────────────┘    └─────────────┘    └─────────────────────┘    │
│         │                  │                     │                 │
│         │                  ▼                     ▼                 │
│         │           ┌─────────────┐    ┌─────────────────────┐    │
│         │           │ Alertmanager│───►│   PagerDuty/Slack   │    │
│         │           └─────────────┘    └─────────────────────┘    │
│         │                                                           │
│         ▼                                                           │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐    │
│  │   Traces    │───►│    Jaeger   │───►│  Distributed UI     │    │
│  │  OpenTelem  │    │   Backend   │    │                     │    │
│  └─────────────┘    └─────────────┘    └─────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'clawdius'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: /metrics
    scheme: http

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['localhost:9093']

rule_files:
  - /etc/prometheus/alerts/clawdius.yml
```

---

## 3. Metrics Catalog

### 3.1 HFT Critical Metrics (P0)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `clawdius_hft_signal_latency_ns` | Histogram | tier, symbol | Signal-to-execution latency |
| `clawdius_hft_ring_buffer_ops_total` | Counter | op (push/pop) | Ring buffer operations |
| `clawdius_hft_ring_buffer_capacity` | Gauge | - | Ring buffer capacity |
| `clawdius_hft_wallet_guard_checks_total` | Counter | result | Risk check invocations |
| `clawdius_hft_wallet_guard_latency_ns` | Histogram | - | Risk check latency |
| `clawdius_hft_market_data_messages_total` | Counter | source | Market data messages received |
| `clawdius_hft_gc_pause_ns` | Gauge | - | GC pause time (should be 0) |

**Histogram Buckets (HFT):**
```yaml
# Sub-microsecond precision
hft_latency_buckets: [100ns, 500ns, 1µs, 2µs, 5µs, 10µs, 50µs, 100µs, 500µs, 1ms]
```

### 3.2 Application Metrics (P0)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `clawdius_fsm_phase_transitions_total` | Counter | from_phase, to_phase | FSM transitions |
| `clawdius_fsm_phase_duration_seconds` | Histogram | phase | Time in each phase |
| `clawdius_fsm_quality_gates_passed_total` | Counter | gate_id | Quality gate passes |
| `clawdius_fsm_quality_gates_failed_total` | Counter | gate_id | Quality gate failures |
| `clawdius_sentinel_sandbox_spawn_total` | Counter | tier, result | Sandbox spawns |
| `clawdius_sentinel_sandbox_spawn_duration_ms` | Histogram | tier | Sandbox spawn time |
| `clawdius_sentinel_sandbox_active_count` | Gauge | tier | Active sandboxes |
| `clawdius_brain_wasm_invocations_total` | Counter | function, result | WASM calls |
| `clawdius_brain_wasm_fuel_consumed_total` | Counter | function | Fuel consumed |
| `clawdius_brain_llm_requests_total` | Counter | provider, result | LLM API calls |
| `clawdius_brain_llm_latency_seconds` | Histogram | provider | LLM response time |
| `clawdius_graph_rag_queries_total` | Counter | type (ast/vector), result | Graph-RAG queries |
| `clawdius_graph_rag_query_latency_seconds` | Histogram | type | Query latency |
| `clawdius_graph_rag_files_indexed` | Gauge | - | Files in index |

### 3.3 Infrastructure Metrics (P1)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `clawdius_process_cpu_seconds_total` | Counter | - | CPU time consumed |
| `clawdius_process_resident_memory_bytes` | Gauge | - | RSS memory |
| `clawdius_process_open_fds` | Gauge | - | Open file descriptors |
| `clawdius_process_threads` | Gauge | - | Thread count |
| `clawdius_db_connections_active` | Gauge | db (sqlite/lancedb) | Active DB connections |
| `clawdius_db_query_latency_seconds` | Histogram | db, query_type | DB query time |
| `clawdius_db_errors_total` | Counter | db, error_type | DB errors |

### 3.4 Security Metrics (P1)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `clawdius_security_capability_issued_total` | Counter | permission | Capabilities issued |
| `clawdius_security_capability_denied_total` | Counter | permission | Capability denials |
| `clawdius_security_sandbox_violations_total` | Counter | tier, violation_type | Sandbox violations |
| `clawdius_security_input_validation_errors_total` | Counter | input_type | Validation failures |
| `clawdius_security_secrets_accessed_total` | Counter | secret_id | Secret access count |

### 3.5 Business Metrics (P2)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `clawdius_chat_sessions_total` | Counter | result | Chat sessions |
| `clawdius_chat_response_latency_seconds` | Histogram | provider | Chat response time |
| `clawdius_refactor_jobs_total` | Counter | result | Refactoring jobs |
| `clawdius_refactor_files_processed` | Counter | - | Files processed |
| `clawdius_tui_renders_total` | Counter | - | TUI frame renders |
| `clawdius_tui_frame_time_ms` | Histogram | - | Frame render time |

---

## 4. Dashboards

### 4.1 HFT Operations Dashboard

```
┌─────────────────────────────────────────────────────────────────────┐
│                     HFT OPERATIONS DASHBOARD                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────┐  ┌───────────────────────────────────┐  │
│  │   Signal Latency      │  │    Ring Buffer Utilization        │  │
│  │   P99: 0.89ms ✅      │  │    ████████░░░░░░░░ 52%          │  │
│  │   Target: <1ms        │  │    Depth: 524,288 / 1,048,576    │  │
│  └───────────────────────┘  └───────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────┐  ┌───────────────────────────────────┐  │
│  │   Wallet Guard        │  │    Market Data Rate               │  │
│  │   P99: 847ns ✅       │  │    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁               │  │
│  │   Target: <100µs      │  │    8.2M msg/s                     │  │
│  └───────────────────────┘  └───────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Signal Latency Heatmap                      │  │
│  │  [Time vs Latency distribution - 24 hour view]                │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Panels:**

| Panel | Type | Query | Refresh |
|-------|------|-------|---------|
| Signal Latency P99 | Stat | `histogram_quantile(0.99, rate(clawdius_hft_signal_latency_ns_bucket[5m]))` | 5s |
| Ring Buffer Depth | Gauge | `clawdius_hft_ring_buffer_capacity - clawdius_hft_ring_buffer_depth` | 5s |
| Wallet Guard P99 | Stat | `histogram_quantile(0.99, rate(clawdius_hft_wallet_guard_latency_ns_bucket[5m]))` | 5s |
| Market Data Rate | Graph | `rate(clawdius_hft_market_data_messages_total[1m])` | 5s |
| Latency Heatmap | Heatmap | `clawdius_hft_signal_latency_ns_bucket` | 10s |

### 4.2 System Health Dashboard

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SYSTEM HEALTH DASHBOARD                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────┐  │
│  │   Memory          │  │   CPU             │  │   FDs         │  │
│  │   42MB / 54MB     │  │   12%             │  │   23 / 64     │  │
│  │   ████████░░      │  │   ██░░░░░░░░      │  │   ████░░░░    │  │
│  └───────────────────┘  └───────────────────┘  └───────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    FSM Phase Distribution                      │  │
│  │  [Bar chart of phase transition counts]                       │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌─────────────────────────────┐  ┌─────────────────────────────┐  │
│  │   Sandbox Pool Status       │  │   DB Connection Pool        │  │
│  │   Tier 1: 4 active          │  │   SQLite: 6/8 active        │  │
│  │   Tier 2: 2 active          │  │   LanceDB: 3/16 active      │  │
│  │   Tier 3: 1 active          │  │                             │  │
│  └─────────────────────────────┘  └─────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.3 Security Dashboard

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SECURITY DASHBOARD                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │   Capability Denials (Last 24h)    [0 events] ✅              │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │   Sandbox Violations (Last 24h)    [0 events] ✅              │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │   Input Validation Errors         [12 events - normal]        │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │   Top Permissions Used                                          │  │
│  │   FS_READ: ████████████████ 847                                │  │
│  │   FS_WRITE: ████████ 423                                       │  │
│  │   NET_TCP: ████ 156                                            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. Metrics Endpoint Specification

### 5.1 Endpoint Design

```
GET /metrics HTTP/1.1
Host: localhost:9090
Accept: text/plain

# Response format: Prometheus exposition format
# HELP clawdius_hft_signal_latency_ns Signal-to-execution latency
# TYPE clawdius_hft_signal_latency_ns histogram
clawdius_hft_signal_latency_ns_bucket{le="100"} 0
clawdius_hft_signal_latency_ns_bucket{le="500"} 1523
clawdius_hft_signal_latency_ns_bucket{le="1000"} 8234
clawdius_hft_signal_latency_ns_bucket{le="+Inf"} 8500
clawdius_hft_signal_latency_ns_sum 5234567
clawdius_hft_signal_latency_ns_count 8500
```

### 5.2 Health Check Endpoints

| Endpoint | Purpose | Response Codes |
|----------|---------|----------------|
| `/health/live` | Liveness probe | 200 = alive |
| `/health/ready` | Readiness probe | 200 = ready, 503 = not ready |
| `/health/startup` | Startup probe | 200 = started |
| `/metrics` | Prometheus metrics | 200 = metrics |

### 5.3 Readiness Conditions

```rust
pub fn is_ready(&self) -> bool {
    self.runtime_initialized
        && self.database_connected
        && self.sandbox_pool_available
        && !self.shutting_down
}
```

---

## 6. Distributed Tracing

### 6.1 Trace Instrumentation

```rust
use opentelemetry::trace::{Tracer, Span};

#[instrument(skip(self))]
pub async fn process_chat_request(&self, request: ChatRequest) -> Result<ChatResponse> {
    let span = tracing::span!(Level::INFO, "chat_request", request_id = %request.id);
    let _enter = span.enter();
    
    // ... processing
    
    Ok(response)
}
```

### 6.2 Key Spans

| Operation | Span Name | Attributes |
|-----------|-----------|------------|
| FSM Transition | `fsm.transition` | from_phase, to_phase, duration |
| Sandbox Spawn | `sandbox.spawn` | tier, command, duration |
| LLM Request | `llm.request` | provider, model, tokens |
| HFT Signal | `hft.signal` | symbol, latency_ns |
| Graph-RAG Query | `graph_rag.query` | query_type, result_count |

---

## 7. Log Aggregation

### 7.1 Structured Logging Format

```json
{
  "timestamp": "2026-03-02T10:30:00.123Z",
  "level": "INFO",
  "target": "clawdius::fsm",
  "message": "Phase transition completed",
  "fields": {
    "from_phase": "RequirementsEngineering",
    "to_phase": "ArchitectureRefinement",
    "duration_ms": 42
  },
  "trace_id": "abc123",
  "span_id": "def456"
}
```

### 7.2 Log Levels by Component

| Component | Default | HFT Mode | Debug |
|-----------|---------|----------|-------|
| FSM | INFO | WARN | DEBUG |
| Sentinel | INFO | WARN | DEBUG |
| Brain | INFO | WARN | DEBUG |
| HFT Broker | WARN | ERROR | DEBUG |
| Graph-RAG | INFO | WARN | DEBUG |
| TUI | WARN | ERROR | DEBUG |

---

## 8. Retention Policies

### 8.1 Metrics Retention

| Resolution | Retention | Storage |
|------------|-----------|---------|
| 15s (raw) | 7 days | Prometheus local |
| 5m (downsampled) | 30 days | Prometheus local |
| 1h (aggregated) | 1 year | Thanos/Cortex |

### 8.2 Logs Retention

| Log Type | Retention | Storage |
|----------|-----------|---------|
| Application logs | 30 days | Loki |
| Security logs | 1 year | Loki |
| Audit logs | 7 years | S3/Glacier |
| HFT trace logs | 7 days | Jaeger |

---

## 9. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| All P0 metrics defined | ✅ | Section 3.1-3.2 |
| Prometheus endpoint implemented | ✅ | Section 5.1 |
| Health checks defined | ✅ | Section 5.2 |
| Dashboards designed | ✅ | Section 4 |
| Distributed tracing configured | ✅ | Section 6 |
| Log aggregation configured | ✅ | Section 7 |
| Retention policies defined | ✅ | Section 8 |

---

**Document Status:** APPROVED  
**Next Review:** After production deployment  
**Sign-off:** Operations Team
