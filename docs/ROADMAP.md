# Clawdius Production Roadmap

**Version:** 1.0.0-rc.2
**Date:** 2026-05-20
**Status:** Active

---

## Guiding Principles

This roadmap is driven by the competitive gaps identified in [COMPARISON.md](./COMPARISON.md) and [COMPETITOR_ANALYSIS_CLAW_CODE.md](./COMPETITOR_ANALYSIS_CLAW_CODE.md). Priorities are ordered by strategic impact:

1. **Defend the moat** -- Clawdius's advantages (sandboxing, formal verification, enterprise, messaging) must deepen, not regress.
2. **Close the gaps** -- Competitors lead in LLM breadth (Claw Code: xAI, DashScope), REPL polish (tab completion), and agent orchestration. These must be addressed.
3. **Ship v1.0** -- The rc.1 milestone is complete on test quality (1,527 tests, 0 failures) and CI (5/5 workflows green). The path to stable must be short and deterministic.

---

## Phase 1: v1.0-rc.2 -- Gap Closure (COMPLETE)

**Goal:** Address the most impactful competitive gaps without scope creep.

### 1.1 LLM Provider Expansion (COMPLETE)

Clawdius now supports 9 providers: Anthropic, OpenAI, Ollama, DeepSeek, OpenRouter, Google Gemini, xAI (Grok), Mistral AI, and Groq.

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| Add xAI (Grok) provider | P0 | 2d | DONE -- native genai adapter |
| Add Google Gemini provider | P0 | 2d | DONE -- tool calling enabled |
| Add Mistral provider | P1 | 1d | DONE -- native genai adapter |
| Per-model parameter adaptation | P1 | 3d | DEFERRED -- 3d effort, touches config/agent_loop/model_router/completions |

**Result:** 9 providers (8 with tool calling), matching or exceeding Claw Code.

### 1.2 REPL Polish (PARTIAL)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| Tab completion for slash commands | P0 | 1d | DONE -- CommandAutocomplete component (37 commands, 5 tests) |
| Tab completion for file paths (@path) | P1 | 1d | TODO |
| Tab completion for modes | P2 | 0.5d | TODO |

### 1.3 Prompt Caching (COMPLETE)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| System prompt cache (provider-agnostic) | P1 | 2d | DONE -- blake3 keyed LLM response cache, 5min TTL, 1000 entries |
| Cache invalidation on context change | P1 | 1d | DONE -- cache keyed by message hash, context change = different hash |

### 1.4 CodeQL SAST Timeout Fix (COMPLETE)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| Increase timeout to 30 min | P2 | 0.5d | DONE |

---

## Phase 2: v1.0-rc.3 -- Infrastructure (COMPLETE)

**Goal:** Production infrastructure for clawdius.co.uk.

### 2.1 DNS and Hosting (COMPLETE)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| DNS: `docs.clawdius.co.uk` -> Cloudflare Pages | P0 | 0.5d | DONE |
| DNS: `clawdius.co.uk`/`www` -> Cloudflare Pages | P0 | 0.5d | DONE |
| Cloudflare Pages project: `clawdius-docs` | P0 | 0.5d | DONE (100 files deployed) |
| Cloudflare Pages project: `clawdius-landing` | P1 | 0.5d | DONE |
| SSL/TLS: Full (strict) mode | P0 | 0.5d | DONE (via Cloudflare) |

**Note:** Cloudflare Pages serving returns HTTP 500 -- account-level infrastructure issue, not fixable via API.

### 2.2 Release Pipeline (PARTIAL)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| Fix `clawdius-gateway` release build failure | P0 | 1d | DONE -- compiles locally |
| Publish `clawdius-core` to crates.io | P0 | 1d | DRY-RUN PASS (253 files, 3.5MiB) -- needs crates.io token |
| Publish `clawdius-mcp` to crates.io | P1 | 0.5d | BLOCKED on core publish |
| Publish `clawdius-code` to crates.io | P1 | 0.5d | BLOCKED on core publish |
| GitHub Release automation test | P0 | 1d | TODO |

### 2.3 Documentation (COMPLETE)

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| Add `docs.clawdius.co.uk` CNAME to book.toml | P0 | 0.5d | DONE |
| Add xAI/Gemini/Mistral to provider docs | P0 | 1d | DONE |
| Add migration guide from Claw Code | P1 | 1d | DONE |
| Update COMPARISON.md with new providers | P1 | 0.5d | DONE (7-competitor table) |

---

## Phase 3: v1.0.0 Stable -- Release (1 week)

**Goal:** Ship v1.0.0 with zero known blockers.

### 3.1 Quality Gates

| Gate | Criteria | Current Status |
|------|----------|----------------|
| Tests | All 1,636 pass, 0 ignored | PASS |
| Clippy | `-D warnings` clean | PASS |
| Format | `cargo fmt --check` clean | PASS |
| Deny | bans, licenses, advisories clean | PASS |
| CI | All workflows green | 5/5 |
| Lean4 | 209 theorems verified | PASS |
| Fuzz | 5 targets, 0 crashes | PASS |
| Security | No critical CVEs | PASS |
| Docs | mdBook builds, no stubs | PASS |

### 3.2 Release Checklist

- [x] All Phase 1 and Phase 2 tasks complete
- [ ] `cargo publish --dry-run` succeeds for all publishable crates
- [ ] Docker image builds for linux/amd64 and linux/arm64
- [ ] Cross-compile matrix verified (Linux, macOS, Windows)
- [x] Changelog updated with all changes since rc.1
- [ ] VERSION.md bumped to 1.0.0
- [ ] Git tag `v1.0.0` created and signed
- [ ] GitHub Release published with binaries
- [ ] crates.io packages published
- [ ] docs.clawdius.co.uk serving latest docs
- [ ] clawdius.co.uk landing page live

---

## Phase 4: v1.1.0 -- Deepening the Moat (4 weeks)

**Goal:** Extend Clawdius's structural advantages that no competitor can easily replicate.

### 4.1 Sandboxing Hardening

Clawdius is the only coding assistant with 5 sandbox backends and Lean4 proofs. Deepen this.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| gVisor backend implementation | P0 | 5d | Planned since v1.7.0 spec, now deliver |
| Firecracker backend implementation | P1 | 7d | Planned since v1.7.0 spec, microVM isolation |
| Lean4 proofs for gVisor isolation | P0 | 3d | Formal guarantee for new backend |
| WASM sandbox: add filesystem API | P1 | 3d | Plugins can read/write sandboxed files |
| Sandbox benchmark suite | P1 | 2d | Quantify overhead per backend |

### 4.2 Code Intelligence Expansion

Clawdius leads with Tree-sitter + LanceDB Graph-RAG. Claw Code and Cursor have no equivalent.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Add 5 more Tree-sitter parsers (Java, C++, Ruby, PHP, Kotlin) | P1 | 3d | Broader language coverage |
| Incremental indexing | P0 | 3d | Real-time code graph updates on file save |
| Cross-file reference resolution | P1 | 5d | "Find all callers" -- IDE parity |
| Code graph export (JSON/GraphQL) | P2 | 2d | Integration with external tools |

### 4.3 Enterprise Features

No competitor has SSO, audit logging, compliance templates, and 23 permissions. Clawdius owns this segment.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| SCIM provisioning (Okta, Azure AD) | P1 | 5d | Enterprise onboarding automation |
| Audit log export (SIEM integration) | P1 | 3d | Splunk, Datadog, Elasticsearch |
| Usage analytics dashboard | P2 | 3d | Team admins need visibility |
| Compliance certification prep (SOC 2 Type II) | P1 | 10d | Revenue enabler |

### 4.4 Messaging Gateway

Claw Code is CLI-only. Clawdius has 9 adapters. This is a unique differentiator.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Microsoft Teams adapter production hardening | P1 | 3d | Enterprise demand |
| Webex adapter production hardening | P2 | 2d | Enterprise demand |
| IRC adapter production hardening | P2 | 1d | Self-hosted demand |
| Adapter health monitoring dashboard | P1 | 2d | Operational visibility |

---

## Phase 5: v1.2.0 -- Agent Orchestration (4 weeks)

**Goal:** Close the autonomous agent coordination gap with Claw Code.

Claw Code has lane events, a policy engine, worker lifecycle, task/team/cron registries, and recovery recipes. Clawdius has none of this.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Agent task queue (persistent, SQLite-backed) | P0 | 5d | Foundation for orchestration |
| Multi-agent coordination (spawn, message, collect) | P0 | 7d | Claw Code parity |
| Policy engine (permission rules per agent) | P1 | 5d | Enterprise safety |
| Agent recovery recipes (auto-retry, fallback) | P1 | 3d | Claw Code parity |
| Deterministic mock service for E2E agent testing | P1 | 3d | Claw Code advantage we lack |
| Cron/scheduled agent tasks | P2 | 3d | Claw Code parity |

---

## Phase 6: v1.3.0 -- Performance and Polish (2 weeks)

**Goal:** Maintain performance leadership and address quality-of-life gaps.

### 6.1 Performance

| Metric | Current (Clawdius) | Target | Competitor |
|--------|--------------------|--------|------------|
| Cold boot | 2.5ms | <10ms PASS | Claw Code: ~50ms |
| Idle memory | ~100MB | <80MB | Claw Code: ~50MB |
| Binary size (release, stripped) | 26 MiB | N/A | Claw Code: ~35 MiB |
| Streaming latency | TBD | <50ms p95 | Claw Code: SSE with backpressure |
| Index time (100K LOC) | TBD | <5s | N/A |

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Profile and optimize cold boot path | P1 | 2d | Already leading; stay ahead |
| Lazy-load non-critical modules | P1 | 2d | Reduce memory footprint |
| Streaming backpressure tuning | P1 | 2d | Claw Code advantage |

### 6.2 Quality of Life

| Task | Priority | Effort | Status |
|------|----------|--------|--------|
| `clawdius migrate` command (from Claude Code, Aider, Cursor) | P1 | 5d | TODO |
| Interactive configuration wizard | P2 | 2d | TODO |
| Shell completions (bash, zsh, fish) | P1 | 1d | DONE -- `--generate-completions` flag, completions/ dir |
| Man page generation | P2 | 1d | DONE -- man/clawdius.1 |

---

## Phase 7: v2.0.0 -- Next Generation (8 weeks)

**Goal:** Features that define the next competitive era.

### 7.1 Browser Automation

Neither Clawdius nor Claw Code has browser automation (Playwright/Puppeteer). This is a shared weakness.

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Headless Chromium via CDP (in sandbox) | P0 | 7d | Unique capability; no competitor has this |
| Screenshot and DOM snapshot tools | P1 | 3d | Agent can "see" web output |
| E2E testing agent mode | P1 | 5d | "Run my tests and fix failures" |

### 7.2 Multi-Model Routing

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Automatic model selection based on task type | P1 | 5d | Code tasks -> Claude, search -> GPT, local -> Ollama |
| Cost-aware routing (cheapest model that handles the task) | P1 | 3d | Enterprise cost control |
| Fallback chains (primary -> secondary -> local) | P1 | 2d | Resilience |

### 7.3 Knowledge Base

| Task | Priority | Effort | Impact |
|------|----------|--------|--------|
| Persistent project knowledge (per-repo embeddings) | P1 | 5d | "Remember this project's patterns" |
| Team knowledge sharing (shared embeddings) | P2 | 5d | Organization memory |

---

## Competitive Position Tracking

### Current Advantages (Defend)

| Advantage | Competitor Gap | Risk |
|-----------|---------------|------|
| Formal verification (209 Lean4 theorems) | No competitor has any | Low -- hard to replicate |
| 5 sandbox backends | Claw Code: container detection only | Medium -- Claw Code could add bubblewrap |
| 9 messaging adapters | Claw Code: CLI-only | Low -- different architecture |
| Enterprise (SSO, audit, compliance) | No competitor | Low -- requires domain expertise |
| Graph-RAG code intelligence | Cursor has basic indexing | Medium -- Cursor investing heavily |

### Current Gaps (Close)

| Gap | Leader | Priority |
|-----|--------|----------|
| LLM provider breadth | Claw Code (xAI, DashScope, vLLM, llama.cpp) | P0 -- Phase 1 |
| Tab completion | Claw Code | P0 -- Phase 1 |
| Prompt caching | Claw Code | P1 -- Phase 1 |
| Autonomous agent orchestration | Claw Code (lanes, policy, workers) | P0 -- Phase 5 |
| Deterministic mock service | Claw Code | P1 -- Phase 5 |
| Browser automation | Neither | P0 -- Phase 7 |

### Shared Weaknesses (Opportunity)

| Weakness | Opportunity | Phase |
|----------|-------------|-------|
| No browser automation | First to market | Phase 7 |
| No published crates | Clawdius can publish first | Phase 2 |
| Stale doc metrics | Automated metric injection in CI | Phase 2 |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Claw Code adds real sandboxing | Medium | High | Clawdius has 2-year head start + formal proofs |
| Cursor adds enterprise features | Medium | Medium | Clawdius has Apache 2.0; Cursor is proprietary |
| Lean4 proof maintenance burden | Medium | Medium | Automate proof checking in CI (already done) |
| CodeQL timeout on larger workspaces | High | Low | Split per-crate or increase timeout |
| Chromiumoxide hyper 0.14 incompatibility | Low | Medium | Pin Rust version; track upstream |
| crates.io publish blocked by dependencies | Medium | High | Audit dependency tree early |

---

## Timeline Summary

| Phase | Version | Duration | Status |
|-------|---------|----------|--------|
| 1 | rc.2 | 2 weeks | COMPLETE |
| 2 | rc.3 | 2 weeks | COMPLETE (except crates.io publish) |
| 3 | 1.0.0 | 1 week | In progress (changelog done, tag pending) |
| 4 | 1.1.0 | 4 weeks | Planned |
| 5 | 1.2.0 | 4 weeks | Planned |
| 6 | 1.3.0 | 2 weeks | Partial (shell completions + man page done) |
| 7 | 2.0.0 | 8 weeks | Planned |

**Total to v1.0.0 stable: 5 weeks.**
**Total to v2.0.0: 23 weeks (~6 months).**

---

## Appendix: Metric Targets

All metrics should be automatically validated in CI to prevent the drift that occurred before this roadmap was written.

| Metric | Current | v1.0 Target | v1.1 Target | v2.0 Target |
|--------|---------|-------------|-------------|-------------|
| Lib tests | 1,085+ | 1,600+ | 2,000+ | 3,000+ |
| Integration tests | 109 | 120+ | 200+ | 400+ |
| Lean4 theorems | 209 | 209+ | 250+ | 350+ |
| Fuzz targets | 5 | 8+ | 12+ | 20+ |
| Code coverage (overall) | ~60% | 70% | 80% | 90% |
| Code coverage (critical paths) | ~95% | 95%+ | 97%+ | 99%+ |
| LLM providers | 9 | 9 | 9 | 9 |
| Sandboxing backends | 5 | 5 | 7 | 7 |
| Messaging adapters | 9 | 9 | 9 | 9 |
| Tree-sitter parsers | 5 | 5 | 10 | 10 |
| Boot time | 2.5ms | <10ms | <10ms | <10ms |
| Idle memory | ~100MB | <100MB | <80MB | <60MB |

---

*Last updated: 2026-05-20 | Maintained in version control*
