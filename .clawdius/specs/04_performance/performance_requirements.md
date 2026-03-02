# Performance Requirements Specification

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-REQ-001 |
| **Version** | 1.0.0 |
| **Phase** | 4 (Performance Engineering) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document establishes performance baselines, latency targets, throughput requirements, and Service Level Agreements (SLAs) for the Clawdius system. All targets are derived from:

- REQ-6.2: Boot latency < 20ms to interactive TUI
- REQ-5.2: Sub-millisecond market data processing (HFT)
- REQ-5.4: Signal dispatch within 100ms
- REQ-7.1: 60 FPS TUI rendering
- HC-001 through HC-004: HFT constraints from YP-HFT-BROKER-001

---

## 2. Latency Targets by Component

### 2.1 Boot and Initialization

| Component | Metric | Target | P99 Target | Critical | Source |
|-----------|--------|--------|------------|----------|--------|
| Full Boot | Time to interactive TUI | < 20ms | < 30ms | Yes | REQ-6.2 |
| Runtime Init | monoio startup | < 2ms | < 3ms | Yes | Derived |
| FSM Init | Phase 0 setup | < 1ms | < 2ms | No | Derived |
| Database Init | SQLite + LanceDB | < 5ms | < 8ms | No | Derived |
| Sandbox Pool Init | Pre-spawn 4 sandboxes | < 8ms | < 12ms | No | Derived |
| WASM Runtime Init | wasmtime startup | < 3ms | < 5ms | No | Derived |

### 2.2 Core Components

| Component | Metric | Target | P99 Target | Critical | Source |
|-----------|--------|--------|------------|----------|--------|
| FSM Phase Transition | State change | < 1µs | < 5µs | No | BP-NEXUS-FSM-001 |
| Sandbox Spawn | Tier 1-4 spawn | < 100ms | < 150ms | No | BP-SENTINEL-001 |
| WASM RPC | Round-trip call | < 1ms | < 2ms | No | BP-BRAIN-001 |
| Graph-RAG Parse | 10K files | < 5s | < 8s | Yes | REQ-5.1 |
| Graph-RAG Query | Semantic search | < 50ms | < 100ms | No | BP-GRAPH-RAG-001 |

### 2.3 HFT Mode (Critical Path)

| Component | Metric | Target | P99 Target | Critical | Source |
|-----------|--------|--------|------------|----------|--------|
| Market Data Ingestion | Per message | < 1µs | < 2µs | Yes | REQ-5.2 |
| Ring Buffer Write | Single entry | < 100ns | < 200ns | Yes | HC-003 |
| Ring Buffer Read | Single entry | < 100ns | < 200ns | Yes | HC-003 |
| Wallet Guard Risk Check | Full validation | < 100µs | < 200µs | Yes | HC-004 |
| Signal-to-Execution | End-to-end | < 1ms | < 2ms | Yes | HC-001 |
| Notification Dispatch | To Matrix | < 100ms | < 200ms | Yes | REQ-5.4 |

### 2.4 TUI Rendering

| Component | Metric | Target | P99 Target | Critical | Source |
|-----------|--------|--------|------------|----------|--------|
| Frame Render | Full redraw | < 16.67ms | < 20ms | Yes | REQ-7.1 |
| Input Latency | Key to screen | < 50ms | < 100ms | No | Derived |
| Diff Render | Partial update | < 5ms | < 8ms | No | Derived |

---

## 3. Throughput Targets

### 3.1 Data Processing

| Component | Metric | Target | Peak | Critical | Source |
|-----------|--------|--------|------|----------|--------|
| Market Data (HFT) | Messages/sec | 10M | 15M | Yes | HFT-015 |
| AST Parsing | Files/sec | 2000 | 5000 | No | Derived |
| Vector Embeddings | Embeddings/sec | 100 | 500 | No | Derived |
| Database Writes | Writes/sec | 10K | 50K | No | Derived |
| Database Reads | Reads/sec | 50K | 200K | No | Derived |

### 3.2 Concurrent Operations

| Component | Metric | Target | Peak | Critical | Source |
|-----------|--------|--------|------|----------|--------|
| Concurrent Sandboxes | Active count | 32 | 64 | No | resource_limits.md |
| Concurrent WASM | Instances | 4 | 8 | No | resource_limits.md |
| Concurrent DB Conns | SQLite | 8 | 16 | No | handle_management.md |
| Concurrent Network | TCP conns | 32 | 64 | No | resource_limits.md |
| Chat Requests | Req/sec | 10 | 50 | No | Derived |

---

## 4. Resource Utilization Limits

### 4.1 Memory Budgets

| Component | Standard Mode | HFT Mode | Enforced | Source |
|-----------|---------------|----------|----------|--------|
| Total Heap | 54 MB | 838 MB | Yes | memory_management.md |
| Ring Buffer | 0 MB | 512 MB | Yes | HC-003 |
| Arena Allocator | 0 MB | 256 MB | Yes | HFT-009 |
| WASM Linear | 20 MB | 20 MB | Yes | resource_limits.md |
| Database Cache | 32 MB | 32 MB | Yes | Derived |
| TUI State | 2 MB | 2 MB | No | Derived |

### 4.2 CPU Utilization

| Component | Standard Mode | HFT Mode | Measurement |
|-----------|---------------|----------|-------------|
| Idle CPU | < 5% | < 1% | Per core |
| Peak CPU | < 80% | < 50% | Per core |
| HFT Isolated Cores | N/A | < 30% | Dedicated cores 0-3 |

### 4.3 I/O Limits

| Resource | Limit | Burst | Enforced |
|----------|-------|-------|----------|
| File Descriptors | 64 | 128 | Yes |
| Disk I/O Read | 100 MB/s | 500 MB/s | No |
| Disk I/O Write | 50 MB/s | 200 MB/s | No |
| Network Ingress | 1 Gbps | 10 Gbps | No |
| Network Egress | 100 Mbps | 1 Gbps | No |

---

## 5. Service Level Agreements (SLAs)

### 5.1 HFT Mode SLA

| SLA ID | Metric | SLO | SLA | Penalty |
|--------|--------|-----|-----|---------|
| SLA-HFT-001 | Signal-to-execution latency | < 1ms | < 2ms | Alert |
| SLA-HFT-002 | Risk check latency | < 100µs | < 200µs | Alert |
| SLA-HFT-003 | GC pause duration | 0µs | 0µs | CRITICAL |
| SLA-HFT-004 | Market data loss | 0% | < 0.001% | Alert |
| SLA-HFT-005 | Ring buffer overflow | 0 | < 100/day | Warning |

### 5.2 Standard Mode SLA

| SLA ID | Metric | SLO | SLA | Penalty |
|--------|--------|-----|-----|---------|
| SLA-STD-001 | Boot time | < 20ms | < 30ms | Warning |
| SLA-STD-002 | TUI frame rate | 60 FPS | 30 FPS | Degraded |
| SLA-STD-003 | Chat response | < 5s | < 30s | Warning |
| SLA-STD-004 | Search latency | < 50ms | < 200ms | Warning |

### 5.3 Availability

| Metric | Target | Measurement |
|--------|--------|-------------|
| Uptime (Standard) | 99.9% | Monthly |
| Uptime (HFT) | 99.99% | Trading hours |
| Recovery Time (RTO) | < 5s | Automatic |
| Data Loss (RPO) | 0 | Transaction log |

---

## 6. Performance Budgets by Phase

### 6.1 Phase Budgets

| Phase | Total Budget | Components | Allocation |
|-------|--------------|------------|------------|
| Boot | 20ms | Runtime (2ms), FSM (1ms), DB (5ms), Sandbox (8ms), WASM (3ms), Buffer (1ms) | Fixed |
| HFT Critical Path | 1ms | Market Data (100µs), Strategy (200µs), Risk (100µs), Dispatch (600µs) | Dynamic |
| TUI Frame | 16.67ms | Input (1ms), Process (5ms), Render (10ms), Buffer (0.67ms) | Fixed |

### 6.2 Budget Enforcement

| Component | Enforcement | Mechanism |
|-----------|-------------|-----------|
| Boot phases | Warning | Timing logs |
| HFT critical path | Hard cutoff | Watchdog timer |
| TUI frame | Frame skip | VSync detection |

---

## 7. Measurement Methodology

### 7.1 Latency Measurement

| Measurement | Tool | Resolution | Location |
|-------------|------|------------|----------|
| Boot phases | `std::time::Instant` | 1ns | Main |
| HFT path | `quanta` crate | 1ns | Broker |
| TUI frames | VSync counter | 16.67ms | TUI |
| Network | PTP timestamps | 1µs | NIC |

### 7.2 Throughput Measurement

| Measurement | Tool | Interval | Aggregation |
|-------------|------|----------|-------------|
| Messages/sec | Counter | 1s | P50, P99, Max |
| Requests/sec | Histogram | 1s | P50, P99, Max |
| I/O throughput | Bytes | 1s | Average, Peak |

### 7.3 Resource Measurement

| Resource | Tool | Interval | Storage |
|----------|------|----------|---------|
| Memory | `jemalloc` stats | 10s | Prometheus |
| CPU | `/proc/stat` | 1s | Prometheus |
| I/O | `/proc/diskstats` | 1s | Prometheus |

---

## 8. Performance Regressions

### 8.1 Regression Thresholds

| Metric | Warning | Failure | Action |
|--------|---------|---------|--------|
| Boot time | +10% | +50% | Block merge |
| HFT latency | +5% | +20% | Block merge |
| Memory usage | +10% | +50% | Block merge |
| TUI frame time | +10% | +50% | Block merge |

### 8.2 Regression Detection

| Method | Frequency | Tool |
|--------|-----------|------|
| CI benchmark | Every PR | criterion |
| Nightly benchmark | Daily | criterion + perf |
| Load test | Weekly | k6 |
| HFT simulation | Daily | PCAP replay |

---

## 9. Traceability Matrix

| Requirement | Performance Target | Document Section |
|-------------|-------------------|------------------|
| REQ-5.1 | Graph-RAG Parse < 5s | 2.2 |
| REQ-5.2 | Market Data < 1µs | 2.3 |
| REQ-5.3 | Risk check < 100µs | 2.3 |
| REQ-5.4 | Notification < 100ms | 2.3 |
| REQ-6.2 | Boot < 20ms | 2.1 |
| REQ-7.1 | TUI 60 FPS | 2.4 |
| HC-001 | Signal-to-exec < 1ms | 2.3 |
| HC-002 | GC pause = 0µs | 5.1 |
| HC-003 | Ring buffer < 1µs | 2.3 |
| HC-004 | Risk check < 100µs | 2.3 |

---

## 10. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| All components have latency targets | Yes | Sections 2.1-2.4 |
| All components have throughput targets | Yes | Section 3 |
| Memory budgets defined | Yes | Section 4.1 |
| CPU limits defined | Yes | Section 4.2 |
| HFT SLAs defined | Yes | Section 5.1 |
| Standard SLAs defined | Yes | Section 5.2 |
| Measurement methodology documented | Yes | Section 7 |
| Regression thresholds defined | Yes | Section 8 |
| Requirements traced | Yes | Section 9 |

---

**Document Status:** APPROVED  
**Next Review:** After benchmark implementation  
**Sign-off:** Performance Engineering Team
