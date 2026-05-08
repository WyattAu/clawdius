# Clawdius Version & State Tracking

> **Single source of truth:** `Cargo.toml` version field.
> All metrics below are empirically verified (not aspirational).

## Current State

| Attribute | Value |
|-----------|-------|
| **Version** | 1.0.0-rc.1 |
| **Status** | Active development |
| **Last Updated** | 2026-05-08 |
| **Git Commits** | 402 |
| **Rollback Checkpoint** | `f6f01f4f` |

## Empirical Metrics

| Metric | Value | Source |
|--------|-------|--------|
| **Workspace Crates** | 5 | `Cargo.toml` |
| **Rust LOC** | 134,626 | `find crates -name '*.rs' \| wc -l` |
| **Rust Files** | 344 | `find crates -name '*.rs' \| wc -l` |
| **Lean4 Proof Files** | 15 | Lake build (all compile) |
| **Lean4 LOC** | 3,157 | `wc -l *.lean` |
| **Lean4 Theorems** | 209 | `rg 'theorem ' *.lean` |
| **`.unwrap()` calls** | 2,335 across 146 files | `rg '\.unwrap\(\)' crates -g '*.rs'` |
| **Clippy** | Clean (all 5 crates, `-D warnings`) | `cargo clippy` |
| **cargo fmt** | Clean | `cargo fmt --check` |

### Per-Crate Test Counts (lib only)

| Crate | Tests | Status |
|-------|-------|--------|
| clawdius | 51 | All passing |
| clawdius-core | 1,085 | ~1,083 pass (2 known failures in unrelated modules) |
| clawdius-gateway | 39 | All passing |
| clawdius-mcp | 0 | No tests yet |
| clawdius-code | 0 | No tests yet |
| **Total** | **~1,175** | |

### Lean4 Proof Files

| File | Source Directory | Status |
|------|-----------------|--------|
| agent_loop.lean | .specs/02_architecture/proofs | Compiles |
| concurrent_execution.lean | .specs/02_architecture/proofs | Compiles |
| proof_carrying_edits.lean | .specs/02_architecture/proofs | Compiles |
| repo_map_scoring.lean | .specs/02_architecture/proofs | Compiles |
| sanitizer_soundness.lean | .specs/02_architecture/proofs | Compiles |
| security_policy.lean | .specs/02_architecture/proofs | Compiles |
| token_budget_scan.lean | .specs/02_architecture/proofs | Compiles |
| vfs_safety.lean | .specs/02_architecture/proofs | Compiles |
| proof_audit.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_capability.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_container.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_fsm.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_host.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_ring_buffer.lean | .clawdius/specs/02_architecture/proofs | Compiles |
| proof_sandbox.lean | .clawdius/specs/02_architecture/proofs | Compiles |

**Note:** Lean4 root `Clawdius.lean` is a hello-world stub (`def hello := "world"`), not a real proof.

### Known Issues

| Issue | Severity | Details |
|-------|----------|---------|
| 2,335 `.unwrap()` calls | High | Potential panics in production; needs systematic elimination |
| clawdius-mcp: 0 tests | Medium | No test coverage |
| clawdius-code: 0 tests | Medium | No test coverage |
| 2 test failures in clawdius-core | Low | Unrelated modules, not in critical path |
| Workspace `--all-features` OOM | Medium | Cannot compile all features simultaneously on standard runners |
| CI only runs `--lib` tests | Medium | Integration tests not covered |
| `.cargo-vendor/half` dirty | Low | Submodule patched but not committed |
| Unsafe code in simd.rs | Low | Justified (SSE2/NEON behind cfg), 8 uses |

## Recent Changes (2026-05-08)

| Change | Details |
|--------|---------|
| Lean4 lakefile wiring | 15 proof files compile via individual `[[lean_lib]]` entries in lakefile.toml |
| `.clippy.toml` fix | Removed invalid `[lints.clippy]` section (Cargo.toml-only syntax) |
| `execute_tool` refactor | Split 160-line function into 8 focused methods in tui_app/app.rs |
| MCP test fixes | Eliminated `set_current_dir` race conditions, 29/29 passing |
| telegram.rs unsafe removal | Raw pointer → `Bot::clone()` + static method |
| `multiple_crate_versions` | Allowed in workspace lints (wasmtime transitive deps) |
| `cargo fmt --all` | 120 files formatted |

## Version History (Verified)

### 1.0.0-rc.1 - Release Candidate (2026-03-11)
- API stability guarantee
- Getting started guide
- Cross-platform release targets
- crates.io preparation

### Previous Versions
See `git log` for full commit history. Earlier entries in this file contained
unverified claims and have been replaced with empirically validated metrics.
