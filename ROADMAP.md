# Clawdius Roadmap
## Strategic Vision & Development Plan

**Current Version:** 1.3.0  
**Next:** v1.4.0 — Credibility & Foundations  
**Last Updated:** 2026-04-01

---

## Executive Summary

Clawdius has deep technical differentiation (formal verification, 7 sandbox backends, SEC 15c3-5 risk controls, 142 Lean4 theorems) but faces a **credibility gap**: three critical features claimed in the competitive analysis — agentic code generation, sandboxed test execution, and direct cargo test invocation — are stubs that return hardcoded values. The next release cycle must close this gap before investing in feature expansion.

### Current State (v1.3.0)

| Metric | Value |
|--------|-------|
| **Rust LOC** | 107,040 |
| **Tests** | 1,162 passing (1,091 unit + 43 property + 28 integration) |
| **Lean4 Proofs** | 142 theorems (138 proven, 4 HashMap-sorry, 39 axioms), 97.2% |
| **Clippy** | 0 warnings, `deny(unsafe_code)`, `deny(unwrap_used)` in config |
| **Panic-prone calls** | ~1,381 unwrap/expect in production code (`.clippy.toml` warns but CI doesn't enforce) |
| **Sandbox Backends** | 7 (WASM, Filtered, Bubblewrap, Sandbox-exec, Container, gVisor, Firecracker) |
| **LLM Providers** | 5 (Anthropic, OpenAI, Ollama, Z.AI, Local) |
| **Lean4 Axioms** | 39 (target: <30) |

### Strategic Pivot

The original roadmap targeted v2.0.0 "agentic workflows" as the next release. This is premature — the agentic executor falls back to a stub string when no LLM client is configured, and the competitive comparison overclaims features that don't work. The revised plan prioritizes:

1. **Credibility** — Make every claimed feature actually work
2. **Reliability** — Eliminate panic surfaces in production code
3. **Observability** — Coverage, benchmarks, monitoring
4. **Distribution** — Binary releases, docs site, community

Only after these foundations are solid does feature expansion make sense.

---

## Phase 5: Credibility & Foundations (v1.4.0)

> **Goal:** Every feature claimed in the README and competitive analysis actually works. Zero stubs on user-facing paths.

### v1.4.0-a — Panic Surface Reduction (Week 1-2)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 5.1 | Audit and classify all 1,182 `.unwrap()` calls by module | 2h | HIGH | Understand scope before fixing |
| 5.2 | Fix critical-path unwraps: `messaging/` (14 files), `nexus/` (12), `agentic/` (11) | 3 days | HIGH | These handle async state and LLM interactions — panics here lose user data |
| 5.3 | Fix initialization-path unwraps: `config.rs`, `plugin/loader.rs`, `onboarding/mod.rs` | 1 day | HIGH | Startup failures should be graceful errors |
| 5.4 | Add `#[cfg(test)]` guards where false positives inflate count | 2h | MEDIUM | Get accurate count |
| 5.5 | Enable `unwrap_used = "warn"` in CI (gradual enforcement) | 2h | HIGH | Prevent regression |
| 5.6 | Fix remaining 2 `todo!()` and 3 `unimplemented!()` calls | 1h | MEDIUM | Zero tolerance policy |
| **Target** | **<200 `.unwrap()` in production code** | — | Measurable |

### v1.4.0-b — Make Agentic Code Generation Real (Week 2-3)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 5.7 | Remove stub fallback in `executor_agent.rs` — error if no LLM client configured | 4h | CRITICAL | Currently silently produces no-ops |
| 5.8 | Implement real `run_cargo_test()` — invoke cargo subprocess, capture output, parse results | 1 day | CRITICAL | Currently returns hardcoded string |
| 5.9 | Implement real `run_sandboxed_tests()` — dispatch to actual sandbox backend | 1 day | HIGH | Currently returns fake `passed: true` |
| 5.10 | Add context-window management for LLM code generation (file slicing, summarization) | 2 days | HIGH | Required for quality generation on real codebases |
| 5.11 | Add multi-turn refinement loop (generate → verify → fix → regenerate) | 2 days | HIGH | Claude Code and Cursor differentiate here |
| 5.12 | Add agentic property tests (generated code compiles, tests pass) | 1 day | MEDIUM | Validate the agent end-to-end |
| **Target** | **`clawdius chat` generates real code that compiles** | — | Measurable |

### v1.4.0-c — Competitive Benchmarking (Week 3)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 5.13 | Run real benchmarks: startup time, memory usage, response P95 | 1 day | HIGH | Currently "targets" are aspirational |
| 5.14 | Benchmark against real competitors: Claude Code, Aider, Cursor (or publish methodology) | 2 days | HIGH | Prove or retract performance claims |
| 5.15 | Update competitive comparison with real data or remove aspirational numbers | 2h | HIGH | Credibility |
| 5.16 | Publish benchmark results in `docs/BENCHMARKS.md` | 2h | MEDIUM | Transparency |

### v1.4.0-d — Distribution (Week 3-4)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 5.17 | Build GitHub Releases with prebuilt binaries (Linux x86_64, aarch64, macOS) | 1 day | HIGH | Users shouldn't need Rust installed |
| 5.18 | Install `cargo-acl` and enforce in CI | 2h | MEDIUM | Prevent quality regression |
| 5.19 | Fix or remove aspirational claims from competitive comparison | 2h | HIGH | Honest marketing |
| **Target** | **`cargo install clawdius` works from GitHub Releases** | — | Measurable |

### v1.4.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | `.unwrap()` count <200 in production code | `grep -rc` automated check |
| G2 | Agentic code gen produces compilable output for 3+ test repos | Integration test |
| G3 | `run_cargo_test()` invokes real cargo | Integration test |
| G4 | `run_sandboxed_tests()` uses real backend | Integration test |
| G5 | Competitive benchmarks published with methodology | Documentation review |
| G6 | Prebuilt binaries in GitHub Release | Manual verification |
| G7 | `unwrap_used = "warn"` enforced in CI | CI pass/fail |

---

## Phase 6: User-Facing Quality (v1.5.0)

> **Goal:** Clawdius is the best-in-class Rust coding assistant for daily use.

### v1.5.0-a — IDE Integration (Week 5-6)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 6.1 | Wire completion module to VSCode extension (LSP or direct) | 3 days | HIGH | Most users discover tools via IDE |
| 6.2 | Wire completion module to JetBrains plugin | 3 days | HIGH | Second-largest IDE market |
| 6.3 | Add file-aware context (surrounding code, imports, types) to completions | 2 days | HIGH | Quality differentiator |
| 6.4 | Add debounce and cancellation for streaming completions | 1 day | MEDIUM | UX polish |
| **Target** | **Inline completions appear in VSCode and JetBrains** | — | Measurable |

### v1.5.0-b — LLM Quality (Week 6-7)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 6.5 | Prompt engineering: system prompts, few-shot examples, instruction formatting | 3 days | HIGH | Generated code quality |
| 6.6 | Context window optimization: smart file selection, summarization, token budgeting | 2 days | HIGH | Large repo support |
| 6.7 | Streaming UX: real-time progress, cancel, edit-in-place | 2 days | MEDIUM | Claude Code-level UX |
| 6.8 | Error recovery: parse compiler errors, feed back to LLM, retry | 2 days | HIGH | Reliability loop |
| **Target** | **Code generation quality matches Claude Code on standard benchmarks** | — | Measurable |

### v1.5.0-c — Developer Experience (Week 7-8)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 6.9 | Git workflow: auto-stage, commit messages, conflict resolution hints | 3 days | HIGH | Aider's key differentiator |
| 6.10 | `clawdius init` project scaffolding (cargo init + CLAUDE.md + config) | 1 day | MEDIUM | Faster onboarding |
| 6.11 | Interactive diff review: show changes, accept/reject/hunk-edit | 2 days | MEDIUM | Claude Code UX pattern |
| 6.12 | Session persistence: resume interrupted coding sessions | 1 day | MEDIUM | Long-running tasks |
| **Target** | **Full coding session from init to commit without leaving CLI** | — | Measurable |

### v1.5.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | Inline completions fire in VSCode (smoke test) | Manual QA |
| G2 | LLM code gen passes "fix this bug" benchmark (3 repos) | Integration test |
| G3 | Context window handles repo with 100+ files | Property test |
| G4 | Git workflow creates valid commits | Integration test |

---

## Phase 7: Community & Observability (v1.6.0)

> **Goal:** Clawdius has real users, real feedback, and real metrics.

### v1.6.0-a — Observability (Week 9-10)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 7.1 | Add `llvm-cov` to CI, publish coverage reports to PRs | 1 day | HIGH | Coverage tracking |
| 7.2 | Code coverage threshold in CI: fail PRs below 80% | 2h | HIGH | Quality enforcement |
| 7.3 | Structured logging with tracing spans for all user-facing paths | 2 days | MEDIUM | Production ops |
| 7.4 | Grafana dashboards for deployed instances | 2 days | MEDIUM | Operational visibility |
| 7.5 | Error telemetry: classified panic reports, graceful degradation tracking | 2 days | MEDIUM | Bug triage |
| **Target** | **Code coverage visible on every PR** | — | Measurable |

### v1.6.0-b — Community (Week 10-12)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 7.6 | Deploy docs.clawdius.dev (API reference, user guide, quickstart) | 3 days | HIGH | Discoverability |
| 7.7 | Write launch blog post with real benchmarks and demo | 1 day | HIGH | Launch marketing |
| 7.8 | Create 5-minute demo video (asciinema or screen recording) | 4h | HIGH | Visual demo |
| 7.9 | HN/Reddit/Lobsters submission with technical angle | 2h | HIGH | Distribution |
| 7.10 | Create Discord server with onboarding bot | 4h | MEDIUM | Community hub |
| 7.11 | Contribute to 3 open-source projects using Clawdius (dogfooding) | ongoing | LOW | Real-world validation |
| **Target** | **docs.clawdius.dev live, blog post published** | — | Measurable |

### v1.6.0-c — Cross-Platform (Week 12)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 7.12 | Add macOS CI runner to GitHub Actions | 4h | MEDIUM | 40% of developers use macOS |
| 7.13 | Add Windows CI runner (basic smoke test) | 8h | LOW | Enterprise requirement |
| 7.14 | Publish aarch64 Linux binary in GitHub Releases | 2h | MEDIUM | ARM server adoption |
| **Target** | **CI runs on 2+ platforms** | — | Measurable |

### v1.6.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | docs.clawdius.dev resolves | `curl` check |
| G2 | Code coverage >80% on critical paths | CI report |
| G3 | GitHub Stars > 50 (organic) | GitHub API |
| G4 | CI runs on Linux + macOS | CI matrix |

---

## Phase 8: Deepening (v1.7.0)

> **Goal:** Clawdius has best-in-class features in at least 2 categories.

### v1.7.0-a — HFT Deepening (Week 13-15)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 8.1 | Paper trading mode: simulated portfolio, P&L tracking, risk metrics | 3 days | MEDIUM | HFT profile architecture-only currently |
| 8.2 | News/sentiment feed adapter (at least 1 real source: Twitter/X API or RSS) | 2 days | MEDIUM | HFT sentiment analysis is architecture-only |
| 8.3 | Real broker connector (Alpaca or IBKR paper trading API) | 3 days | MEDIUM | Live market data |
| 8.4 | Lean4 axiom reduction: 39 → <30 (target <20 for HFT-critical) | 3 days | MEDIUM | Formal verification completeness |
| 8.5 | WCET benchmarks for wallet guard and risk check on real workloads | 1 day | LOW | Prove latency claims |

### v1.7.0-b — Sandbox Deepening (Week 15-17)

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 8.6 | Make gVisor backend functional (currently just a binary path) | 3 days | MEDIUM | Claimed in competitive analysis |
| 8.7 | Make Firecracker backend functional (microVM lifecycle management) | 3 days | MEDIUM | Claimed in competitive analysis |
| 8.8 | Sandbox escape test suite: verify isolation for each backend | 2 days | HIGH | Security proof |
| 8.9 | Resource limits enforcement (CPU, memory, network per sandbox) | 2 days | HIGH | Prevent abuse |
| 8.10 | Add Firecracker + gVisor to CI (smoke test, not full E2E) | 1 day | LOW | CI validation |

### v1.7.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | Paper trading runs for 100+ simulated trades | Integration test |
| G2 | At least 1 real news source adapter works | Integration test |
| G3 | 2+ sandbox backends pass escape test suite | Integration test |
| G4 | Lean4 axioms <30 | Proof compilation |

---

## Phase 9: Ecosystem (v1.8.0)

> **Goal:** Clawdius has a plugin ecosystem and third-party integrations.

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 9.1 | Plugin marketplace: discovery, install, version, trust scoring | 5 days | HIGH | Plugin system exists but no discoverability |
| 9.2 | Plugin SDK documentation with worked examples | 3 days | MEDIUM | Ecosystem enabler |
| 9.3 | DAP adapter (Debug Adapter Protocol) for IDE integration | 5 days | MEDIUM | VSCode debug integration |
| 9.4 | Neovim plugin (Lua-based, using LSP client) | 3 days | LOW | Vim community demand |
| 9.5 | Emacs package (Elisp, using LSP client) | 3 days | LOW | Emacs community demand |
| 9.6 | MCP server mode: expose Clawdius as an MCP tool server | 2 days | MEDIUM | Claude Code MCP interop |

### v1.8.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | External plugin can be installed via `clawdius plugin install <url>` | Integration test |
| G2 | DAP adapter attaches to VSCode debug session | Manual QA |

---

## Deferred to v2.0.0+

| Feature | Why Deferred |
|---------|---------------|
| GraphQL API | REST API covers current needs; no user demand signal |
| Air-gapped install | Complex deployment; no enterprise customer demand yet |
| GUI / Desktop App | CLI + IDE plugins cover developer use case; Electron-style GUI is a different product |
| Autonomous multi-agent (OpenDevin-style) | Requires solid single-agent first; premature |
| Kubernetes Helm charts | Docker Compose covers self-hosted; K8s is overkill for current scale |
| LLM sentiment analysis for trading | HFT deepening is sufficient for now |
| Multi-repo RAG | Single-repo works; multi-repo adds complexity |

---

## Metrics Trajectory

### Engineering Quality

| Metric | v1.3.0 | v1.4.0 Target | v1.6.0 Target | v1.8.0 Target |
|--------|---------|-----------------|-----------------|-----------------|
| `.unwrap()` in prod | ~1,182 | <200 | <100 | <50 |
| Test count | 1,162 | 1,300+ | 1,500+ | 2,000+ |
| Property tests | 43 | 60+ | 80+ | 100+ |
| Lean4 axioms | 39 | 39 | 35 | <30 |
| Code coverage | Unknown | Measured | >80% critical | >90% |
| CI platforms | 1 (Linux) | 1 | 2 (L+M) | 2+ |

### Distribution

| Metric | v1.3.0 | v1.4.0 Target | v1.6.0 Target | v1.8.0 Target |
|--------|---------|-----------------|-----------------|-----------------|
| GitHub Stars | 0 | Organic | 50+ | 500+ |
| Prebuilt binaries | None | Linux x86_64 | L+M+aarch64 | 5+ targets |
| docs.clawdius.dev | Not live | — | Live | Updated |
| Demo video | None | — | Published | Updated |
| Blog posts | 0 | 1 | 3+ | 5+ |

### Reliability

| Metric | v1.3.0 | v1.4.0 Target | v1.5.0 Target | v1.7.0 Target |
|--------|---------|-----------------|-----------------|-----------------|
| Stub features claimed | 3 | 0 | 0 | 0 |
| Panic surfaces | ~1,381 | <200 | <100 | <50 |
| Sandbox backends functional | 2 (WASM, Filtered) | 4 | 6 | 7 |
| Agentic workflows | Stub | Functional | Iterative | Multi-turn |

---

## Key Risk: Over-Engineering vs. Shipping

The biggest risk to this roadmap is spending too long on foundations and not enough on shipping. The mitigations:

1. **Every phase has measurable quality gates** — no phase transitions without proof
2. **Phase 5 (v1.4.0) is the last "catch-up" phase** — it fixes credibility gaps, not adds features
3. **Phases 6+ add user-visible value** — IDE completions, community, docs
4. **6-month horizon** — if v1.8.0 isn't substantially complete, reassess the approach

### Decision Points

| Date | Decision | Criteria |
|------|----------|----------|
| After v1.4.0 | Continue to v1.5.0? | Are stubs fixed? Is CI enforcing unwrap limits? |
| After v1.6.0 | Continue to v1.7.0? | Are real users providing feedback? Is docs site getting traffic? |
| After v1.8.0 | Plan v2.0.0? | Is the plugin ecosystem forming? Are there paying customers? |

---

## Conclusion

Clawdius v1.3.0 has exceptional technical depth (formal verification, 7 sandbox backends, HFT-grade performance) but needs to close a credibility gap before competing for users. The revised roadmap prioritizes:

1. **v1.4.0 (2 weeks):** Fix stubs, reduce panics, publish benchmarks, ship binaries
2. **v1.5.0 (4 weeks):** IDE completions, LLM quality, developer UX
3. **v1.6.0 (4 weeks):** Coverage, community launch, cross-platform CI
4. **v1.7.0 (4 weeks):** HFT deepening, sandbox completion, axiom reduction
5. **v1.8.0 (4 weeks):** Plugin ecosystem, DAP/Neovim/Emacs, MCP server mode

**Estimated total: ~18 weeks to a credible, user-ready v1.8.0.**

*This roadmap is a living document. Review after each phase.*
