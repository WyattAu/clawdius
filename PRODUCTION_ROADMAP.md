# Clawdius Production Roadmap

> Current: v1.0.0-rc.2 | Generated: 2026-05-20 | Supersedes: ROADMAP.md, ROADMAP_PATH_FORWARD.md

## Current State (Verified)

| Metric | Value | Status |
|--------|-------|--------|
| Workspace crates | 5 | Builds clean |
| Rust files | 344 | All compile |
| Lib tests | 1,284 | 0 failures |
| Integration tests | 158 | 0 failures |
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
- Strip decorative emoji from docs/book/src/intro.md (🛡️, ⚡, 🔧)
- Convert ✅/❌ to "Yes"/"No" or "Supported"/"Not Supported" in all comparison tables
- Consolidate duplicate documents: ROADMAP.md + ROADMAP_PATH_FORWARD.md -> this file
- Archive PATH_FORWARD.md (v0.5.0 era) to .reports/archived/
- Standardize version references to single source: Cargo.toml version field
**Success:** `rg '🚀|✅|❌|🛡️|⚡|🔧|🎉' docs/ .docs/ --count | wc -l` == 0

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
**Actions:**
- Add `.gitattributes` merge strategy: `crates/*/src/lib.rs merge=union`
- Add CI check: fail if any lib.rs has <10 `pub mod` declarations
- Consider splitting into `mod.rs` directory tree
- Add CLAWDIUS_PROTECT_LIBRS env var to scripts
**Success:** CI catches truncated lib.rs; no observed clobber after 2 weeks

### 3.2 Dependency Tree Simplification
**DONE**
Audit complete; report at .reports/dependency_audit.md
**Actions:**
- Audit 20+ transitive version duplicates
- Where semver-compatible: use `[patch.crates-io]` to unify
- Where semver-incompatible: document in Cargo.toml with justification
- Evaluate removing unused optional dependencies (slack-morphism, matrix-sdk if not built)
**Success:** <10 documented duplicate versions

### 3.3 Feature Flag Matrix
**DONE**
Matrix complete; report at .reports/feature_flag_matrix.md
Note: `--all-features` fails on `vector-db` and `telegram` features
**Actions:**
- Map all feature flags across 5 crates
- Identify conflicting combinations
- Add CI matrix job: test key combinations (default, all-platforms, browser, embeddings)
- Document supported feature sets in `FEATURES.md`
**Success:** No OOM from `--all-features` or clearly documented limitations

## Phase 4: Performance Engineering (Week 7-9)

### 4.1 PGO (Profile-Guided Optimization) Pipeline
**Actions:**
- Add CI job: build with `pgo-instrument` profile
- Run representative workload (file analysis, LLM streaming, session management)
- Build with `pgo-optimized` profile using collected profiling data
- Benchmark before/after: latency, throughput, binary size
**Success:** >10% latency reduction on hot paths vs baseline

### 4.2 Benchmark Regression Detection
**Actions:**
- Wire `criterion` benchmarks to CI with `--save-baseline`
- Establish baseline metrics in `.specs/06_5_regression/baseline_metrics.toml`
- Add CI step: compare against baseline, fail if >5% regression
- Add per-benchmark thresholds to `BENCHMARKS.md`
**Success:** Automated alerts on performance regressions

### 4.3 Memory Profiling
**Actions:**
- Add `dhat` or `valgrind massif` profiling to CI
- Identify allocation hotspots in hot paths (LLM streaming, diff computation)
- Target: <100MB RSS for typical CLI session; <200MB for gateway with 100 concurrent users
**Success:** Memory budget documented and enforced

### 4.4 Cold Start Optimization
**Actions:**
- Measure time-to-first-token across all LLM providers
- Pre-warm model configs, tokenizers
- Add benchmark for shell tool startup latency
**Success:** <500ms CLI startup; <2s first-token latency

## Phase 5: Release Preparation (Week 9-11)

### 5.1 crates.io Publishing
**Actions:**
- Verify all crate metadata (description, categories, keywords, license)
- Ensure no `path` dependencies leak to published crates
- Test dry-run: `cargo publish --dry-run` for each crate
- Set up CI for automated publish on v* git tag
**Success:** All 5 crates publishable to crates.io

### 5.2 API Stability Audit
**Actions:**
- Audit public API surface per crate
- Mark unstable APIs with `#[doc(hidden)]`
- Document stable API contract in module docs
- Add semver checks to CI (cargo-semver-checks)
**Success:** Explicit semver stability guarantees for all public types

### 5.3 Cross-Platform CI Matrix
**Actions:**
- Add CI matrix: Linux (x86_64, aarch64), macOS (aarch64), Windows (x86_64)
- Test sandbox backends per platform (bubblewrap = Linux only)
- Test feature combinations per platform
**Success:** CI green on all 4 platform targets

### 5.4 Installation Packaging
**Actions:**
- Verify `install.sh` works on all platforms
- Test Homebrew formula (`homebrew-clawdius.rb`)
- Add `cargo install clawdius` support
- Add `cargo-binstall` metadata
- Docker image: slim profile, multi-arch (amd64, arm64)
**Success:** Installation succeeds on all 4 platforms

## Phase 6: Ecosystem & Scale (Week 11-14)

### 6.1 Gateway Platform Adapters
**Actions:**
- Add unit tests for each adapter's message formatting
- Add integration tests with mock servers for Telegram, Discord
- Add rate limiting regression tests
- Target: >50 tests across all adapters
**Success:** Gateway test suite covers all 9 platform adapters

### 6.2 Editor Integration Hardening
**Actions:**
- Verify VSCode extension builds, loads, and communicates correctly
- Test LSP client against real language servers (rust-analyzer, typescript-language-server)
- Add integration tests for editor communication protocol
- Test JetBrains plugin build and gradle configuration
**Success:** Both VSCode and JetBrains plugins build successfully in CI

### 6.3 Documentation Site
**Actions:**
- Generate API docs from rustdoc
- Add architecture overview diagram (C4 model from Blue Papers)
- Add getting-started guide with verified commands
- Add security model documentation
- Host on docs.clawdius.dev via netlify.toml
**Success:** docs.clawdius.dev serves accurate, versioned documentation

### 6.4 Docker Image Publication
**Actions:**
- Multi-stage Dockerfile optimization (slim builder, distroless runtime)
- Publish to GitHub Container Registry (ghcr.io)
- Add Docker Compose example for gateway + CLI
- Add Kubernetes deployment example
**Success:** `docker pull ghcr.io/clawdius/clawdius:1.0.0` works

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
**Actions:**
- Replace `let _ =` with proper error handling in API persistence code:
  - `api/rest.rs`: `record_tenant_task()`, `state.add_message()` (6 sites)
  - `llm/model_router.rs`: `cost_tracker.record()` (3 sites)
  - `api/sprint_handler.rs`: `record_tenant_task()` (1 site)
- Replace `let _ =` channel sends with `.ok()` or logged errors in:
  - `api/agent_loop.rs` (12 sites), `api/sprint_handler.rs` (8 sites)
  - `orchestrator/worker.rs` (9 sites)
- Use `tracing::warn!()` for non-critical drops, `?` for critical paths
**Success:** Zero `let _ =` on Result types in `api/`, `llm/`, `messaging/` modules

### 7.2 Logging Migration (40+ sites)
**Priority:** High
**Actions:**
- Replace `eprintln!()` with `tracing::debug!()`/`tracing::warn!()` in:
  - `agentic/sprint/engine.rs` (18 instances)
  - `agentic/tool_use.rs` (5 instances)
  - `agentic/sprint/mod.rs` (8 instances)
  - `agentic/review_engine.rs` (2 instances)
  - `api/sprint_handler.rs` (4 instances)
**Success:** Zero `eprintln!()` in library code

### 7.3 Gateway Adapter Hardening
**Priority:** Medium
**Actions:**
- Fix hardcoded "fallback" chat_id in signal.rs, whatsapp.rs edit_message
- Fix hardcoded "default" roomId in rocketchat.rs
- Implement DDP real-time connection for RocketChat `start()`
- Add integration tests per adapter with mock servers
**Success:** All 9 adapters pass message roundtrip tests

### 7.4 UUID Generation Fix
**Priority:** High
**Actions:**
- Replace `uuid_v4_placeholder()` in `agentic/parallel_sprint.rs` with proper
  `uuid::Uuid::new_v4()` calls
- The current timestamp-based ID generation is collision-prone under concurrency
**Success:** All IDs use cryptographically-sound UUID v4

### 7.5 CI/CD Pipeline Hardening
**Priority:** High
**Actions:**
- Migrate from `actions/cache@v4` to `Swatinem/rust-cache@v2` for smarter caching
  (resolves stale cache compilation failures)
- Pin all actions to commit SHAs (supply chain security)
- Remove redundant `lean_action_ci.yml` (duplicates `ci.yml` lean4-proofs job)
- Remove duplicate benchmark execution between `ci.yml` and `benchmarks.yml`
- Make security pipeline blocking (remove `continue-on-error` from audit/vet)
- Add GPG signing validation to release workflow
**Success:** CI passes consistently on all platforms; security failures block merges

### 7.6 Documentation Consistency
**Priority:** Medium
**Actions:**
- Establish single test count source: generate badge from `cargo test -- --list`
- Fix binary size references (currently 4 different values across docs)
- Fix boot time references (currently 3 different values across docs)
- Remove all references to non-existent v1.2.0
- Remove references to non-existent Zhipu AI and Groq providers
- Add DNS records for docs.clawdius.dev pointing to GitHub Pages
**Success:** All metrics consistent across all documentation files

### 7.7 Website and Domain Infrastructure
**Priority:** Medium
**Actions:**
- Configure docs.clawdius.dev DNS (CNAME to wyattau.github.io)
- Fix or decommission docs.clawdius.co.uk (currently returns 500)
- Remove or fix netlify.toml if not actively using Netlify
- Create dedicated landing page deployment (currently only docs deploy)
**Success:** docs.clawdius.dev serves documentation; landing page deployed

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
| Silent Result discard causes data loss | Medium | High | Phase 7.1 remediation of 38+ `let _ =` sites |
| CI stale cache causes false failures | High | Medium | Migrate to rust-cache v2 (DL-009) |
| UUID collision under concurrent sprints | Medium | High | Phase 7.4 fix timestamp-based IDs |
| Domain misconfiguration blocks docs access | High | Medium | Phase 7.7 DNS alignment |

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
| DL-011 | Single canonical docs domain: docs.clawdius.dev | Eliminates .co.uk/.dev mismatch; aligned with landing page | 2026-05-20 |
| DL-012 | Merge landing page into docs deployment | GitHub Pages only supports one deployment source | 2026-05-20 |

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
- Rust CI: fmt, clippy, test lib, test integration, deny, build release
- Lean4 proofs: lake build
- Benchmarks: run + regression detection
- Scheduled: weekly benchmark comparison
