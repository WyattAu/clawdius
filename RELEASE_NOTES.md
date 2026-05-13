# Clawdius Release Notes

## v1.0.0-rc.1 (2026-05-11)

### Overview
Release candidate 1 for Clawdius v1.0.0. All quality gates pass: 1,395 tests (0 failures), clippy clean, Lean4 proofs compile (31/31 jobs), cargo deny clean.

### Critical Changes
- Production unwrap auditing: zero production .unwrap() calls across all 5 workspace crates
- Clippy enforcement: `-D warnings` clean on all targets (lib + tests + benches)
- Git hooks: pre-commit (fast) and pre-push (full suite) enforcement
- Lean4 proofs: 15 proof files, 209 theorems verified via `lake build`
- Benchmark infrastructure: criterion benchmarks wired with CI regression detection

### Known Issues
- 2 transitive CVEs (matrix-sdk-base), blocked on upstream matrix-sdk >= 0.11
- `--all-features` OOM on CI (documented limitation, feature matrix planned)
- 20+ transitive dependency version duplicates (documented in Cargo.toml)
- Git hooks may timeout on cold cache (documented in VERSION.md)

### Upgrade Notes
- Workspace lints: `unwrap_used = "warn"` at workspace level
- clawdius-core: `#![deny(clippy::unwrap_used)]` in production code
- Test code: `unwrap()` and `expect()` allowed via `#![cfg_attr(test, allow(...))]`
- Pre-commit hook: 7 checks (lib.rs integrity, merge markers, fmt, clippy, lib tests, deny, Lean4)
- Pre-push hook: 6 checks (full lib tests, integration, all tests, clippy, deny, Lean4)

### Archive
Historical release notes and development roadmaps archived in `.reports/archived/`.
