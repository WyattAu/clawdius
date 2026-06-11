# Clawdius: Roadmap to Production and Beyond

> Version: 1.0.0-rc.2 | Date: 2026-05-27 | Author: Architecture Audit

## Executive Summary

Clawdius is a Rust-native, formally verified AI coding engine with 5 workspace crates, 2,019
tests (0 failures), 209 Lean4 theorems, and a multi-platform messaging gateway. This document
covers the path from current rc.2 to stable v1.0.0 release and the post-production future.

---

## Current State (Empirically Verified)

| Metric | Value |
|--------|-------|
| Version | 1.0.0-rc.2 |
| Workspace crates | 5 (clawdius, core, gateway, mcp, code) |
| Total tests | 2,019 (1,425 lib + 232 integration + 27 property + 335 adapter) |
| Test failures | 0 |
| Lean4 proofs | 17 files, 209 theorems, 31/31 lake jobs |
| Clippy | Clean (-D warnings, pedantic, nursery) |
| cargo fmt | Clean workspace-wide |
| cargo deny | Clean (advisories, bans, licenses) |
| Rust version | 1.92+ (pinned) |
| Coverage | ~60% overall, 100% mcp/code, 64% core, 61% gateway, 6% CLI |
| Unsafe blocks | 4 (simd.rs SSE2/NEON only) |
| Production unwraps | 0 (deny active on core) |
| CI pipelines | 7 workflows (CI, security, lean4, benchmarks, docs, docker, release) |
| Git hooks | pre-commit (fmt, clippy, lib tests, deny, lean4) + pre-push (full suite) |
| Documentation | mdBook (123 pages), landing page, GitHub Pages deployed |
| Binary size | 26 MiB (release, stripped) |
| Cold start | 2.5 ms |

---

## Phase A: Release Candidate Finalization (Week 1-2)

### A.1 CLI Coverage Gap

**Problem:** CLI crate at 5.6% branch coverage. 25+ subcommands at 0% coverage.
**Priority:** High (user-facing surface)
**Actions:**
- Add integration tests for each CLI subcommand (chat, config, session, model, etc.)
- Test error paths (missing config, invalid provider, network timeout)
- Test shell completion generation (bash, zsh, fish)
- Target: >40% CLI coverage

### A.2 Transitive CVE Resolution

**Problem:** 2 tracked CVEs in matrix-sdk-base (blocked on upstream >= 0.11).
**Priority:** Medium
**Actions:**
- Poll matrix-sdk releases weekly
- If blocked >4 weeks: evaluate patch override or feature-gate removal
- For rustls-webpki: poll lancedb >= 0.28
- Document resolution timeline in deny.toml

### A.3 Gateway Crate Safety

**Problem:** clawdius-gateway allows clippy::unwrap_used and clippy::expect_used.
**Priority:** Medium
**Actions:**
- Audit all unwrap/expect calls in production code
- Replace with proper error handling (? operator or expect with invariant messages)
- Add crate-level deny for unwrap_used
- Target: 0 production unwraps

### A.4 Release Workflow Hardening

**Problem:** Homebrew tap push fails (needs PAT, not GITHUB_TOKEN). GPG signing
silently disabled. Clippy quality gate is warn-only.
**Priority:** High (blocks release)
**Actions:**
- Add HOMEWORK_TAP_TOKEN secret for Homebrew tap push
- Add GPG_PRIVATE_KEY secret or remove signing step
- Escalate release clippy to -D warnings (after A.3)
- Add cargo publish --dry-run before actual publish
- Fix shell completion archive structure (bash/zsh/fish distinct files)

### A.5 API Stability Commitment

**Problem:** No formal API stability guarantee. No MIGRATION.md.
**Priority:** High (pre-release requirement)
**Actions:**
- Define public API surface for each crate
- Add semver-checks to release gate (currently non-blocking)
- Create API_STABILITY.md with stability tiers
- Document breaking change policy

**Exit Criteria:** All A.x items resolved. Zero known blockers for v1.0.0.

---

## Phase B: Stable Release (Week 3)

### B.1 v1.0.0 Release

**Actions:**
- Tag v1.0.0
- Run full release workflow (7 platform targets, SBOM, GPG signatures)
- Publish 5 crates to crates.io (core first, then dependent crates)
- Update Homebrew tap
- Create GitHub release with checksums
- Publish announcement blog post

### B.2 Installation Verification

**Actions:**
- Verify cargo install clawdius works
- Verify Homebrew install works
- Verify Docker pull works
- Verify all platform binaries (linux-amd64, linux-arm64, macos-amd64, macos-arm64, windows)
- Test upgrade path from rc.2 to 1.0.0

### B.2 Documentation Finalization

**Actions:**
- Verify all 123 mdBook pages render correctly
- Verify landing page metrics match VERSION.md
- Test all internal links (SUMMARY.md -> actual files)
- Add "Getting Started" video or walkthrough
- Ensure CNAME (clawdius.co.uk) is configured

**Exit Criteria:** v1.0.0 published, installable, documented.

---

## Phase C: Post-Production Hardening (Week 4-6)

### C.1 Coverage Improvement

**Current:** ~60% overall. CLI at 6%.
**Target:** >80% overall, >95% critical paths.
**Actions:**
- CLI: integration tests for all 25+ subcommands
- Core: increase from 64% to >80% (focus on agentic, storage, sandbox modules)
- Gateway: increase from 61% to >80% (focus on adapter modules)
- Add coverage enforcement to CI (fail if coverage drops below baseline)
- Add coverage badge to README

### C.2 Performance Optimization

**Actions:**
- PGO (Profile-Guided Optimization): fix profiling workload in pgo.yml
  - Current workload is trivial (--help only). Use real benchmark suite.
  - Measure PGO vs non-PGO improvement
- Binary size optimization: investigate .cargo-vendor/half patch crate necessity
- Startup latency: profile and optimize cold start path
- Memory: profile heap usage under load

### C.3 Security Hardening

**Actions:**
- Make security gate meaningful (remove continue-on-error from audit/vet/fuzz)
- Add Docker image vulnerability scanning (Trivy) to docker.yml
- Fix composite action API key propagation (clawdius/action.yml)
- Add Dependabot or Renovate for automated dependency updates
- Evaluate FIPS 140-2 compliance for encryption modules

### C.4 Observability

**Actions:**
- Add structured logging to all crates
- Add metrics export (Prometheus format)
- Add health check endpoints to gateway
- Add request tracing (OpenTelemetry)
- Set up error reporting (Sentry integration, already feature-gated)

**Exit Criteria:** >80% coverage, PGO optimized, security gate enforced.

---

## Phase D: Feature Development (Week 7-12)

### D.1 Enhanced Agentic Capabilities

**Current:** Basic multi-agent orchestration with sprint execution.
**Planned:**
- Multi-file editing coherence (edit groups that must succeed/fail together)
- Agent-to-agent communication protocol
- Task decomposition with dependency graphs
- Verification agent (formal proof integration with Lean4)

### D.2 Plugin System

**Current:** Plugin SDK documented but stub.
**Planned:**
- Plugin API definition (Rust trait + FFI boundary)
- Plugin sandboxing (WASM-based isolation)
- Plugin marketplace infrastructure
- Example plugins (custom LLM provider, custom tool)

### D.3 Enterprise Features

**Current:** SSO, audit, compliance modules feature-gated.
**Planned:**
- Team management with RBAC
- Usage billing and metering (Stripe integration exists)
- Audit log export (JSON, SIEM-compatible)
- Compliance dashboard (SOC 2, GDPR readiness)

### D.4 Editor Integration Improvements

**Current:** VSCode extension (JSON-RPC), Vim/Neovim, Emacs documented.
**Planned:**
- VSCode extension: inline completion, code actions, diagnostics
- JetBrains plugin: build and publish
- LSP server: full Language Server Protocol compliance
- Zed editor support

### D.5 Multi-Modal Support

**Planned:**
- Image input support (vision models)
- File attachment handling
- Screenshot/paste integration in TUI

**Exit Criteria:** Plugin system live, agent capabilities enhanced, enterprise features available.

---

## Phase E: Ecosystem Growth (Month 4-6)

### E.1 Distribution

**Actions:**
- AUR package (Arch Linux)
- Nix flake in nixpkgs
- Chocolatey package (Windows)
- Snap package (Linux)
- Docker Hub official image
- Cloud marketplace listings

### E.2 Community Building

**Actions:**
- Discord server for community support
- Contribution guidelines refinement
- Bug bounty program
- Conference talks and blog posts
- Case studies and user testimonials

### E.3 Advanced Features

**Planned:**
- Real-time collaboration (multi-user sessions)
- Code review automation (PR summarization, review suggestions)
- Knowledge base integration (vector DB RAG with LanceDB)
- Custom model fine-tuning pipeline
- CI/CD integration (GitHub Actions, GitLab CI)

### E.4 Performance Targets

| Metric | Current | 6-Month Target |
|--------|---------|-----------------|
| Cold start | 2.5 ms | <2 ms |
| Binary size | 26 MiB | <20 MiB |
| Memory (idle) | 1.7 KiB | <1 KiB |
| Test count | 2,019 | >3,000 |
| Coverage | ~60% | >85% |
| Providers | 9 | >12 |
| Platform adapters | 9 | >12 |

**Exit Criteria:** Multiple distribution channels, active community, advanced features shipped.

---

## Phase F: Long-Term Vision (Month 6-12)

### F.1 Self-Hosted Cloud

- Cloud-native deployment (Kubernetes Helm chart)
- Horizontal scaling for gateway
- Multi-tenant isolation
- Custom model hosting integration

### F.2 Formal Verification Expansion

- Expand Lean4 proofs to cover more modules
- Property-based testing for all state machines
- Model checking for concurrent systems (TLA+/SPIN)
- Proof-carrying code for plugin sandbox

### F.3 AI-Native Development

- Code generation from natural language specifications
- Automated test generation from formal specs
- Architecture synthesis from requirements
- Self-improving code quality analysis

### F.4 Research Integration

- Multi-lingual knowledge graph (EN/ZH/RU/DE/FR/JP/KO)
- Cross-lingual code analysis
- Automated literature review integration
- Domain-specific AI assistants (embedded, safety-critical)

---

## Known Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Transitive CVE escalation | Medium | High | Weekly upstream polling, patch overrides |
| Lean4 proof breakage | Low | Medium | Pin toolchain, CI gate on lake build |
| Performance regression | Medium | Medium | Criterion baseline comparison in CI |
| API breaking changes | Medium | High | Semver checks, API stability tiers |
| Supply chain attack | Low | Critical | Pinned actions, cargo-deny, cargo-vet |
| Contributor burnout | Medium | Medium | Automated tooling, clear roadmap |

---

## Decision Log

| ID | Decision | Date | Status |
|----|----------|------|--------|
| RD-001 | Tokio selected over monoio for async runtime | 2026-05-27 | Final |
| RD-002 | GitHub Pages for docs (not Cloudflare) | 2026-05-27 | Final |
| RD-003 | PGO optimization deferred to post-release | 2026-05-27 | Active |
| RD-004 | Plugin system planned for Phase D | 2026-05-27 | Planned |
| RD-005 | Enterprise features behind feature flags | 2026-05-27 | Active |
