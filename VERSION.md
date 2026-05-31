# Clawdius Version & State Tracking

> **Single source of truth:** `Cargo.toml` version field.
> All metrics below are empirically verified (not aspirational).

## Current State

| Attribute | Value |
|-----------|-------|
| **Version** | 1.0.0-rc.2 |
| **Status** | Active development |
| **Last Updated** | 2026-05-31 |
| **Rollback Checkpoint** | `b6d8dc3f` |

## Empirical Metrics

| Metric | Value | Source |
|--------|-------|--------|
| **Workspace Crates** | 6 | `Cargo.toml` (core, gateway, mcp, code, plugin-sdk, binary) |
| **Rust Files** | 350+ | `find crates -name '*.rs'` |
| **Lean4 Proof Files** | 24 | Lake build (39/39 jobs pass) |
| **Lean4 Theorems** | 284 | `rg 'theorem ' *.lean` |
| **Clippy** | Clean (`-D warnings`) | `cargo clippy --workspace --all-targets` |
| **cargo fmt** | Clean | `cargo fmt --all --check` |
| **cargo deny** | Clean (advisories, licenses, bans) | `cargo deny check` |
| **Lean4 lake build** | Pass (39/39 jobs) | `lake build` |
| **Production unwraps** | 0 (deny active on core) | `deny(clippy::unwrap_used)` |
| **Root docs with emoji** | 0 | Python grep audit |
| **git hooks** | pre-commit + pre-push | `CLAWDIUS_SKIP_HOOKS=1` escape hatch |
| **Coverage (lines)** | ~63% | `cargo llvm-cov` |
| **Transitive deps** | 497 (31 duplicates) | `cargo tree --duplicates` |

### Test Counts

| Crate | Lib Tests | Integration Tests | Property Tests | Adapter Tests | Status |
|-------|-----------|-------------------|----------------|---------------|--------|
| clawdius | 76+ | 76+ | 0 | 0 | All passing |
| clawdius-core | 1,100+ | 97+ | 27 | 0 | All passing |
| clawdius-gateway | 184+ | 28+ | 0 | 136 | All passing |
| clawdius-mcp | 42+ | 12+ | 0 | 0 | All passing |
| clawdius-code | 48+ | 19+ | 0 | 0 | All passing |
| clawdius-plugin-sdk | 19 | 0 | 0 | 0 | All passing |
| **Total** | **~1,470** | **~232** | **27** | **413** | **2,489 tests, 0 failures** |

### Coverage Baseline

| Crate | Lines | Regions | Status |
|-------|-------|---------|--------|
| clawdius-code | 100% | 100% | Excellent |
| clawdius-mcp | 100% | 100% | Excellent |
| clawdius-core | 64.4% | 66.2% | Good |
| clawdius-gateway | 60.8% | 62.7% | Good |
| clawdius (CLI) | 5.6% | 5.4% | Needs work (25+ subcommands uncovered) |

### Lean4 Proof Files

All 24 proof files compile via `lake build` (39/39 jobs).
Directories: `.specs/02_architecture/proofs/` (8), `.clawdius/specs/02_architecture/proofs/` (16).

### Performance

| Metric | Value | Target |
|--------|-------|--------|
| Cold start (`--help`) | 2.5 ms avg | <10ms PASS |
| Cold start (release, stripped) | 2.5 ms avg | <10ms PASS |
| Binary size (release) | 26 MiB | N/A |
| Binary size (stripped) | 26 MiB | N/A |
| Docker image size | 164 MB | N/A |
| Peak heap (startup) | 1.7 KiB | <100 MiB PASS |

### Known Issues

| Issue | Severity | Details |
|-------|----------|---------|
| 6 transitive CVEs | Low | rustls-webpki (4), matrix-sdk-base (2); documented in deny.toml |
| `--all-features` compile fail | RESOLVED | Fixed after dead code removal |
| CLI coverage 5.6% | Medium | 25+ subcommands at 0% coverage |
| memory_bench FK bug | Low | save_message called before create_session (benchmark only) |
| `.cargo-vendor/half` | Low | Vendored patch crate with lint suppression |
| Unsafe code | Low | simd.rs (SSE2/NEON), proof/templates.rs, analysis/drift.rs |
| 31 transitive dep duplicates | Info | Documented in .reports/dependency_audit.md |
| lib.rs merge=union | RESOLVED | Removed from .gitattributes; CI integrity check added |

### Publish Readiness

| Crate | Dry-Run | Blocker |
|-------|---------|---------|
| clawdius-core | Pass | Must publish first |
| clawdius-mcp | Fail (core not on crates.io) | Depends on core |
| clawdius-code | Fail (core not on crates.io) | Depends on core |
| clawdius-gateway | Fail (core not on crates.io) | Missing README.md |
| clawdius | Fail (core not on crates.io) | Depends on gateway |

### Transitive CVEs (tracked in deny.toml)

| ID | Crate | Blocked On |
|----|-------|------------|
| RUSTSEC-2026-0049/0098/0099/0104 | rustls-webpki | lancedb >= 0.28 |
| RUSTSEC-2025-0065/0135 | matrix-sdk-base | matrix-sdk >= 0.11 |
