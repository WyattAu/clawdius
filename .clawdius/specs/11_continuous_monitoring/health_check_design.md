# Health Check Endpoints Design

**Document ID:** HC-CLAWDIUS-011  
**Version:** 1.0.0  
**Phase:** 11 (Continuous Monitoring)  
**Date:** 2026-03-02  
**Status:** APPROVED

---

## 1. Executive Summary

This document defines the health check API for Clawdius, including liveness, readiness, and startup probes for Kubernetes/orchestration integration.

### 1.1 Endpoint Overview

| Endpoint | Purpose | Check Type | Expected Use |
|----------|---------|------------|--------------|
| `/health/live` | Liveness | Shallow | Kill & restart if failing |
| `/health/ready` | Readiness | Deep | Remove from load balancer |
| `/health/startup` | Startup | Initialization | Block until ready |
| `/metrics` | Prometheus | Metrics | Monitoring scrape |

---

## 2. API Specification

### 2.1 Liveness Probe

**Purpose:** Determine if the process is alive and not deadlocked.

```
GET /health/live HTTP/1.1
Host: localhost:9090

HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "alive",
  "timestamp": "2026-03-02T10:30:00.123Z"
}
```

**Response Codes:**
| Code | Meaning | Action |
|------|---------|--------|
| 200 | Process is alive | Continue |
| 500 | Internal error | Restart container |
| Timeout (5s) | Deadlocked | Restart container |

**Implementation:**
```rust
pub async fn liveness() -> impl IntoResponse {
    Json(json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
```

**Kubernetes Configuration:**
```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 9090
  initialDelaySeconds: 5
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3
```

---

### 2.2 Readiness Probe

**Purpose:** Determine if the process is ready to accept traffic.

```
GET /health/ready HTTP/1.1
Host: localhost:9090

HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "ready",
  "checks": {
    "runtime": "ok",
    "database": "ok",
    "sandbox_pool": "ok",
    "brain_wasm": "ok"
  },
  "timestamp": "2026-03-02T10:30:00.123Z"
}
```

**Failure Response:**
```
HTTP/1.1 503 Service Unavailable
Content-Type: application/json

{
  "status": "not_ready",
  "checks": {
    "runtime": "ok",
    "database": "failed: connection refused",
    "sandbox_pool": "ok",
    "brain_wasm": "ok"
  },
  "timestamp": "2026-03-02T10:30:00.123Z"
}
```

**Response Codes:**
| Code | Meaning | Action |
|------|---------|--------|
| 200 | All checks passed | Route traffic |
| 503 | One or more checks failed | Do not route traffic |

**Readiness Conditions:**
```rust
pub struct ReadinessChecker {
    runtime_initialized: AtomicBool,
    database_connected: AtomicBool,
    sandbox_pool_available: AtomicBool,
    brain_wasm_ready: AtomicBool,
    shutting_down: AtomicBool,
}

impl ReadinessChecker {
    pub fn check(&self) -> HealthStatus {
        let checks = vec![
            ("runtime", self.runtime_initialized.load(Ordering::Relaxed)),
            ("database", self.database_connected.load(Ordering::Relaxed)),
            ("sandbox_pool", self.sandbox_pool_available.load(Ordering::Relaxed)),
            ("brain_wasm", self.brain_wasm_ready.load(Ordering::Relaxed)),
        ];
        
        let all_healthy = checks.iter().all(|(_, status)| *status)
            && !self.shutting_down.load(Ordering::Relaxed);
        
        HealthStatus {
            status: if all_healthy { "ready" } else { "not_ready" },
            checks: checks.into_iter().map(|(name, status)| {
                (name.to_string(), if status { "ok" } else { "failed" })
            }).collect(),
        }
    }
}
```

**Kubernetes Configuration:**
```yaml
readinessProbe:
  httpGet:
    path: /health/ready
    port: 9090
  initialDelaySeconds: 10
  periodSeconds: 5
  timeoutSeconds: 5
  failureThreshold: 3
```

---

### 2.3 Startup Probe

**Purpose:** Allow slow startup without being killed by liveness probe.

```
GET /health/startup HTTP/1.1
Host: localhost:9090

HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "started",
  "phase": "complete",
  "components_initialized": ["runtime", "database", "sandbox_pool", "brain_wasm"],
  "startup_duration_ms": 18,
  "timestamp": "2026-03-02T10:30:00.123Z"
}
```

**During Startup:**
```
HTTP/1.1 503 Service Unavailable
Content-Type: application/json

{
  "status": "starting",
  "phase": "initializing",
  "components_initialized": ["runtime"],
  "components_pending": ["database", "sandbox_pool", "brain_wasm"],
  "timestamp": "2026-03-02T10:30:00.123Z"
}
```

**Kubernetes Configuration:**
```yaml
startupProbe:
  httpGet:
    path: /health/startup
    port: 9090
  initialDelaySeconds: 0
  periodSeconds: 1
  timeoutSeconds: 5
  failureThreshold: 30  # Allow 30s startup time
```

---

## 3. Prometheus Metrics Endpoint

### 3.1 Endpoint Specification

```
GET /metrics HTTP/1.1
Host: localhost:9090
Accept: text/plain

HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4; charset=utf-8

# HELP clawdius_build_info Build information
# TYPE clawdius_build_info gauge
clawdius_build_info{version="1.0.0",commit="abc123",rust_version="1.85.0"} 1

# HELP clawdius_hft_signal_latency_ns Signal-to-execution latency
# TYPE clawdius_hft_signal_latency_ns histogram
clawdius_hft_signal_latency_ns_bucket{le="100"} 0
clawdius_hft_signal_latency_ns_bucket{le="500"} 1523
clawdius_hft_signal_latency_ns_bucket{le="1000"} 8234
clawdius_hft_signal_latency_ns_bucket{le="+Inf"} 8500
clawdius_hft_signal_latency_ns_sum 5234567
clawdius_hft_signal_latency_ns_count 8500

# HELP clawdius_process_resident_memory_bytes Resident memory in bytes
# TYPE clawdius_process_resident_memory_bytes gauge
clawdius_process_resident_memory_bytes 42000000

# ... more metrics
```

### 3.2 Metrics Registry

```rust
use prometheus::{Registry, Counter, Histogram, Gauge, Opts, HistogramOpts};

pub struct MetricsRegistry {
    registry: Registry,
    
    // HFT metrics
    hft_signal_latency: Histogram,
    hft_ring_buffer_ops: Counter,
    hft_wallet_guard_checks: Counter,
    
    // Application metrics
    fsm_transitions: Counter,
    sandbox_spawns: Counter,
    llm_requests: Counter,
    
    // Build info
    build_info: Gauge,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        let hft_signal_latency = Histogram::with_opts(HistogramOpts::new(
            "clawdius_hft_signal_latency_ns",
            "Signal-to-execution latency"
        ).buckets(vec![100.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0])).unwrap();
        
        // ... register other metrics
        
        registry.register(Box::new(hft_signal_latency.clone())).unwrap();
        
        Self {
            registry,
            hft_signal_latency,
            // ...
        }
    }
    
    pub fn export(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
```

---

## 4. Detailed Health Checks

### 4.1 Runtime Health Check

```rust
pub fn check_runtime_health() -> HealthCheckResult {
    let monoio_ready = monoio::runtime::is_available();
    
    HealthCheckResult {
        name: "runtime",
        status: if monoio_ready { "ok" } else { "failed" },
        details: json!({
            "monoio_available": monoio_ready,
            "thread_count": std::thread::available_parallelism().map(|p| p.get()).unwrap_or(0),
        }),
    }
}
```

### 4.2 Database Health Check

```rust
pub async fn check_database_health(pool: &SqlitePool) -> HealthCheckResult {
    let sqlite_result = sqlx::query("SELECT 1").fetch_one(pool).await;
    let lancedb_result = lancedb::check_connection().await;
    
    HealthCheckResult {
        name: "database",
        status: if sqlite_result.is_ok() && lancedb_result.is_ok() { "ok" } else { "failed" },
        details: json!({
            "sqlite": if sqlite_result.is_ok() { "ok" } else { "failed" },
            "lancedb": if lancedb_result.is_ok() { "ok" } else { "failed" },
            "pool_size": pool.size(),
            "idle_connections": pool.num_idle(),
        }),
    }
}
```

### 4.3 Sandbox Pool Health Check

```rust
pub fn check_sandbox_pool_health(pool: &SandboxPool) -> HealthCheckResult {
    let available = pool.available_count();
    let total = pool.total_count();
    
    HealthCheckResult {
        name: "sandbox_pool",
        status: if available > 0 { "ok" } else { "degraded" },
        details: json!({
            "available": available,
            "total": total,
            "by_tier": {
                "tier1": pool.tier_count(SandboxTier::Tier1),
                "tier2": pool.tier_count(SandboxTier::Tier2),
                "tier3": pool.tier_count(SandboxTier::Tier3),
                "tier4": pool.tier_count(SandboxTier::Tier4),
            },
        }),
    }
}
```

### 4.4 Brain WASM Health Check

```rust
pub fn check_brain_wasm_health(brain: &BrainRpc) -> HealthCheckResult {
    let wasm_ready = brain.is_ready();
    let fuel_available = brain.available_fuel();
    
    HealthCheckResult {
        name: "brain_wasm",
        status: if wasm_ready { "ok" } else { "failed" },
        details: json!({
            "wasm_ready": wasm_ready,
            "fuel_available": fuel_available,
            "instance_count": brain.instance_count(),
        }),
    }
}
```

---

## 5. Graceful Shutdown

### 5.1 Shutdown Sequence

```
┌─────────────────────────────────────────────────────────────────────┐
│                    GRACEFUL SHUTDOWN SEQUENCE                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Signal Received (SIGTERM/SIGINT)                               │
│     │                                                               │
│     ▼                                                               │
│  2. Mark shutting_down = true                                      │
│     │  (Readiness probe returns 503)                               │
│     ▼                                                               │
│  3. Stop accepting new requests                                    │
│     │                                                               │
│     ▼                                                               │
│  4. Drain in-flight requests (timeout: 30s)                        │
│     │                                                               │
│     ▼                                                               │
│  5. Close sandbox pool                                             │
│     │                                                               │
│     ▼                                                               │
│  6. Close database connections                                     │
│     │                                                               │
│     ▼                                                               │
│  7. Flush metrics                                                  │
│     │                                                               │
│     ▼                                                               │
│  8. Exit with code 0                                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Shutdown Handler

```rust
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

pub async fn graceful_shutdown(
    readiness: Arc<ReadinessChecker>,
    sandbox_pool: SandboxPool,
    db_pool: SqlitePool,
) {
    // Mark as not ready
    readiness.shutting_down.store(true, Ordering::Relaxed);
    
    // Wait for in-flight requests (30s timeout)
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Close resources
    drop(sandbox_pool);
    drop(db_pool);
    
    info!("Graceful shutdown complete");
}
```

---

## 6. Kubernetes Deployment Configuration

### 6.1 Complete Probe Configuration

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: clawdius
spec:
  template:
    spec:
      containers:
        - name: clawdius
          image: clawdius:1.0.0
          ports:
            - containerPort: 9090
              name: http-metrics
          livenessProbe:
            httpGet:
              path: /health/live
              port: 9090
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 5
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /health/ready
              port: 9090
            initialDelaySeconds: 10
            periodSeconds: 5
            timeoutSeconds: 5
            failureThreshold: 3
          startupProbe:
            httpGet:
              path: /health/startup
              port: 9090
            initialDelaySeconds: 0
            periodSeconds: 1
            timeoutSeconds: 5
            failureThreshold: 30
          lifecycle:
            preStop:
              exec:
                command: ["/bin/sh", "-c", "sleep 10"]
```

### 6.2 Service Configuration

```yaml
apiVersion: v1
kind: Service
metadata:
  name: clawdius
spec:
  selector:
    app: clawdius
  ports:
    - name: http-metrics
      port: 9090
      targetPort: 9090
```

### 6.3 ServiceMonitor (Prometheus Operator)

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: clawdius
spec:
  selector:
    matchLabels:
      app: clawdius
  endpoints:
    - port: http-metrics
      path: /metrics
      interval: 15s
```

---

## 7. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| Liveness probe defined | ✅ | Section 2.1 |
| Readiness probe defined | ✅ | Section 2.2 |
| Startup probe defined | ✅ | Section 2.3 |
| Metrics endpoint defined | ✅ | Section 3 |
| Detailed health checks | ✅ | Section 4 |
| Graceful shutdown | ✅ | Section 5 |
| Kubernetes configs | ✅ | Section 6 |

---

**Document Status:** APPROVED  
**Next Review:** After production deployment  
**Sign-off:** Operations Team
