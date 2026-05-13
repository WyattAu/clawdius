# Clawdius Roadmap: Path Forward

> State as of 2026-05-11. Based on empirical audit of 1,735 tests, 344 Rust files, 15 Lean4 proof files.

## Current State Summary

| Dimension | Status | Details |
|-----------|--------|---------|
| Tests | 1,735 passing, 0 failures | 1,194 lib + 158 integration + 383 other |
| Clippy | Clean | `-D warnings` across all 5 crates |
| Format | Clean | `cargo fmt --all --check` |
| Deny | Clean | Advisories, licenses, bans |
| Lean4 | 31/31 jobs pass | 15 proof files, 209 theorems |
| Pre-commit | 5-check gate | fmt, clippy, tests, deny, Lean4 |
| CVEs | 6 transitive | Documented, blocked on upstream |

## Phase 1: Hardening (Week 1-2)

### 1.1 Production Unwrap Elimination

**Problem:** 121 production files contain `.unwrap()` calls. The workspace allows `unwrap_used = "allow"`.

**Plan:**
- Per-crate audit: classify each `.unwrap()` as infallible (e.g., `Mutex::lock` after `new`) or fallible
- Replace fallible unwraps with `?` propagation or `expect("invariant: ...")` with justification
- Escalate workspace lint: `unwrap_used = "warn"` (currently `allow`)
- Target: < 30 production files with unwraps

**Success criteria:** `rg '\.unwrap\(\)' crates/ -g '*.rs' --files | grep -v test | wc -l` < 30

### 1.2 Transitive Dependency CVEs

**Problem:** 6 CVEs in rustls-webpki and matrix-sdk-base.

**Plan:**
- Monitor lancedb releases for rustls upgrade (unblocks 4 CVEs)
- Monitor matrix-sdk 0.11 release (unblocks 2 CVEs)
- Evaluate alternative: pin rustls independently via `[patch.crates-io]`
- For matrix-sdk: consider conditional compilation (only build matrix adapter when feature is enabled)

**Blocked on:** lancedb >= 0.28, matrix-sdk >= 0.11

### 1.3 Documentation Cleanup

**Problem:** 15+ root markdown files with emojis, version inconsistencies, and stale content.

**Plan:**
- Strip all emoji from root .md files (RELEASE_NOTES, ROADMAP, SECURITY, etc.)
- Consolidate duplicate files: `ROADMAP.md` + `ROADmap.md`, `CHANGELOG.md` + `CHANGES.md`
- Archive historical implementation logs to `.reports/archived/`
- Standardize version references to single source: `Cargo.toml` -> `VERSION.md`
- Fix inaccurate references (e.g., `clawdius-webview` removal, `clawdius-server` -> `clawdius-gateway`)

### 1.4 Pre-commit Hook Hardening

**Problem:** Hook checks working tree, not staged content. IDE file watchers can clobber files between stage and commit.

**Plan:**
- Use `git stash --keep-index` pattern to isolate staged content
- Add `--locked` flag to cargo checks for reproducibility
- Add Lean4 proof check as mandatory (not optional)
- Add `cargo test --workspace` (full suite, not just lib) as optional slow path

## Phase 2: Test Coverage (Week 2-4)

### 2.1 Branch Coverage Baseline

**Problem:** No empirical branch coverage measurement.

**Plan:**
- Add `tarpaulin` or `llvm-cov` to CI
- Establish baseline branch coverage per crate
- Target: > 80% overall, > 95% critical path (llm, sandbox, session, config)
- Add coverage report to VERSION.md

### 2.2 clawdius-mcp Test Expansion

**Problem:** Only 5 integration tests for MCP protocol.

**Plan:**
- Add unit tests for MCP handler (currently 0 lib tests)
- Add error path tests (malformed requests, timeout, resource limits)
- Add concurrency tests (parallel tool execution)
- Target: 30+ tests for clawdius-mcp

### 2.3 clawdius-code Test Expansion

**Problem:** Only 5 integration tests for JSON-RPC.

**Plan:**
- Add unit tests for JSON-RPC method dispatch
- Add error path tests
- Add VSCode extension communication tests
- Target: 30+ tests for clawdius-code

### 2.4 Property-Based Testing

**Plan:**
- Add `proptest` properties for core data structures (session, config, token counting)
- Add `proptest` for HTML compressor (roundtrip properties)
- Add `proptest` for diff computation (idempotency, reversibility)

## Phase 3: Architecture (Week 4-6)

### 3.1 lib.rs Protection

**Problem:** `crates/clawdius-core/src/lib.rs` keeps getting clobbered to a cargo init stub by agentic tools.

**Plan:**
- Add `.gitattributes` merge driver for lib.rs
- Add CI check: fail if lib.rs has < 10 `pub mod` declarations
- Consider splitting lib.rs into a `mod.rs` tree that's harder to accidentally overwrite
- Root cause: identify which tool is writing the stub and add explicit exclusion

### 3.2 Crate Dependency Simplification

**Problem:** 20+ transitive version duplicates (documented in Cargo.toml).

**Plan:**
- Audit each duplicate: is it semver-incompatible or lazy upstream?
- Where possible, use `[patch.crates-io]` to unify
- Document remaining duplicates with justification
- Target: < 10 documented duplicates

### 3.3 Feature Flag Audit

**Problem:** Workspace `--all-features` causes OOM.

**Plan:**
- Map all feature flags per crate
- Identify conflicting feature combinations
- Add CI matrix for feature combinations
- Document supported feature sets

## Phase 4: Performance (Week 6-8)

### 4.1 PGO Build Pipeline

**Problem:** PGO profiles defined in Cargo.toml but no CI pipeline to generate profiling data.

**Plan:**
- Add CI job: build with `pgo-instrument` profile
- Run representative workload (file analysis, LLM streaming, session management)
- Build with `pgo-optimized` profile using collected data
- Benchmark before/after
- Target: 10-20% latency improvement on hot paths

### 4.2 Benchmark Suite

**Problem:** BENCHMARKS.md has data but no automated regression detection.

**Plan:**
- Wire `criterion` benchmarks to CI
- Establish baseline metrics in `.specs/06_5_regression/baseline_metrics.toml`
- Add regression detection with 5% threshold
- Target: automated alerts on performance regressions

### 4.3 Memory Profiling

**Plan:**
- Add `dhat` or `jemalloc` profiling to CI
- Identify allocation hotspots
- Target: < 100MB RSS for typical CLI session

## Phase 5: Release Preparation (Week 8-10)

### 5.1 crates.io Publishing

**Plan:**
- Verify all crate metadata (description, categories, keywords)
- Ensure no `path` dependencies leak to published crates
- Test dry-run publish: `cargo publish --dry-run`
- Set up CI for automated publish on tag

### 5.2 API Stability

**Problem:** Many modules use `pub` broadly without semver guarantees.

**Plan:**
- Audit public API surface per crate
- Mark unstable APIs with `#[doc(hidden)]` or `#[unstable]`
- Document stable API contract
- Target: explicit semver stability for all public types

### 5.3 Cross-Platform Testing

**Plan:**
- Add CI matrix: Linux (x86_64, aarch64), macOS (aarch64), Windows (x86_64)
- Test sandbox backends per platform (bubblewrap = Linux only)
- Test WASM compilation for clawdius-webview (if re-enabled)

## Phase 6: Ecosystem (Week 10-12)

### 6.1 Editor Integration Hardening

**Plan:**
- Verify VSCode extension builds and loads
- Test LSP client against real language servers
- Add integration tests for editor communication
- Document extension installation workflow

### 6.2 Gateway Platform Adapters

**Problem:** 9 platform adapters (Telegram, Discord, Slack, Matrix, etc.) but only mock adapter tested.

**Plan:**
- Add unit tests for each adapter's message formatting
- Add integration tests with mock servers
- Target: > 20 tests per adapter for active platforms (Telegram, Discord)

### 6.3 Documentation Site

**Plan:**
- Generate API docs from rustdoc
- Add getting-started guide with verified commands
- Add architecture overview diagram
- Host on docs.clawdius.dev

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| lib.rs clobbering by agentic tools | High | High | `.gitattributes`, CI check, root cause fix |
| lancedb upgrade breaks vector DB | Medium | High | Pin version, test before upgrade |
| OOM on CI with all features | High | Medium | Feature matrix CI, memory profiling |
| Production unwrap panics | Medium | High | Phase 1.1 unwrap elimination |
| VSCode extension breakage | Low | Medium | Integration tests, manual smoke test |

## Decision Log

| ID | Decision | Rationale | Date |
|----|----------|-----------|------|
| DL-001 | Allow unwrap_used in workspace | 1,664 existing uses; incremental elimination | 2026-05-08 |
| DL-002 | Document CVEs in deny.toml | Cannot fix transitive deps; transparency > silence | 2026-05-11 |
| DL-003 | Keep Lean4 proofs as compile-check only | Full verification requires `lean` runtime; CI has it | 2026-05-11 |
| DL-004 | Use `--no-verify` for emergency fixes | Pre-commit hook checks working tree, not staged | 2026-05-11 |
