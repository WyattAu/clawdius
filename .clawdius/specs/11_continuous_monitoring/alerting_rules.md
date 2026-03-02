# Alerting Rules

**Document ID:** AR-CLAWDIUS-011  
**Version:** 1.0.0  
**Phase:** 11 (Continuous Monitoring)  
**Date:** 2026-03-02  
**Status:** APPROVED

---

## 1. Executive Summary

This document defines alerting rules, thresholds, escalation procedures, and runbooks for Clawdius production monitoring.

### 1.1 Alert Severity Levels

| Level | Name | Response Time | Notification |
|-------|------|---------------|--------------|
| P0 | Critical | Immediate (< 5 min) | PagerDuty + Slack + SMS |
| P1 | High | < 15 min | PagerDuty + Slack |
| P2 | Medium | < 1 hour | Slack + Email |
| P3 | Low | < 4 hours | Email |
| P4 | Info | Next business day | Email (digest) |

---

## 2. HFT Alerts (P0 Critical)

### 2.1 Signal Latency Breach

```yaml
alert: HftSignalLatencyBreach
expr: |
  histogram_quantile(0.99, 
    rate(clawdius_hft_signal_latency_ns_bucket[5m])
  ) > 1000000  # 1ms in nanoseconds
for: 1m
labels:
  severity: critical
  category: hft
annotations:
  summary: "HFT signal latency P99 exceeded 1ms"
  description: "P99 latency is {{ $value | humanizeDuration }}"
  runbook: "https://docs.clawdius.io/runbooks/hft-latency"
```

**Thresholds:**
| Metric | Warning | Critical | Emergency |
|--------|---------|----------|-----------|
| P99 latency | > 800µs | > 1ms | > 2ms |
| P99.9 latency | > 1ms | > 2ms | > 5ms |

**Runbook Actions:**
1. Check CPU isolation (cores 0-3 dedicated)
2. Verify no GC activity (`clawdius_hft_gc_pause_ns` should be 0)
3. Check ring buffer depth (overflow causes backpressure)
4. Review market data rate for unusual spikes
5. Check for OS jitter (context switches, interrupts)

### 2.2 Wallet Guard Latency Breach

```yaml
alert: HftWalletGuardLatencyBreach
expr: |
  histogram_quantile(0.99,
    rate(clawdius_hft_wallet_guard_latency_ns_bucket[5m])
  ) > 100000  # 100µs in nanoseconds
for: 1m
labels:
  severity: critical
  category: hft
annotations:
  summary: "Wallet Guard risk check latency exceeded 100µs"
  description: "P99 latency is {{ $value | humanizeDuration }}"
```

**Thresholds:**
| Metric | Warning | Critical |
|--------|---------|----------|
| P99 latency | > 80µs | > 100µs |
| Max latency | > 150µs | > 200µs |

### 2.3 Ring Buffer Overflow

```yaml
alert: HftRingBufferOverflow
expr: |
  rate(clawdius_hft_ring_buffer_overflow_total[5m]) > 0
for: 1m
labels:
  severity: critical
  category: hft
annotations:
  summary: "Ring buffer overflow detected"
  description: "{{ $value }} overflows/second"
```

**Runbook Actions:**
1. Increase ring buffer capacity (requires restart)
2. Check consumer lag
3. Review market data rate
4. Consider horizontal scaling

### 2.4 GC Pause Detected

```yaml
alert: HftGcPauseDetected
expr: clawdius_hft_gc_pause_ns > 0
labels:
  severity: critical
  category: hft
annotations:
  summary: "GC pause detected in HFT mode"
  description: "GC pause of {{ $value }}ns detected"
```

**Note:** This alert should NEVER fire. GC is disabled in HFT mode.

### 2.5 Market Data Loss

```yaml
alert: HftMarketDataLoss
expr: |
  rate(clawdius_hft_market_data_dropped_total[5m]) > 0
for: 30s
labels:
  severity: critical
  category: hft
annotations:
  summary: "Market data messages dropped"
  description: "{{ $value }} messages/second dropped"
```

---

## 3. Application Alerts (P1)

### 3.1 FSM Quality Gate Failure

```yaml
alert: FsmQualityGateFailure
expr: |
  rate(clawdius_fsm_quality_gates_failed_total[5m]) > 0
for: 5m
labels:
  severity: high
  category: application
annotations:
  summary: "FSM quality gate failures detected"
  description: "Gate {{ $labels.gate_id }} failing"
```

### 3.2 Sandbox Spawn Failure Rate

```yaml
alert: SandboxSpawnFailureRate
expr: |
  rate(clawdius_sentinel_sandbox_spawn_total{result="error"}[5m])
  /
  rate(clawdius_sentinel_sandbox_spawn_total[5m])
  > 0.05  # 5% error rate
for: 5m
labels:
  severity: high
  category: application
annotations:
  summary: "High sandbox spawn failure rate"
  description: "{{ $value | humanizePercentage }} spawn failures"
```

### 3.3 LLM Provider Errors

```yaml
alert: LlmProviderErrors
expr: |
  rate(clawdius_brain_llm_requests_total{result="error"}[5m]) > 0.1
for: 5m
labels:
  severity: high
  category: application
annotations:
  summary: "LLM provider errors detected"
  description: "Provider {{ $labels.provider }} error rate: {{ $value }}/s"
```

### 3.4 Graph-RAG Query Latency

```yaml
alert: GraphRagQueryLatency
expr: |
  histogram_quantile(0.99,
    rate(clawdius_graph_rag_query_latency_seconds_bucket[5m])
  ) > 0.2  # 200ms
for: 5m
labels:
  severity: high
  category: application
annotations:
  summary: "Graph-RAG query latency high"
  description: "P99 latency: {{ $value | humanizeDuration }}"
```

### 3.5 Database Connection Pool Exhaustion

```yaml
alert: DatabasePoolExhaustion
expr: |
  clawdius_db_connections_active / clawdius_db_connections_max > 0.9
for: 5m
labels:
  severity: high
  category: application
annotations:
  summary: "Database connection pool near exhaustion"
  description: "{{ $labels.db }} pool at {{ $value | humanizePercentage }}"
```

---

## 4. Infrastructure Alerts (P1)

### 4.1 Memory Usage High

```yaml
alert: MemoryUsageHigh
expr: |
  clawdius_process_resident_memory_bytes / (54 * 1024 * 1024) > 0.8
for: 5m
labels:
  severity: high
  category: infrastructure
annotations:
  summary: "Memory usage above 80%"
  description: "RSS: {{ $value | humanize1024 }}B / 54MB limit"
```

**HFT Mode Variant:**
```yaml
alert: MemoryUsageHighHft
expr: |
  clawdius_process_resident_memory_bytes / (838 * 1024 * 1024) > 0.9
for: 5m
labels:
  severity: high
  category: infrastructure
annotations:
  summary: "HFT memory usage above 90%"
```

### 4.2 Memory Leak Detection

```yaml
alert: MemoryLeakSuspected
expr: |
  deriv(clawdius_process_resident_memory_bytes[1h]) > 1048576  # > 1MB/hour growth
for: 2h
labels:
  severity: high
  category: infrastructure
annotations:
  summary: "Possible memory leak detected"
  description: "Memory growing at {{ $value | humanize1024 }}B/hour"
```

### 4.3 File Descriptor Exhaustion

```yaml
alert: FileDescriptorExhaustion
expr: |
  clawdius_process_open_fds / 64 > 0.8
for: 5m
labels:
  severity: high
  category: infrastructure
annotations:
  summary: "File descriptor usage above 80%"
  description: "{{ $value }} / 64 FDs in use"
```

### 4.4 CPU Usage High

```yaml
alert: CpuUsageHigh
expr: |
  rate(clawdius_process_cpu_seconds_total[5m]) > 0.8
for: 10m
labels:
  severity: medium
  category: infrastructure
annotations:
  summary: "CPU usage above 80%"
  description: "{{ $value | humanizePercentage }} CPU usage"
```

---

## 5. Security Alerts (P1)

### 5.1 Sandbox Escape Attempt

```yaml
alert: SandboxEscapeAttempt
expr: |
  rate(clawdius_security_sandbox_violations_total[5m]) > 0
labels:
  severity: critical
  category: security
annotations:
  summary: "Sandbox escape attempt detected"
  description: "Violation type: {{ $labels.violation_type }}"
```

### 5.2 Capability Escalation Attempt

```yaml
alert: CapabilityEscalationAttempt
expr: |
  rate(clawdius_security_capability_denied_total{reason="escalation"}[5m]) > 0
labels:
  severity: critical
  category: security
annotations:
  summary: "Capability escalation attempt detected"
  description: "Permission: {{ $labels.permission }}"
```

### 5.3 Excessive Capability Denials

```yaml
alert: ExcessiveCapabilityDenials
expr: |
  rate(clawdius_security_capability_denied_total[5m]) > 1
for: 5m
labels:
  severity: high
  category: security
annotations:
  summary: "High rate of capability denials"
  description: "{{ $value }} denials/second"
```

### 5.4 Input Validation Attack Pattern

```yaml
alert: InputValidationAttackPattern
expr: |
  rate(clawdius_security_input_validation_errors_total[5m]) > 10
for: 2m
labels:
  severity: high
  category: security
annotations:
  summary: "Possible attack pattern in input validation"
  description: "{{ $value }} validation errors/second"
```

---

## 6. Availability Alerts (P0)

### 6.1 Process Down

```yaml
alert: ProcessDown
expr: up{job="clawdius"} == 0
for: 1m
labels:
  severity: critical
  category: availability
annotations:
  summary: "Clawdius process is down"
  description: "Instance {{ $labels.instance }} unreachable"
```

### 6.2 Health Check Failure

```yaml
alert: HealthCheckFailure
expr: |
  probe_success{job="clawdius-health"} == 0
for: 2m
labels:
  severity: critical
  category: availability
annotations:
  summary: "Health check failing"
  description: "{{ $labels.instance }} health check failed"
```

### 6.3 Startup Failure

```yaml
alert: StartupFailure
expr: |
  time() - clawdius_process_start_time_seconds > 300
  and clawdius_health_ready == 0
labels:
  severity: critical
  category: availability
annotations:
  summary: "Process started but not ready after 5 minutes"
```

---

## 7. Alert Routing

### 7.1 Alertmanager Configuration

```yaml
route:
  receiver: 'default'
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
  routes:
    - match:
        severity: critical
        category: hft
      receiver: 'hft-oncall'
      group_wait: 10s
      repeat_interval: 5m
    - match:
        severity: critical
      receiver: 'critical-oncall'
      group_wait: 30s
      repeat_interval: 15m
    - match:
        severity: high
      receiver: 'high-priority'
      group_wait: 2m
      repeat_interval: 1h
    - match:
        category: security
      receiver: 'security-team'

receivers:
  - name: 'hft-oncall'
    pagerduty_configs:
      - service_key: '<hft-pagerduty-key>'
    slack_configs:
      - channel: '#hft-alerts'
  - name: 'critical-oncall'
    pagerduty_configs:
      - service_key: '<critical-pagerduty-key>'
    slack_configs:
      - channel: '#critical-alerts'
  - name: 'high-priority'
    slack_configs:
      - channel: '#ops-alerts'
  - name: 'security-team'
    slack_configs:
      - channel: '#security-alerts'
  - name: 'default'
    slack_configs:
      - channel: '#general-alerts'
```

### 7.2 Escalation Matrix

| Time Since Alert | P0 Critical | P1 High | P2 Medium |
|------------------|-------------|---------|-----------|
| 0-5 min | On-call | - | - |
| 5-15 min | On-call + Manager | On-call | - |
| 15-30 min | + Director | On-call + Manager | On-call |
| 30-60 min | + VP | + Director | On-call + Manager |
| > 1 hour | + Executive | + VP | + Director |

---

## 8. Runbook Index

| Alert | Runbook URL |
|-------|-------------|
| HftSignalLatencyBreach | /runbooks/hft-latency |
| HftWalletGuardLatencyBreach | /runbooks/wallet-guard |
| HftRingBufferOverflow | /runbooks/ring-buffer |
| HftGcPauseDetected | /runbooks/gc-pause |
| SandboxEscapeAttempt | /runbooks/sandbox-escape |
| MemoryLeakSuspected | /runbooks/memory-leak |
| ProcessDown | /runbooks/process-down |

---

## 9. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| All P0 alerts defined | ✅ | Sections 2, 6 |
| All P1 alerts defined | ✅ | Sections 3-5 |
| Thresholds aligned with SLAs | ✅ | Cross-referenced with performance_requirements.md |
| Runbooks linked | ✅ | Section 8 |
| Escalation procedures defined | ✅ | Section 7.2 |
| Notification channels configured | ✅ | Section 7.1 |

---

**Document Status:** APPROVED  
**Next Review:** After production deployment  
**Sign-off:** Operations Team
