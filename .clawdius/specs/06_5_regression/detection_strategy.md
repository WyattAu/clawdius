# Regression Detection Strategy

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-DETECT-001 |
| **Version** | 1.0.0 |
| **Phase** | 5.5 (Performance Regression Baseline) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document defines the regression detection methodology for Clawdius, establishing statistical methods, significance thresholds, and CI/CD integration points to catch performance regressions before they reach production.

---

## 2. Statistical Detection Algorithm

### 2.1 Detection Method: Two-Sample T-Test

The regression detection uses a two-sample t-test to determine statistical significance:

```
t = (μ_current - μ_baseline) / sqrt((σ²_current/n_current) + (σ²_baseline/n_baseline))

Where:
- μ = sample mean
- σ = sample standard deviation  
- n = sample size
```

### 2.2 Detection Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Sample Size | 1000 | Statistical power > 0.95 |
| Confidence Level | 99% | Critical HFT requirements |
| Significance Level (α) | 0.01 | Low false positive rate |
| Warm-up Iterations | 100 | Cache priming |
| Measurement Time | 10s | Stable measurements |

### 2.3 Regression Classification

```rust
enum RegressionStatus {
    NoRegression,
    Warning,      // > threshold_warning but < threshold_failure
    Failure,      // > threshold_failure
    Improvement,  // Significant improvement detected
}

fn classify_regression(
    current_mean: f64,
    baseline_mean: f64,
    threshold_warning: f64,
    threshold_failure: f64,
    p_value: f64,
) -> RegressionStatus {
    let change_pct = (current_mean - baseline_mean) / baseline_mean * 100.0;
    
    if p_value > 0.01 {
        return RegressionStatus::NoRegression;
    }
    
    if change_pct > threshold_failure {
        RegressionStatus::Failure
    } else if change_pct > threshold_warning {
        RegressionStatus::Warning
    } else if change_pct < -threshold_warning {
        RegressionStatus::Improvement
    } else {
        RegressionStatus::NoRegression
    }
}
```

---

## 3. Statistical Significance Thresholds

### 3.1 Component-Specific Thresholds

| Component Category | Warning | Failure | Block Merge |
|-------------------|---------|---------|-------------|
| HFT Critical Path | +5% | +20% | Yes |
| Boot Time | +10% | +50% | Yes |
| TUI Rendering | +10% | +50% | No |
| Sandbox Operations | +10% | +50% | No |
| WASM RPC | +10% | +50% | No |
| Graph-RAG | +10% | +50% | No |
| Throughput | -5% | -20% | Yes |

### 3.2 P-Value Requirements

| Test Type | P-Value Threshold | Action |
|-----------|-------------------|--------|
| PR Benchmark | < 0.01 | Flag regression |
| Nightly Benchmark | < 0.05 | Create issue |
| Release Benchmark | < 0.01 | Block release |

### 3.3 Effect Size Requirements

| Metric | Cohen's d | Interpretation |
|--------|-----------|----------------|
| Small effect | 0.2 | Ignore if within threshold |
| Medium effect | 0.5 | Warning |
| Large effect | 0.8 | Failure |

---

## 4. CI/CD Integration Points

### 4.1 Pull Request Checks

```yaml
# .github/workflows/performance.yml
name: Performance Check

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Download baseline
        uses: actions/download-artifact@v4
        with:
          name: baseline-metrics
          
      - name: Run benchmarks
        run: cargo bench -- --save-baseline pr-${{ github.sha }}
        
      - name: Compare against baseline
        run: |
          cargo critcmp baseline pr-${{ github.sha }} \
            --threshold 5% \
            --output json > comparison.json
          
      - name: Check for regressions
        run: |
          python scripts/check_regression.py comparison.json
          
      - name: Comment on PR
        uses: actions/github-script@v7
        with:
          script: |
            const results = require('./comparison.json');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: formatBenchmarkResults(results)
            });
```

### 4.2 Merge to Main

```yaml
# On merge to main - update baseline
on:
  push:
    branches: [main]

jobs:
  update-baseline:
    runs-on: self-hosted
    steps:
      - name: Run full benchmark suite
        run: cargo bench -- --save-baseline main
        
      - name: Upload baseline
        uses: actions/upload-artifact@v4
        with:
          name: baseline-metrics
          path: target/criterion/
          retention-days: 90
          
      - name: Update baseline metrics TOML
        run: |
          python scripts/update_baseline_toml.py \
            --input target/criterion/ \
            --output .clawdius/specs/06_5_regression/baseline_metrics.toml
```

### 4.3 Nightly Benchmarks

```yaml
# Nightly comprehensive benchmark
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM UTC

jobs:
  nightly:
    runs-on: self-hosted-hft
    steps:
      - name: Configure isolated cores
        run: sudo cset proc --set /isolated --cpu 0-3
        
      - name: Run HFT benchmarks on isolated cores
        run: |
          sudo cset proc --exec /isolated -- \
            cargo bench -- hft --sample-size 10000
            
      - name: Run memory benchmarks
        run: cargo bench -- memory
        
      - name: Check for drift
        run: |
          python scripts/check_drift.py \
            --baseline .clawdius/specs/06_5_regression/baseline_metrics.toml \
            --current target/criterion/
            
      - name: Create issue if drift detected
        if: failure()
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.issues.create({
              owner: context.repo.owner,
              repo: context.repo.repo,
              title: 'Performance drift detected',
              body: 'See workflow run for details',
              labels: ['performance', 'investigation']
            });
```

### 4.4 Release Gate

```yaml
# Release blocking check
on:
  release:
    types: [created]

jobs:
  release-gate:
    runs-on: self-hosted
    steps:
      - name: Run all benchmarks with strict thresholds
        run: cargo bench -- --sample-size 10000
        
      - name: Strict regression check (2% threshold)
        run: |
          python scripts/check_regression.py \
            --threshold 2 \
            --fail-on-any-regression
            
      - name: Generate release performance report
        run: |
          python scripts/generate_report.py \
            --format html \
            --output release-perf-report.html
```

---

## 5. Detection Workflow

### 5.1 Automated Detection Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    PR Opened                                 │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Run Micro-Benchmarks                            │
│  (FSM, Ring Buffer, Wallet Guard, Sandbox, WASM RPC)        │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Compare Against Baseline                        │
│  (Two-sample t-test, p < 0.01)                              │
└────────────────────────┬────────────────────────────────────┘
                         │
          ┌──────────────┼──────────────┐
          │              │              │
          ▼              ▼              ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ < 5%     │   │ 5-20%    │   │ > 20%    │
    │ PASS     │   │ WARNING  │   │ FAIL     │
    └────┬─────┘   └────┬─────┘   └────┬─────┘
         │              │              │
         ▼              ▼              ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Allow    │   │ Comment  │   │ Block    │
    │ Merge    │   │ on PR    │   │ Merge    │
    └──────────┘   └──────────┘   └──────────┘
```

### 5.2 Manual Investigation Flow

```
┌─────────────────────────────────────────────────────────────┐
│              Regression Detected                             │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Collect Diagnostics                             │
│  - Flamegraph generation                                    │
│  - Memory profile                                           │
│  - Cache miss analysis                                      │
│  - CPU cycle count                                          │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Identify Root Cause                             │
│  - Code change diff analysis                                │
│  - Compiler optimization differences                        │
│  - Environment changes                                      │
│  - External dependency changes                              │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Resolution                                     │
│  - Revert offending change                                  │
│  - Optimize implementation                                  │
│  - Update baseline (if intentional)                         │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Baseline Management

### 6.1 Baseline Versioning

| Baseline Type | Versioning | Storage |
|---------------|------------|---------|
| Main | `main-YYYYMMDD-HASH` | Git LFS |
| Release | `vX.Y.Z` | Git tag |
| PR | `pr-XXXX` | CI artifacts (30 days) |
| Nightly | `nightly-YYYYMMDD` | S3 (90 days) |

### 6.2 Baseline Update Policy

| Trigger | Action | Approval |
|---------|--------|----------|
| Merge to main | Auto-update | Automatic |
| Performance improvement | Update with comment | Automatic |
| Performance degradation | Block + investigate | Manual |
| Release | Tag and archive | Automatic |

### 6.3 Baseline Validation

```rust
fn validate_baseline(new: &Baseline, old: &Baseline) -> Result<(), ValidationError> {
    for (benchmark, new_metrics) in &new.metrics {
        let old_metrics = old.metrics.get(benchmark)
            .ok_or(ValidationError::MissingBenchmark)?;
        
        let change = (new_metrics.mean - old_metrics.mean) / old_metrics.mean;
        
        if change.abs() > 0.10 {
            warn!("Large baseline change for {}: {:.2}%", benchmark, change * 100.0);
        }
        
        if new_metrics.sample_size < 100 {
            return Err(ValidationError::InsufficientSamples);
        }
        
        if new_metrics.std_dev / new_metrics.mean > 0.5 {
            warn!("High variance for {}: {:.2}%", benchmark, 
                  new_metrics.std_dev / new_metrics.mean * 100.0);
        }
    }
    
    Ok(())
}
```

---

## 7. Alerting Integration

### 7.1 Alert Triggers

| Condition | Severity | Channel |
|-----------|----------|---------|
| HFT regression > 5% | Critical | PagerDuty, Slack |
| HFT regression > 20% | Critical | PagerDuty, Slack, Email |
| Standard regression > 10% | Warning | Slack |
| Standard regression > 50% | Error | Slack, Email |
| Drift detected (nightly) | Info | Slack |
| Improvement detected | Info | Slack |

### 7.2 Alert Enrichment

```json
{
  "alert_type": "performance_regression",
  "severity": "critical",
  "benchmark": "ring_buffer.push",
  "baseline_mean_ns": 21,
  "current_mean_ns": 28,
  "change_pct": 33.3,
  "p_value": 0.001,
  "commit": "abc123",
  "pr_number": 42,
  "author": "developer",
  "diagnostics": {
    "flamegraph_url": "https://s3.example.com/flamegraphs/abc123.svg",
    "comparison_url": "https://ci.example.com/benchmarks/compare/abc123"
  }
}
```

---

## 8. Exemptions and Overrides

### 8.1 Exemption Process

1. Create issue with `performance-exemption` label
2. Document reason for expected regression
3. Specify expected magnitude and duration
4. Get approval from performance lead
5. Add exemption to `exemptions.toml`

### 8.2 Exemption Format

```toml
# exemptions.toml
[exemptions.EXM-001]
benchmark = "graph_rag.semantic_search"
reason = "New security validation step required"
expected_change_pct = 15.0
expires = "2026-04-01"
approved_by = "performance-lead"
issue = "https://github.com/example/clawdius/issues/123"
```

### 8.3 Override Mechanism

```bash
# CI override with documented reason
gh pr comment 42 --body "Performance override: Adding security validation (EXM-001)"
gh pr merge 42 --admin
```

---

## 9. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| Statistical method defined | Yes | Two-sample t-test |
| Significance thresholds defined | Yes | Section 3 |
| CI/CD integration documented | Yes | Section 4 |
| Baseline management defined | Yes | Section 6 |
| Alerting integration defined | Yes | Section 7 |
| Exemption process defined | Yes | Section 8 |

---

**Document Status:** APPROVED  
**Next Review:** After first regression detection  
**Sign-off:** Performance Engineering Team
