# Performance Alerting Rules

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-ALERT-001 |
| **Version** | 1.0.0 |
| **Phase** | 5.5 (Performance Regression Baseline) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document defines alert thresholds, severity levels, notification channels, and escalation procedures for performance monitoring in Clawdius.

---

## 2. Alert Severity Levels

### 2.1 Severity Definitions

| Level | Name | Description | Response Time |
|-------|------|-------------|---------------|
| P0 | Critical | Production impact, HFT SLA violation | Immediate (< 5 min) |
| P1 | High | Significant regression, merge blocked | < 30 min |
| P2 | Medium | Warning threshold exceeded | < 4 hours |
| P3 | Low | Informational, trend detection | Next business day |
| P4 | Info | Improvement detected, metrics update | No action required |

### 2.2 Severity Assignment Matrix

| Regression | HFT Component | Boot | Standard | Throughput |
|------------|---------------|------|----------|------------|
| > 20% | P0 | P1 | P2 | P0 |
| 10-20% | P1 | P2 | P3 | P1 |
| 5-10% | P2 | P3 | P3 | P2 |
| < 5% | P3 | P4 | P4 | P3 |

---

## 3. Alert Thresholds by Component

### 3.1 HFT Critical Path (P0)

| Benchmark | Warning | Critical | Blocker |
|-----------|---------|----------|---------|
| `ring_buffer.push` | +5% | +10% | +20% |
| `ring_buffer.pop` | +5% | +10% | +20% |
| `ring_buffer.roundtrip` | +5% | +10% | +20% |
| `wallet_guard.full_validation` | +5% | +10% | +20% |
| `hft_pipeline.market_data_ingestion` | +5% | +10% | +20% |
| `hft_pipeline.signal_generation` | +5% | +10% | +20% |
| `hft_pipeline.full_pipeline` | +5% | +10% | +20% |
| `throughput.market_data` | -5% | -10% | -20% |

### 3.2 Boot & Initialization (P1)

| Benchmark | Warning | Critical | Blocker |
|-----------|---------|----------|---------|
| `boot` | +10% | +25% | +50% |
| `fsm.phase_transition` | +10% | +25% | +50% |
| `fsm.full_cycle` | +10% | +25% | +50% |

### 3.3 Standard Components (P2)

| Benchmark | Warning | Critical | Blocker |
|-----------|---------|----------|---------|
| `sandbox.tier_*` | +10% | +25% | +50% |
| `wasm_rpc.*` | +10% | +25% | +50% |
| `graph_rag.*` | +10% | +25% | +50% |
| `tui.frame_render` | +10% | +25% | +50% |
| `throughput.file_parsing` | -10% | -25% | -50% |

---

## 4. Notification Channels

### 4.1 Channel Configuration

| Channel | Type | Use Case | Response |
|---------|------|----------|----------|
| PagerDuty | Pager | P0 alerts | 24/7 on-call |
| Slack #perf-alerts | Chat | All alerts | Team monitoring |
| Slack #hft-alerts | Chat | HFT-specific | HFT team |
| Email perf-team@ | Email | Summary reports | Daily digest |
| GitHub PR Comment | CI | PR-specific | Developer |

### 4.2 Routing Rules

```yaml
routing:
  - match:
      severity: P0
      component: hft
    receivers:
      - pagerduty-hft-oncall
      - slack-hft-alerts
      - slack-perf-alerts
      
  - match:
      severity: P0
    receivers:
      - pagerduty-oncall
      - slack-perf-alerts
      
  - match:
      severity: P1
    receivers:
      - slack-perf-alerts
      - email-perf-team
      
  - match:
      severity: P2
    receivers:
      - slack-perf-alerts
      
  - match:
      severity: P3
    receivers:
      - slack-perf-alerts
      
  - match:
      severity: P4
    receivers:
      - slack-perf-alerts
```

### 4.3 Notification Templates

#### P0 Critical Alert

```
🚨 PERFORMANCE CRITICAL 🚨

Component: {{component}}
Benchmark: {{benchmark}}
Severity: P0

Baseline: {{baseline_mean}} {{unit}}
Current: {{current_mean}} {{unit}}
Change: +{{change_pct}}%

SLA Impact: {{sla_impact}}
Commit: {{commit_hash}}
PR: #{{pr_number}}

⚠️ IMMEDIATE ACTION REQUIRED

Runbook: https://wiki.example.com/perf/runbook/{{component}}
Diagnostics: {{diagnostics_url}}
```

#### P2 Warning Alert

```
⚠️ Performance Warning

Component: {{component}}
Benchmark: {{benchmark}}

Baseline: {{baseline_mean}} {{unit}}
Current: {{current_mean}} {{unit}}
Change: +{{change_pct}}%

Commit: {{commit_hash}}
PR: #{{pr_number}}

Review within 4 hours.
```

---

## 5. Escalation Procedures

### 5.1 Escalation Timeline

| Time | Action | Owner |
|------|--------|-------|
| T+0 | Alert triggered | Automated |
| T+5m | P0: First page | On-call |
| T+15m | P0: Second page (if unacked) | On-call backup |
| T+30m | P0: Escalate to manager | Engineering Manager |
| T+1h | P0: Escalate to director | Director |
| T+2h | P1: First notification | Team lead |
| T+4h | P1: Escalate if no response | Engineering Manager |

### 5.2 Acknowledgment Process

```bash
# Acknowledge alert
pd-ack --incident $INCIDENT_ID --user $USER

# Add note
pd-note --incident $INCIDENT_ID --message "Investigating root cause"

# Resolve
pd-resolve --incident $INCIDENT_ID --message "Reverted commit abc123"
```

### 5.3 Escalation Contacts

| Role | Primary | Backup |
|------|---------|--------|
| On-call | perf-oncall@example.com | perf-backup@example.com |
| HFT On-call | hft-oncall@example.com | hft-backup@example.com |
| Team Lead | perf-lead@example.com | eng-lead@example.com |
| Manager | eng-manager@example.com | director@example.com |

---

## 6. Alert Aggregation and Deduplication

### 6.1 Deduplication Rules

| Rule | Window | Condition |
|------|--------|-----------|
| Same benchmark | 1 hour | Same commit |
| Same component | 15 min | Multiple benchmarks |
| Same PR | 1 hour | Any benchmark |

### 6.2 Aggregation

```yaml
aggregation:
  - name: component_aggregation
    group_by: [component, commit]
    window: 15m
    max_alerts: 1
    
  - name: pr_aggregation
    group_by: [pr_number]
    window: 1h
    max_alerts: 3
```

### 6.3 Rate Limiting

| Alert Type | Max per Hour | Max per Day |
|------------|--------------|-------------|
| P0 | 5 | 20 |
| P1 | 10 | 50 |
| P2 | 20 | 100 |
| P3 | 50 | 200 |

---

## 7. Runbooks

### 7.1 HFT Latency Regression

```markdown
## HFT Latency Regression Runbook

### Symptoms
- Ring buffer latency > 100ns
- Wallet Guard latency > 100µs
- Pipeline latency > 1ms

### Immediate Actions
1. Check recent commits: `git log --oneline -20`
2. Identify affected benchmark
3. Generate flamegraph: `cargo bench -- --profile-time 30`
4. Compare with baseline

### Common Causes
1. Cache miss increase
2. Memory allocation in hot path
3. Lock contention
4. Compiler regression

### Resolution
1. Revert offending commit
2. Create issue for investigation
3. Update baseline if intentional
```

### 7.2 Boot Time Regression

```markdown
## Boot Time Regression Runbook

### Symptoms
- Boot time > 20ms
- Component initialization slow

### Immediate Actions
1. Check initialization logs
2. Profile boot sequence: `perf record -g ./target/release/clawdius`
3. Identify slow component

### Common Causes
1. Database schema migration
2. Heavy configuration load
3. Network timeout during init

### Resolution
1. Defer non-critical initialization
2. Implement lazy loading
3. Cache configuration
```

### 7.3 Memory Regression

```markdown
## Memory Regression Runbook

### Symptoms
- Memory usage > target
- Memory growth over time

### Immediate Actions
1. Check memory profile: `heaptrack ./target/release/clawdius`
2. Identify allocation hotspots
3. Check for leaks: `valgrind --leak-check=full`

### Common Causes
1. Unbounded collection growth
2. Missing cleanup in drop
3. Arena overallocation

### Resolution
1. Add bounds to collections
2. Implement proper cleanup
3. Tune arena sizes
```

---

## 8. Monitoring Dashboards

### 8.1 Real-Time Dashboard

| Panel | Metric | Update Interval |
|-------|--------|-----------------|
| HFT Latency | Ring buffer P99 | 1s |
| HFT Latency | Wallet Guard P99 | 1s |
| Pipeline | End-to-end P99 | 1s |
| Throughput | Messages/sec | 1s |
| Memory | Total heap | 10s |
| Alerts | Active count | Real-time |

### 8.2 Trend Dashboard

| Panel | Metric | Time Range |
|-------|--------|------------|
| Latency Trend | P50, P95, P99 | 30 days |
| Throughput Trend | Mean | 30 days |
| Regression Count | Weekly | 90 days |
| SLA Compliance | Daily | 30 days |

### 8.3 CI Dashboard

| Panel | Metric | Scope |
|-------|--------|-------|
| PR Benchmarks | Pass/Fail | Last 100 PRs |
| Baseline Drift | % change | Last 30 days |
| Build Times | Duration | Last 30 days |

---

## 9. Maintenance Windows

### 9.1 Scheduled Maintenance

| Window | Duration | Impact |
|--------|----------|--------|
| Sunday 02:00-04:00 UTC | 2h | No alerts generated |
| Post-release | 1h | Reduced thresholds |

### 9.2 Maintenance Mode

```bash
# Enable maintenance mode
perf-maintenance enable --duration 2h --reason "Database migration"

# Alerts are suppressed during maintenance
# Post-maintenance baseline comparison is required
```

---

## 10. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| Severity levels defined | Yes | Section 2 |
| Thresholds by component | Yes | Section 3 |
| Notification channels | Yes | Section 4 |
| Escalation procedures | Yes | Section 5 |
| Deduplication rules | Yes | Section 6 |
| Runbooks provided | Yes | Section 7 |
| Dashboards defined | Yes | Section 8 |

---

**Document Status:** APPROVED  
**Next Review:** After first P0 alert  
**Sign-off:** Performance Engineering Team
