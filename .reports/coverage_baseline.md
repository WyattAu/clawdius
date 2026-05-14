# Branch Coverage Baseline

**Date:** 2026-05-14
**Tool:** `cargo-llvm-cov` v0.8.7
**Command:** `cargo llvm-cov --workspace --lib --ignore-filename-regex="tests/" --output-dir=target/coverage`
**Profile:** `--lib` (unit tests only, integration tests excluded)

## Tool Availability

`cargo-llvm-cov` was **not** pre-installed. It was installed via `cargo install cargo-llvm-cov` (v0.8.7) and coverage ran successfully.

## Per-Crate Coverage Summary

| Crate | Regions | Region Cover | Functions | Func Cover | Lines | Line Cover |
|---|---|---|---|---|---|---|
| clawdius-code | 766 | **100.00%** | 57 | **100.00%** | 356 | **100.00%** |
| clawdius-mcp | 627 | **100.00%** | 47 | **100.00%** | 292 | **100.00%** |
| clawdius-gateway | 4,946 | **66.03%** | 501 | 52.69% | 3,176 | **60.80%** |
| clawdius-core | 70,273 | **65.16%** | 5,899 | 62.81% | 49,126 | **64.44%** |
| clawdius | 6,658 | **7.66%** | 289 | 12.46% | 4,912 | **5.62%** |

## Totals

| Metric | Count | Missed | Cover |
|---|---|---|---|
| Regions | 83,270 | 32,313 | **61.19%** |
| Functions | 6,793 | 2,684 | **60.49%** |
| Lines | 57,862 | 23,350 | **59.65%** |
| Branches | 0 | 0 | N/A (no branch instrumentation) |

**Note:** Branch coverage shows 0/0 because the default `cargo llvm-cov` uses line/region coverage. To get actual branch coverage, add `--branch` flag:
```
cargo llvm-cov --workspace --lib --branch --ignore-filename-regex="tests/" --output-dir=target/coverage
```

## Key Findings

1. **Two crates have 100% coverage** — `clawdius-code` and `clawdius-mcp` are fully covered.
2. **`clawdius` (CLI) is critically under-tested** — only 5.62% line coverage. Nearly all CLI subcommands (chat, auto, checkpoint, config, git, memory, etc.) sit at 0.00%. This is the biggest coverage gap.
3. **`clawdius-core` is moderately covered at ~64%** — the bulk of the codebase. Several modules have strong coverage (`code_parser` 94.5%, `error_recovery` 90%, `browser_daemon` 72.8%) but others are at 0% (`actions/refactor`, `actions/mod`).
4. **`clawdius-gateway` at ~61%** — reasonable but needs improvement for a network-facing component.
5. **Total workspace coverage: ~60%** — solid foundation, CLI layer is the primary gap.

## Recommendations for CI Integration

### 1. Add coverage job to CI

```yaml
# .github/workflows/coverage.yml
name: Coverage
on: [push, pull_request]
jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: llvm-tools-preview
      - run: cargo install cargo-llvm-cov
      - run: cargo llvm-cov --workspace --lib --branch --ignore-filename-regex="tests/" --output-dir=target/coverage -- --test-threads=1
      - run: cargo llvm-cov report --branch > coverage_report.txt
      - uses: actions/upload-artifact@v4
        with:
          name: coverage-report
          path: |
            target/coverage/
            coverage_report.txt
```

### 2. Enforce minimum coverage threshold

Add a CI gate that fails if coverage drops below the baseline:
```bash
# Fail if line coverage < 59.65%
cargo llvm-cov --workspace --lib --ignore-filename-regex="tests/" 2>&1 | tail -1 | awk '{if ($11+0 < 59.65) exit 1}'
```

### 3. Track coverage trends

Upload to Codecov or similar (the repo already has `codecov.yml`). The `--codecov-output` flag generates compatible JSON:
```bash
cargo llvm-cov --workspace --lib --codecov --output-path codecov.json
```

### 4. Priority areas for improvement

| Priority | Crate/Module | Current | Target | Approach |
|---|---|---|---|---|
| P0 | clawdius CLI | 5.62% | 40%+ | Add unit tests for each CLI subcommand handler |
| P1 | clawdius-core::actions | ~30% | 70%+ | Test refactor, docs action modules |
| P1 | clawdius-gateway | 60.80% | 80%+ | Add HTTP integration tests with httpmock |
| P2 | clawdius-core (overall) | 64.44% | 75%+ | Target uncovered modules with proptest/property tests |

### 5. Branch coverage

To get true branch coverage (currently showing N/A), run with `--branch`. This requires `llvm-tools-preview` component:
```bash
rustup component add llvm-tools-preview
cargo llvm-cov --workspace --lib --branch --ignore-filename-regex="tests/" --output-dir=target/coverage
```

## Generated Artifacts

- HTML report: `target/coverage/index.html`
- Raw data: `target/coverage/` directory
