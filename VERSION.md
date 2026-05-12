# Clawdius Version & State Tracking

> **Single source of truth:** `Cargo.toml` version field.
> All metrics below are empirically verified (not aspirational).

## Current State

| Attribute | Value |
|-----------|-------|
| **Version** | 1.0.0-rc.1 |
| **Status** | Active development |
| **Last Updated** | 2026-05-12 |
| **Rollback Checkpoint** | `885a67c4` |

## Empirical Metrics

| Metric | Value | Source |
|--------|-------|--------|
| **Workspace Crates** | 5 | `Cargo.toml` |
| **Rust Files** | 344 | `find crates -name '*.rs'` |
| **Lean4 Proof Files** | 15 | Lake build (31/31 jobs pass) |
| **Lean4 Theorems** | 209 | `rg 'theorem ' *.lean` |
| **Clippy** | Clean (`-D warnings`) | `cargo clippy --workspace --all-targets` |
| **cargo fmt** | Clean | `cargo fmt --all --check` |
| **cargo deny** | Clean (advisories, licenses, bans) | `cargo deny check` |
| **Lean4 lake build** | Pass (31/31 jobs) | `lake build` |
| **Production unwraps** | 0 fallible (9 in doc comments/debug) | Manual audit |
| **Root docs with emoji** | 0 | Grep audit |
| **git hooks** | pre-commit + pre-push | Enforce quality gates at commit and push |
| **git hooks** | pre-commit + pre-push | Enforce all quality gates |

### Test Counts

| Crate | Lib Tests | Integration Tests | Status |
|-------|-----------|-------------------|--------|
| clawdius | 12 | 51 | All passing |
| clawdius-core | 1,075 | 97 | All passing (2 ignored) |
| clawdius-gateway | 107 | 0 | All passing |
| clawdius-mcp | 9 | 5 | All passing |
| clawdius-code | 9 | 5 | All passing |
| **Total** | **1,212** | **158** | **0 failures** |

### Lean4 Proof Files

All 15 proof files compile via `lake build` (31/31 jobs).
Directories: `.specs/02_architecture/proofs/` (8), `.clawdius/specs/02_architecture/proofs/` (7).

### Known Issues

| Issue | Severity | Details |
|-------|----------|---------|
| 6 transitive CVEs | Low | rustls-webpki (4), matrix-sdk-base (2); documented in deny.toml |
| `--all-features` OOM | Medium | Cannot compile all features simultaneously |
| `.cargo-vendor/half` | Low | Vendored patch crate with lint suppression |
| Unsafe code | Low | simd.rs (SSE2/NEON), proof/templates.rs, analysis/drift.rs |
| 20+ transitive dep duplicates | Info | Documented in Cargo.toml comments |

### Transitive CVEs (tracked in deny.toml)

| ID | Crate | Blocked On |
|----|-------|------------|
| RUSTSEC-2026-0049/0098/0099/0104 | rustls-webpki | lancedb >= 0.28 |
| RUSTSEC-2025-0065/0135 | matrix-sdk-base | matrix-sdk >= 0.11 |
