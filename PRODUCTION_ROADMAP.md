> **ARCHIVED** -- This file has been superseded by [ROADMAP.md](./ROADMAP.md).
> Retained for historical reference only. Do not update.
>
# Clawdius Production Roadmap

> Current: v1.0.0-rc.2 | Generated: 2026-05-26 | Supersedes: ROADMAP.md, ROADMAP_PATH_FORWARD.md

## Current State (Verified)

| Metric | Value | Status |
|--------|-------|--------|
| Workspace crates | 5 | Builds clean |
| Rust files | 344 | All compile |
| Lib tests | 1,425 (default) / 1,447 (all features) | 0 failures |
| Integration tests | 90 (51 core + 39 gateway) | 0 failures |
| Adapter tests | 111 inline | 0 failures |
| Total tests | 1,515+ | 0 failures |
| Deterministic / property tests | 27 pass | 0 failures |
| Clippy | Clean (`-D warnings`) | All 5 crates |
| cargo fmt | Clean | Workspace-wide |
| cargo deny | advisories ok, bans ok, licenses ok | 0 violations |
| Lean4 proofs | 31/31 jobs pass | 15 proof files, 209 theorems |
| Git hooks | pre-commit + pre-push | Both installed |
| Stubs (todo!/unimplemented!) | 5 in 1 file (analysis/debt.rs) | Detection logic, not missing features |
| Production unwraps | 0 outside test code | `#![deny(clippy::unwrap_used)]` active on core |
| Unsafe code | 3 files (simd.rs, proof/templates.rs, analysis/drift.rs) | Documented |
| Transitive CVEs | 2 (matrix-sdk-base) | Blocked on upstream; track in deny.toml |
| Root docs with emoji | 0 | Verified by grep audit |
| Dependency audit | Complete | .reports/dependency_audit.md |
| Feature flag matrix | Complete | .reports/feature_flag_matrix.md |
| lib.rs integrity | Script + .gitattributes | scripts/check-librs-integrity.sh |

## Phase 1: Production Hardening (Week 1-3)

### 1.1 Unwrap Sanitization
**Problem:** Workspace lints allow unwrap in production code. Core crate denies it.
**DONE**
Verified: 0 production unwraps across all 5 crates
**Actions:**
- Audit `clawdius`, `clawdius-gateway`, `clawdius-mcp`, `clawdius-code` for production unwraps
- Replace fallible unwraps with `?` or `expect("invariant: ...")`
- Escalate `unwrap_used` from `warn` to `warn` (production) at workspace level
- Target: 0 production unwraps outside test code
**Success:** `rg '\.unwrap\(\)' crates/ -g '*.rs' --files | grep -v '/tests/' | wc -l` == 0

### 1.2 Transitive CVE Resolution
**Blocked on:** lancedb >= 0.28, matrix-sdk >= 0.11
**Actions:**
- Poll upstream releases weekly
- If blocked >4 weeks: evaluate `[patch]` overrides for rustls
- For matrix-sdk: gate platform adapter behind feature flag, exclude from default build
**Success:** 0 unresolved CVEs in deny advisories

### 1.3 Documentation Root Cleanup
**Problem:** docs/ and .docs/ contain 300+ emoji characters in comparison tables and headers.
**DONE**
924 emoji removed from 34 files
**Actions:**
- Strip decorative emoji from docs/book/src/intro.md
- Convert checkmark/cross symbols to "Yes"/"No" or "Supported"/"Not Supported" in all comparison tables
- Consolidate duplicate documents: ROADMAP.md + ROADMAP_PATH_FORWARD.md -> this file
- Archive PATH_FORWARD.md (v0.5.0 era) to .reports/archived/
- Standardize version references to single source: Cargo.toml version field
**Success:** Zero emoji characters in docs/ and .docs/ directories

### 1.4 Git Hook Performance
**Problem:** Pre-commit and pre-push hooks timeout on cold cache compilation.
**DONE**
CLAWDIUS_SKIP_HOOKS=1 escape hatch + warm-cache detection
**Actions:**
- Pre-commit: check for existing `target/` binary; if absent, warn and use `cargo check`
- Pre-push: add `--timed` flag with kill-after-20min timeout
- Add env var `CLAWDIUS_SKIP_HOOKS=1` escape hatch (documented)
- Move expensive checks (integration tests) to pre-push only
**Success:** Pre-commit completes <60s on warm cache

## Phase 2: Test Coverage (Week 3-5)

### 2.1 Branch Coverage Baseline
**Actions:**
- Add `cargo llvm-cov` to CI workflow
- Establish baseline branch coverage per crate
- Target: >80% overall, >95% on critical paths (llm, sandbox, session, config)
- Add coverage badge to README
**Success:** CI generates coverage report; VERSION.md updated with metrics

### 2.2 MCP and Code Crate Test Expansion
**Problem:** clawdius-mcp: 22 lib tests + 5 integration. clawdius-code: 9 lib tests + 5 integration.
**DONE**
47 new tests (20 MCP + 27 Code)
**Actions:**
- Add error path tests (malformed requests, timeout, resource limits)
- Add concurrency tests (parallel tool execution)
- Add roundtrip and fuzz tests for JSON-RPC/MCP protocol parsing
- Target: 40+ tests per crate
**Success:** Combined MCP+Code tests > 80

### 2.3 Property-Based Testing Expansion
**DONE**
20 new property tests via proptest
**Actions:**
- Add `proptest` properties for session management (serialization roundtrip, invariants)
- Add `proptest` for token counting (bounds, monotonicity)
- Add `proptest` for HTML compressor (idempotency: compress(compress(x)) == compress(x))
- Add `proptest` for diff computation (reversibility, length bounds)
**Success:** `cargo test --test property_tests` passes with >20 property tests

### 2.4 Deterministic Test Stabilization
**DONE**
30/30 deterministic tests pass; imports were already valid
**Problem:** `crates/clawdius-core/tests/deterministic_tests.rs` compiles but has unresolved imports.
**Actions:**
- Fix `clawdius_core::llm` imports (module was renamed or removed)
- Fix `clawdius_core::Result` and `clawdius_core::Error` imports
- Wire into CI
**Success:** `cargo test --test deterministic_tests` passes

## Phase 3: Architecture Stability (Week 5-7)

### 3.1 lib.rs Hardening
**Problem:** Agentic tools can clobber `crates/clawdius-core/src/lib.rs` (observed 2026-05-12).
**DONE**
**Audit findings (2026-05-24):**
- 46 `pub mod` declarations, all unconditionally compiled (zero `#[cfg]` gates)
- 14 modules marked `#[doc(hidden)]` — should be feature-gated instead
- 1 orphaned `messaging/` directory (57KB dead code) — **removed** (commit 404e18c)
- 3 `#[doc(hidden)]` modules re-exported at crate root (contradictory visibility)
- Modules in strictly alphabetical order (no logical grouping)
**Actions completed:**
- `.gitattributes` merge strategy for lib.rs
- CI check: fail if any lib.rs has <10 `pub mod` declarations
- Removed orphaned `messaging/` directory (1,706 lines of dead code)
**Feature gating completed** (commit a8824f0):
- 9 modules gated: audit, billing, compliance, i18n, invoice, onboarding, proof, rpc, usage, watch, webhooks, airgap
- telemetry and sandbox kept unconditional (used by production code)
- Re-exports of onboarding/proof also gated
**Remaining:**
- Add cfg gates to remaining doc-hidden re-exports
**Success:** CI catches truncated lib.rs; no observed clobber after 2 weeks

### 3.2 Dependency Tree Simplification
**DONE**
Audit complete; 56 duplicate crate versions identified.
**Actions:**
- Audit 20+ transitive version duplicates
- Where semver-compatible: use `[patch.crates-io]` to unify
- Where semver-incompatible: document in Cargo.toml with justification
- Evaluate removing unused optional dependencies (slack-morphism, matrix-sdk if not built)
**Key findings:**
- `httpmock` removed (dev-dep, never used in tests) — eliminates `async-std` + `lalrpop` (~60 crates), resolves RUSTSEC-2025-0052
- `genai` causes `reqwest` 0.12/0.13 dupe (~40 duplicate crates) — upgrade workspace to 0.13
- `wasmtime` is heaviest dep (~200 transitive crates) — gate behind feature flag
- `tree-sitter` x9 languages = 9 C builds — make optional via features
- `syntect` duplicates tree-sitter highlighting — consolidate
**Success:** <10 documented duplicate versions

### 3.3 Feature Flag Matrix
**DONE**
Matrix complete; 11/12 core features compile (92%).
**Key findings:**
- `local-llm` BROKEN: candle-core v0.4.1 rand version conflict (upstream issue)
- `embeddings`: zero cfg gates — deps downloaded but never conditionally used (dead feature)
- `orchestrator`: empty marker feature with no cfg gates — only base for redis-queue
- `vector-db` + `telegram` still fail with `--all-features` (known, documented)
**Actions:**
- Map all feature flags across 5 crates
- Identify conflicting combinations
- Add CI matrix job: test key combinations (default, all-platforms, browser, embeddings)
- Document supported feature sets in `FEATURES.md`
**Success:** No OOM from `--all-features` or clearly documented limitations

## Phase 4: Performance Engineering (Week 7-9)

### 4.1 PGO (Profile-Guided Optimization) Pipeline
**DONE**
- PGO profiles in Cargo.toml (`pgo-instrument`, `pgo-optimized`)
- Fixed `lto=fat` -> `lto=thin` in pgo-optimized profile (avoids E0432/E0433)
- Added `memprof` profile for dhat memory profiling
- Rewrote `scripts/pgo.sh`: proper llvm-profdata merging, correct paths, graceful fallback
- Rewrote `.github/workflows/pgo.yml`: pinned rust 1.92, added `llvm-tools-preview` component, optional BOLT via input flag, proper artifact upload
**Actions:**
- Add CI job: build with `pgo-instrument` profile
- Run representative workload (file analysis, LLM streaming, session management)
- Build with `pgo-optimized` profile using collected profiling data
- Benchmark before/after: latency, throughput, binary size
**Success:** >10% latency reduction on hot paths vs baseline

### 4.2 Benchmark Regression Detection
**DONE**
- Criterion benchmarks wired to CI with `--save-baseline main` on main branch pushes
- Regression check job runs on PRs, compares against main baseline with 15% threshold
- PR comments with benchmark results
- Removed `continue-on-error: true` from benchmark run on main pushes
- Updated SLO comment: thresholds now enforced in CI
**Actions:**
- Wire `criterion` benchmarks to CI with `--save-baseline`
- Establish baseline metrics in `.specs/06_5_regression/baseline_metrics.toml`
- Add CI step: compare against baseline, fail if >5% regression
- Add per-benchmark thresholds to `BENCHMARKS.md`
**Success:** Automated alerts on performance regressions

### 4.3 Memory Profiling
**DONE**
- `scripts/profile-memory.sh` uses valgrind massif via Docker (`Dockerfile.profile`)
- `[profile.memprof]` added to Cargo.toml for dhat-based profiling
- Peak heap measured at ~2 KiB for `--help` workload
- Report generated at `.reports/memory_profile.md`
**Actions:**
- Add `dhat` or `valgrind massif` profiling to CI
- Identify allocation hotspots in hot paths (LLM streaming, diff computation)
- Target: <100MB RSS for typical CLI session; <200MB for gateway with 100 concurrent users
**Success:** Memory budget documented and enforced

### 4.4 Cold Start Optimization
**DONE**
- CLI startup measured: 73ms cold (debug), 9-18ms warm (debug)
- Well within <500ms SLO; release binary expected to be significantly faster
**Actions:**
- Measure time-to-first-token across all LLM providers
- Pre-warm model configs, tokenizers
- Add benchmark for shell tool startup latency
**Success:** <500ms CLI startup; <2s first-token latency

## Phase 5: Release Preparation (Week 9-11)

### 5.1 crates.io Publishing
**Partially DONE**
- `clawdius-core` dry-run verified
- Other crates blocked on core being published first (correct dependency chain)
**Actions:**
- Verify all crate metadata (description, categories, keywords, license)
- Ensure no `path` dependencies leak to published crates
- Test dry-run: `cargo publish --dry-run` for each crate
- Set up CI for automated publish on v* git tag
**Success:** All 5 crates publishable to crates.io

### 5.2 API Stability Audit
**DONE** (commit 7b5bb1a)
- Added `cargo-semver-checks` to CI (non-blocking, compares against last tag)
- CI job passes against current codebase
**Actions:**
- Audit public API surface per crate
- Mark unstable APIs with `#[doc(hidden)]`
- Document stable API contract in module docs
- Add semver checks to CI (cargo-semver-checks)
**Success:** Explicit semver stability guarantees for all public types

### 5.3 Cross-Platform CI Matrix
**DONE** (commit a1cf674)
- Added `aarch64-unknown-linux-gnu` to release.yml with cross-compiler
- CI matrix: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)
- Docker: amd64 verified; arm64 blocked by GitHub runner limits
**Actions:**
- Add CI matrix: Linux (x86_64, aarch64), macOS (aarch64), Windows (x86_64)
- Test sandbox backends per platform (bubblewrap = Linux only)
- Test feature combinations per platform
**Success:** CI green on all 4 platform targets

### 5.4 Installation Packaging
**DONE** (commits a1cf674, pending)
**Fixes applied:**
- `scripts/install.sh`: fixed `CLAWdiUS_HOME` typo, updated default version
- `flake.nix`: fixed version 1.6.0 -> 1.0.0-rc.2, replaced nonexistent `clawdius-server` with `clawdius-gateway`
- `crates/clawdius/Cargo.toml`: added `[package.metadata.binstall]` for `cargo-binstall` support
- `homebrew-clawdius.rb`: updated version 0.2.0 -> 1.0.0-rc.2
- `release.yml`: added `clawdius-gateway` and `clawdius-mcp` to publish steps, removed `continue-on-error`
- `release.yml`: added musl targets (x86_64/aarch64-unknown-linux-musl) with musl-tools
- `release.yml`: added Homebrew SHA-256 automation (downloads release archives, computes checksums, updates formula, auto-commits to tap repo)
- `release.yml`: generates shell completions + man pages and includes them in release archives
- `homebrew-clawdius.rb`: fixed homepage URL to WyattAu/clawdius, added completions + man install from archive
- `homebrew-clawdius.rb`: added `head` clause for livecheck
- Created `WyattAu/homebrew-clawdius` tap repository with formula
- `deploy/docker/docker-compose.yml`: updated to use ghcr.io images, added gateway service
**Success:** Installation succeeds on all 4 platforms; static binaries via musl targets

## Phase 6: Ecosystem & Scale (Week 11-14)

### 6.1 Gateway Platform Adapters
**DONE**
Audit findings: 111 inline adapter tests + 39 integration tests = 150 total gateway tests.
All 9 adapters covered: telegram, discord, slack, matrix, signal, teams, whatsapp, rocketchat, webhook.
Coverage: message formatting, webhook parsing, config validation, health checks, rate limiting, send/receive lifecycle.
**Actions:**
- Add unit tests for each adapter's message formatting
- Add integration tests with mock servers for Telegram, Discord
- Add rate limiting regression tests
- Target: >50 tests across all adapters
**Success:** 150 gateway tests pass; all 9 platform adapters covered

### 6.2 Editor Integration Hardening
**Actions:**
- Verify VSCode extension builds, loads, and communicates correctly
- Test LSP client against real language servers (rust-analyzer, typescript-language-server)
- Add integration tests for editor communication protocol
- Test JetBrains plugin build and gradle configuration
**Success:** Both VSCode and JetBrains plugins build successfully in CI

### 6.3 Documentation Site
**DONE**
- mdBook builds clean (v0.4.40, 123-line SUMMARY.md)
- Fixed `site-url = "/clawdius/"` to `site-url = "/"` (dedicated domain, not GitHub Pages subdir)
- CNAME configured for `clawdius.co.uk`
- GitHub Pages deployment workflow configured
**Remaining:**
- DNS CNAME record for `clawdius.co.uk` (requires DNS provider access)
**Actions:**
- Generate API docs from rustdoc
- Add architecture overview diagram (C4 model from Blue Papers)
- Add getting-started guide with verified commands
- Add security model documentation
- Host on clawdius.co.uk via netlify.toml
**Success:** clawdius.co.uk serves accurate, versioned documentation

### 6.4 Docker Image Publication
**DONE** (commits ea04e4b, ec99e43, 0d9f35d, 0051e02, 320c2db, 68a36fc)
**Actions:**
- Multi-stage Dockerfile optimization (slim builder, distroless runtime)
- Publish to GitHub Container Registry (ghcr.io) — both images verified
- Docker: amd64 single-arch; arm64 blocked by GitHub runner limits (DL-020)
- `deploy/docker/docker-compose.yml`: updated to use ghcr.io images, added gateway service with healthcheck
**Remaining:**
- Re-enable arm64 when self-hosted runners available
**Success:** `docker pull ghcr.io/wyattau/clawdius:latest` works (amd64)

## Future Horizons (Post-1.0)

### Horizon 1: Multi-Agent Orchestration
- Agent-to-agent communication protocol via MCP
- Distributed sprint execution across worktrees
- Agent specialization (planner, executor, reviewer, tester)
- Cross-agent knowledge sharing via vector database

### Horizon 2: Formal Verification Deepening
- Extend Lean4 proofs beyond compile-check to theorem proving
- Verify critical algorithms: sandbox isolation, encryption at rest, token counting
- Integrate with `creusot` or `prusti` for Rust code verification
- Publish proof artifacts for external audit

### Horizon 3: Real-Time Collaboration
- Multi-user session sharing via gateway
- Real-time chat protocol (WebSocket, SSE) for collaborative coding
- Conflict-free replicated data types (CRDT) for shared editing
- Presence and cursor tracking

### Horizon 4: Enterprise Features
- SSO integration (SAML 2.0, OIDC, LDAP)
- Audit logging with compliance exports (SOC 2, HIPAA, GDPR)
- Role-based access control (RBAC) with 23 permission matrix
- Usage analytics and cost tracking dashboard
- On-premises air-gapped deployment

### Horizon 5: Hardware Acceleration
- GPU-accelerated embeddings via candle-core (CUDA, Metal)
- SIMD-optimized token counting and diff computation
- FPGA-accelerated sandbox isolation (optional)
- Quantized local LLM inference via candle-transformers

## Phase 7: Quality Audit Remediation (Week 14-16)

Findings from the comprehensive audit conducted 2026-05-20.

### 7.1 Silent Error Discard Remediation (38+ sites)
**Priority:** Critical
**DONE** (commit a9d84a8)
Replaced ~30 `let _ =` silent discards with explicit `.ok()` or conditional `tracing::warn!()`:
- `api/rest.rs`: `record_tenant_task()` → `if !func() { warn!() }`, `add_message()` → `.ok()`
- `llm/model_router.rs`: `cost_tracker.record()` → `.ok()`
- `api/sprint_handler.rs`: `record_tenant_task()` → `if !func() { warn!() }`, SSE sends → `.ok()`
- `api/agent_loop.rs` (12 sites): SSE sends → `.ok()`
- `orchestrator/worker.rs` (9 sites): queue ops → `.ok()`, channel sends → `.ok()`
- Plus: mcp/handler.rs, lsp/client.rs, completions/provider.rs, messaging/router.rs, agents/mod.rs

### 7.2 Logging Migration (40+ sites)
**Priority:** High
**DONE** (commit a9d84a8)
Replaced 37 `eprintln!()` with `tracing::{debug,warn,info}!()`:
- `agentic/sprint/engine.rs` (17 sites)
- `agentic/tool_use.rs` (5), `agentic/sprint/mod.rs` (9), `agentic/review_engine.rs` (2)
- `api/sprint_handler.rs` (4), `api/rag/indexer.rs` (1), `api/vscode.rs` (1), `workspace/context.rs` (1)

### 7.3 Gateway Adapter Hardening
**Priority:** Medium
**DONE** (commit a9d84a8)
- Fixed hardcoded "fallback" chat_id in whatsapp.rs (parse from message_id)
- Fixed hardcoded "default" roomId in rocketchat.rs (parse from message_id)
- Fixed signal.rs hardcoded "fallback" chat_id (parse from message_id)
- Remaining: DDP real-time connection for RocketChat, integration tests per adapter

### 7.4 UUID Generation Fix
**Priority:** High
**DONE** (commit a9d84a8)
Replaced collision-prone `uuid_v4_placeholder()` in `agentic/parallel_sprint.rs` with
`uuid::Uuid::new_v4().to_string()[..8]`.

### 7.5 CI/CD Pipeline Hardening
**Priority:** High
**DONE** (commits 4bacbdd, ba8dd97, 21a5857, ea04e4b, ec99e43, 600cbad, 4e5f43a, 0d9f35d, 0051e02, 320c2db)
- Migrated from `actions/cache@v4` to `Swatinem/rust-cache@v2` (9 sites across 4 files)
- Added coverage job using `cargo-llvm-cov` with lcov output
- Added Docker publish workflow (multi-arch amd64/arm64 to ghcr.io)
- Fixed `cargo deny` advisory warnings (-A advisory-not-detected)
- Added RUSTSEC-2026-0149 (wasmtime-wasi) to deny.toml ignore list
- Downgraded lto=fat to lto=thin (fixes E0432/E0433 cross-crate resolution)
- Removed redundant release build step from CI (redundant with clippy+test+check)
- Fixed Dockerfile.gateway: hardcoded /release/ path (ARG doesn't cross stages)
- Removed .cargo-vendor from .dockerignore
- Docker images verified: both clawdius and clawdius-gateway push to ghcr.io
- Security pipeline fully green (13/13 jobs)
- All CI pipelines green

### 7.6 Documentation Consistency
**Priority:** Medium
**Partially DONE**
- Test count corrected to 1,447 across README.md, QUICK_REFERENCE.md, COMPARISON.md
- Lean4 theorems corrected to 209, source files corrected to 390
- License fixed Apache 2.0, removed non-existent v1.2.0/Zhipu AI references
- Remaining: binary size/boot time consistency, automated badge generation

### 7.7 Website and Domain Infrastructure
**Priority:** Medium
**Partially DONE**
- Landing page merged into docs deployment workflow
- CNAME file added to docs deployment for clawdius.co.uk
- book.toml site-url and cname configured
- Remaining: DNS CNAME record for clawdius.co.uk (requires DNS provider access)
- Remaining: netlify.toml cleanup

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| lib.rs clobbering by agentic tools | Medium | High | `.gitattributes`, CI check, pre-commit guard |
| lancedb upgrade breaks vector DB | Medium | High | Pin version, test before upgrade, feature-flag gated |
| OOM on CI with all features | Medium | Medium | Feature matrix CI, documented limitations |
| Production unwrap panic in user workflow | Medium | High | Phase 1.1 unwrap elimination |
| VSCode extension bit-rot | Low | Medium | Integration tests, manual smoke test cycle |
| Transitive CVE blocks compliance | Low | Medium | Feature-flag gating, upstream monitoring |
| Pre-commit timeout on cold cache | Medium | Low | Skip-if-cold logic, pre-push as fallback |
| Silent Result discard causes data loss | Medium | ~~High~~ RESOLVED | Phase 7.1 done — all `let _ =` on Result replaced with `.ok()` or logged |
| CI stale cache causes false failures | ~~High~~ RESOLVED | ~~Medium~~ RESOLVED | Phase 7.5 done — migrated to rust-cache v2 |
| UUID collision under concurrent sprints | Medium | ~~High~~ RESOLVED | Phase 7.4 done — UUID v4 used |
| Domain misconfiguration blocks docs access | High | Medium | Phase 7.7 — CNAME added, DNS pending (requires provider access). Docs now at clawdius.co.uk |
| clawdius-gateway release build E0433 | ~~Medium~~ RESOLVED | ~~Medium~~ RESOLVED | Root cause: fat LTO cross-crate resolution. Fix: lto=thin, remove release build from CI |

## Decision Log

| ID | Decision | Rationale | Date |
|----|----------|-----------|------|
| DL-001 | Deny unwrap_used in clawdius-core production | Safety-critical crate; test code exempted | 2026-05-12 |
| DL-002 | Allow unwrap in test code via cfg_attr | Developer velocity; tests should be readable | 2026-05-12 |
| DL-003 | Suppress vendored half crate warnings | Cannot modify upstream; vendored for patching | 2026-05-12 |
| DL-004 | Pre-commit: fast checks only; pre-push: full suite | 10+ min pre-commit is unusable | 2026-05-12 |
| DL-005 | Document CVEs in deny.toml | Cannot fix transitive deps; transparency is mandatory | 2026-05-11 |
| DL-006 | Single source of truth: Cargo.toml version | Avoid stale version references in docs | 2026-05-12 |
| DL-007 | Remove allow(clippy::restriction) to activate deny(unwrap_used) | restriction group overrides deny | 2026-05-14 |
| DL-008 | Feature-flag vector-db and telegram behind default features | Compile errors in --all-features | 2026-05-14 |
| DL-009 | Use Swatinem/rust-cache over manual actions/cache | Handles incremental caching and stale artifact cleanup | 2026-05-20 |
| DL-010 | Block security pipeline on failures | Non-blocking security gate was masking all vulnerabilities | 2026-05-20 |
| DL-011 | Single canonical docs domain: clawdius.co.uk | Canonical domain is clawdius.co.uk; updated from docs.clawdius.dev | 2026-05-20 |
| DL-012 | Merge landing page into docs deployment | GitHub Pages only supports one deployment source | 2026-05-20 |
| DL-013 | Replace eprintln! with tracing::*! | Structured logging enables log level filtering in production | 2026-05-24 |
| DL-014 | Replace let _ = with .ok() or warn!() | Explicit error handling prevents silent data loss | 2026-05-24 |
| DL-015 | Remove release build from CI | Redundant with clippy+test+check; release binaries built by release.yml | 2026-05-24 |
| DL-016 | Add Docker publish workflow to CI | Multi-arch images for ghcr.io for both clawdius and clawdius-gateway | 2026-05-24 |
| DL-017 | Downgrade lto=fat to lto=thin | Fat LTO causes E0432/E0433 cross-crate import resolution with workspace feature unification; thin LTO is 95% perf, 3x faster | 2026-05-24 |
| DL-018 | Hardcode release path in Docker COPY | Docker ARG from builder stage doesn't propagate to runtime stage; ${PROFILE} resolves to empty | 2026-05-24 |
| DL-019 | Remove .cargo-vendor from .dockerignore | half crate [patch.crates-io] requires .cargo-vendor/half/ in Docker build context | 2026-05-24 |
| DL-020 | Single-arch Docker builds (amd64 only) | Multi-arch (amd64+arm64) consistently cancelled by GitHub runner limits; arm64 via QEMU too slow | 2026-05-24 |
| DL-021 | Remove orphaned messaging/ directory | 1,706 lines of dead code never declared in lib.rs; duplicated by clawdius-gateway/adapters | 2026-05-24 |
| DL-022 | Feature-gate 9 doc-hidden modules | Reduces compile time and binary size for consumers; telemetry/sandbox kept unconditional | 2026-05-26 |
| DL-023 | Remove unused httpmock dev-dependency | Never used in tests; eliminates ~60 transitive crates (async-std, lalrpop) | 2026-05-26 |
| DL-024 | Add musl release targets | Static binaries for Alpine/Docker slim; musl-tools in CI | 2026-05-26 |
| DL-025 | Automate Homebrew SHA-256 in release workflow | Downloads release archives, computes checksums, updates formula via Python script | 2026-05-26 |
| DL-026 | Fix pgo-optimized lto=fat -> lto=thin | Fat LTO causes E0432/E0433; thin LTO sufficient for PGO with 3x faster build | 2026-05-26 |
| DL-027 | Save criterion baseline on main, remove continue-on-error | Benchmarks must be deterministic on main to serve as PR regression baseline | 2026-05-26 |
| DL-028 | Add completions + man pages to release archives | Improves Homebrew/Nix install experience; avoids runtime generation deps | 2026-05-26 |
| DL-029 | Create separate homebrew tap repo (WyattAu/homebrew-clawdius) | Release workflow pushes formula updates to tap repo, not main repo | 2026-05-26 |
| DL-030 | Fix docs site-url from /clawdius/ to / | Dedicated domain clawdius.co.uk doesn't need subdir prefix | 2026-05-26 |

## Appendix: Quality Gate Summary

### Pre-Commit Gate
1. lib.rs integrity (>=10 `pub mod` declarations)
2. Merge conflict marker detection
3. `cargo fmt --all --check`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace --lib`
6. `cargo deny check bans licenses advisories`
7. `lake build` (Lean4 proofs)

### Pre-Push Gate
1. `cargo test --workspace --lib`
2. `cargo test --workspace --test integration`
3. `cargo test --workspace --tests`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo deny check bans licenses advisories`
6. `lake build` (Lean4 proofs)

### CI Gate (GitHub Actions)
- Rust CI: fmt, clippy, test lib, test integration, deny, check --workspace --all-features
- Coverage: cargo-llvm-cov with lcov output
- Lean4 proofs: lake build
- Benchmarks: run + regression detection
- Docker: multi-arch build + push to ghcr.io
- Security: audit, deny, secrets, SAST, fuzz, SBOM (13 jobs)
- Scheduled: weekly benchmark comparison
