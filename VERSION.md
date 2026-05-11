# Clawdius Version & State Tracking

> **Single source of truth:** `Cargo.toml` version field.
> All metrics below are empirically verified (not aspirational).

## Current State

| Attribute | Value |
|-----------|-------|
| **Version** | 1.0.0-rc.1 |
| **Status** | Active development |
| **Last Updated** | 2026-05-11 |
| **Rollback Checkpoint** | `c9a6c546` |

## Empirical Metrics

| Metric | Value | Source |
|--------|-------|--------|
| **Workspace Crates** | 5 | `Cargo.toml` |
| **Rust Files** | 344 | `find crates -name '*.rs'` |
| **Lean4 Proof Files** | 15 | Lake build (all compile, 31 jobs) |
| **Lean4 Theorems** | 209 | `rg 'theorem ' *.lean` |
| **Clippy** | Clean (all 5 crates, `-D warnings`) | `cargo clippy --workspace --all-targets` |
| **cargo fmt** | Clean | `cargo fmt --all --check` |
| **cargo deny** | Clean (advisories, licenses, bans) | `cargo deny check` |
| **Lean4 lake build** | Pass (31/31 jobs) | `lake build` |

### Test Counts (workspace-wide)

| Crate | Lib Tests | Integration Tests | Status |
|-------|-----------|-------------------|--------|
| clawdius | 12 | 51 | All passing |
| clawdius-core | 1,075 | 97 | All passing (2 ignored) |
| clawdius-gateway | 107 | 0 | All passing |
| clawdius-mcp | 0 | 5 | All passing |
| clawdius-code | 0 | 5 | All passing |
| **Total** | **1,194 lib** | **158 integration** | **0 failures** |

### Lean4 Proof Files

All 15 proof files compile via `lake build` (31 jobs total).
Source directories: `.specs/02_architecture/proofs/` (8 files), `.clawdius/specs/02_architecture/proofs/` (7 files).

### Known Issues

| Issue | Severity | Details |
|-------|----------|---------|
| Workspace `--all-features` OOM | Medium | Cannot compile all features simultaneously |
| `.cargo-vendor/half` dirty | Low | Submodule patched but not committed |
| Unsafe code in simd.rs, proof/templates.rs, analysis/drift.rs | Low | Justified (SSE2/NEON, proof templates, drift detection) |
| 6 transitive CVEs (rustls-webpki, matrix-sdk-base) | Low | Documented in deny.toml, blocked on upstream |
| 121 production files with `.unwrap()` | Medium | Plan to escalate to warn |

## Version History

### 1.0.0-rc.1 - Release Candidate (2026-03-11)
- API stability guarantee
- Getting started guide
- Cross-platform release targets
- crates.io preparation
