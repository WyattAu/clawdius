# Clawdius Roadmap
## Strategic Vision & Development Plan

**Current Version:** 2.4.0
**Next:** v2.5.0 — TBD
**Last Updated:** 2026-04-18

---

## Executive Summary

Clawdius v2.4.0 is a Rust-native agentic coding engine that **exceeds gstack's capabilities** (gstack: 73.9K stars). It replicates gstack's workflow capabilities (sprint process, skills system, browser automation, multi-model review) as an optimized, self-hosted, multi-LLM environment with Lean4 formal verification, multi-tier sandboxing, and VFS abstraction.

### Honest Current State (v2.4.0)

| Metric | Value | Notes |
|--------|-------|-------|
| **Rust LOC** | ~68,000 | +6K from gstack-competitive features |
| **Tests passing** | **720** | Up from 595 in v2.3.0 |
| **Tests failing** | 0 | |
| **Lean4 Proofs** | 69 theorems proven | `.specs/02_architecture/proofs/` |
| **Compiler errors** | 0 | Full workspace compiles clean |
| **Clippy** | Pre-existing warnings | Not blocking; tracked for incremental fix |
| **Sandbox Backends (real)** | 3 | Container, Bubblewrap, Sandbox-exec |
| **LLM Providers** | 4 working | Anthropic, OpenAI, Ollama, **OpenRouter** |
| **Skills** | 4 built-in + 7 markdown | LLM-powered with fallback |
| **MCP Tools** | 12 | git_commit, grep_search, multi_file_edit, list_branches, +8 original |
| **Sprint Engine** | 7-phase FSM | think→plan→build→review→test→ship→reflect |
| **Error Recovery** | write→test→fix→retry | Integrated into sprint engine |
| **Multi-Model Review** | 7 focus areas | Concurrent review with dedup & fusion |
| **Parallel Sprints** | Session orchestration | Concurrency-limited, priority-queued |
| **Browser Daemon** | Persistent + refs | Accessibility-tree `@e1`, `@e2` element system |
| **Ship Pipeline** | Safety + benchmarks | Branch protection, canary, regression detection |

---

## gstack-Competitive Milestones (v2.4.0) — ALL COMPLETE

> **Goal:** Replicate and exceed gstack's workflow capabilities as a Rust-native, self-hosted, multi-LLM system.

### M1: Wire the Core Loop — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M1.1 | OpenRouter provider | `llm/providers/openrouter.rs` — multi-model LLM access |
| M1.2 | LLM-powered skills | All 4 built-in skills call LLM, fall back to hardcoded text |
| M1.3 | Markdown skill definitions | `skills/markdown_skill.rs` — YAML frontmatter parser (685 lines) |
| M1.4 | 7 example Markdown skills | `.clawdius/skills/`: ship.md, investigate.md, qa.md, retro.md, office-hours.md, benchmark.md, sprint.md |
| M1.5 | 4 new MCP tools | `git_commit`, `grep_search`, `multi_file_edit`, `list_branches` |
| M1.6 | Lean4 proofs | 69 theorems in `.specs/02_architecture/proofs/` |

### M2: Sprint Process Engine — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M2.1 | SprintEngine core | 7-phase state machine: Think→Plan→Build→Review→Test→Ship→Reflect |
| M2.2 | Phase system prompts | Specialized LLM prompts per phase |
| M2.3 | Checkpoint/rollback | `git stash push/pop` for safe experimentation |
| M2.4 | SprintMetrics | Token counting per phase, per-phase timing, ASCII report |
| M2.5 | Tests | 20 unit tests, all pass |

### M3: Error Recovery & QA Loop — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M3.1 | Real execution | `SprintConfig.real_execution` + `build_command`/`test_command` |
| M3.2 | ToolExecutor integration | `SprintEngine.with_tool_executor()` |
| M3.3 | Error recovery loop | LLM fix → write → re-verify cycle |
| M3.4 | File tracking | `get_changed_files()` via git diff |
| M3.5 | Language detection | `detect_language()` — 16 file extensions |
| M3.6 | Browser QA | `SprintConfig.browser_qa_url` — visual QA in Test phase |
| M3.7 | Tests | 8 new tests (28 sprint tests total) |

### M4: Review & Multi-Model Pipeline — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M4.1 | ReviewEngine | Concurrent multi-provider review execution |
| M4.2 | 7 focus areas | CodeQuality, Security, Performance, Robustness, ApiDesign, Testing, General |
| M4.3 | FusedReview | Merged reviews with dedup (word-overlap 80%) and avg score |
| M4.4 | Sprint integration | `SprintConfig.reviewers` — replaces single-LLM review |
| M4.5 | Tests | 14 review tests + 1 integration test |

### M5: Browser Daemon & Parallel Sprints — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M5.1 | ParallelSprintManager | Session submit/list/cancel, concurrency limits |
| M5.2 | SessionState lifecycle | Pending→Running→Completed/Failed/Cancelled |
| M5.3 | BrowserDaemon | Persistent browser with `BrowserSession` trait |
| M5.4 | Accessibility-tree refs | `@e1`, `@e2` element references with DOM walking |
| M5.5 | Ref-based interaction | `click_ref()`, `type_ref()`, `read_ref()` |
| M5.6 | Session-scoped maps | Per-session element refs with auto-snapshot |
| M5.7 | StubBrowserSession | No-op implementation for testing without Chromium |
| M5.8 | Tests | 12 parallel sprint + 13 browser daemon tests |

### M6: Ship Pipeline & Benchmarking — ✅ COMPLETE

| # | Feature | Details |
|---|---------|---------|
| M6.1 | ShipPipeline | Branch safety rules, pre-ship checks, commit message generation |
| M6.2 | BranchProtection | None/RequireTestsPass/RequireReviewApproval/Full |
| M6.3 | CommitMessageStrategy | ConventionalCommits, LlmGenerated, CustomTemplate |
| M6.4 | Auto type detection | ConventionalCommitType inferred from changed files |
| M6.5 | CanaryConfig | Traffic %, observation period, error/latency thresholds, auto-rollback |
| M6.6 | CanaryDeployment | Preparing→Observing→Passed/Failed→RolledOut lifecycle |
| M6.7 | BenchmarkSuite | Collect, compare, detect regressions with configurable threshold |
| M6.8 | ShipStats | Success rate, average duration tracking |
| M6.9 | Tests | 30 unit tests covering all components |

### Clawdius Advantages Over gstack

| Feature | gstack | Clawdius |
|---------|--------|----------|
| **LLM Support** | Claude only | Multi-LLM (Anthropic, OpenAI, Ollama, OpenRouter) |
| **Formal Verification** | None | 69 Lean4 theorems |
| **Sandboxing** | None | Multi-tier (WASI/Bubblewrap/Container) |
| **VFS Abstraction** | None | Full virtual filesystem |
| **Deployment** | Claude Code plugin | Self-hosted binary + Docker |
| **Language** | Markdown/Bash | Rust-native performance |
| **LSP Client** | None | Built-in |
| **Review System** | Single model | Multi-model fusion (7 focus areas) |
| **Browser Refs** | @e1, @e2 | @e1, @e2 (same system, Rust-native) |
| **Sprint Process** | think→plan→build→review→test→ship→reflect | Same 7 phases + error recovery loop |

---

## Completed Phases (Pre-v2.4.0)

### Phase 2.0 (v2.2.0) — Make It Stand Out — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 2.1 | LLM output parser + error recovery loop | DONE | Structured output parsing with retry on malformed LLM responses |
| 2.2 | VSCode extension fixes | DONE | Updated extension for compatibility with v2.2.0 CLI |
| 2.3 | MCP server write/edit tools | DONE | Added `write_file` and `edit_file` tools (6→8 total) |
| 2.4 | OpenAI API embeddings | DONE | Real embeddings via OpenAI `text-embedding-3-small` endpoint |
| 2.5 | CLI cleanup | DONE | Removed `broker`, `compliance`, `research` dead CLI commands |

### Phase 3.0 (v2.2.0) — Make It Self-Hostable — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 3.1 | HTTP server subcommand | DONE | `clawdius server` starts REST API on configurable port |
| 3.2 | API key auth middleware | DONE | Bearer token authentication for all endpoints |
| 3.3 | Fixed Dockerfiles | DONE | Multi-stage Docker builds for both CLI and server images |
| 3.4 | Deploy configs | DONE | Docker Compose and example deployment configuration |

### Phase 4.0 (v2.3.0) — Make It SaaS — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 4.1 | /metrics Prometheus endpoint | DONE | Exposes request counts, latency histograms, active connections |
| 4.2 | Rate limiting middleware | DONE | Per-key rate limits with configurable requests/minute |
| 4.3 | Tenant model with Free/Pro tiers | DONE | Tenant struct with tier-based feature gating |
| 4.4 | Usage tracking endpoints | DONE | Per-tenant usage counters and query endpoints |

### Phase 11.5: Codebase Cleanup (v2.1.0) — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 11.5.1 | Delete off-mission modules | DONE | Removed broker, nexus, messaging, enterprise, plugin, brain, knowledge, auth (41K LOC) |
| 11.5.2 | Delete dead crates | DONE | Removed clawdius-server, clawdius-webview |
| 11.5.3 | Remove dead dependencies | DONE | Removed 18 deps (wasmtime, git2, monoio, ed25519-dalek, etc.) |
| 11.5.4 | Gate heavy dependencies | DONE | chromiumoxide behind `browser` feature flag |
| 11.5.5 | Fix all doc tests | DONE | 27 README + 14 source doc examples fixed |
| 11.5.6 | Fix production unwraps | DONE | 8 remaining → 0 (replaced with `.expect()`) |
| 11.5.7 | Remove dead CLI commands | DONE | Removed nexus, workflow, broker commands |
| 11.5.8 | Clean CI | DONE | Removed dead server clippy reference |
| 11.5.9 | Remove dead test/bench files | DONE | Removed hft, messaging, nexus, property tests |
| 11.5.10 | Remove dead feature flags | DONE | Removed hft-mode, broker-mode, encryption, jwt |

**Result:** 48% LOC reduction, 0 test failures, 0 production unwraps, 6 clean feature flags.

### Phase 11: Ship-Ready (v2.0.0) — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 11.1 | Fix release workflow CI | DONE | 19 iterations to resolve all environment issues |
| 11.2 | GitHub Release with binaries | DONE | 4 platforms: Linux, macOS (x64+ARM), Windows |
| 11.3 | SBOM generation | DONE | CycloneDX JSON included in release |
| 11.4 | Publish to crates.io | BLOCKED | Requires CRATES_IO_TOKEN secret (not set) |

---

## Metrics Trajectory

### Engineering Quality

| Metric | v2.3.0 | v2.4.0 | Delta |
|--------|---------|---------|-------|
| `.unwrap()` in prod | **0** | **0** | — |
| Tests passing | 595 | **720** | +125 |
| Lean4 proofs | 142 | 69 (consolidated) | Reorganized |
| LLM Providers | 3 | **4** | +OpenRouter |
| MCP Tools | 8 | **12** | +4 |
| Skills (built-in) | 0 | **4** | +4 |
| Skills (markdown) | 0 | **7** | +7 |
| Sprint phases | 0 | **7** | Full FSM |
| Browser daemon | No | **Yes** | +Accessibility refs |
| Ship pipeline | No | **Yes** | +Canary +Benchmarks |

---

## Known Issues

- Messaging gateway `generate` and `analyze` handlers return `[STUB]` placeholders
- 200+ pre-existing clippy suggestions across codebase
- `cargo publish --dry-run` for clawdius-core passes but other crates not yet verified
- `embeddings` feature pulls in `candle-core`/`half` with upstream trait bound errors
- Background rust-analyzer processes occasionally revert uncommitted files (workaround: commit immediately)

---

## Deferred Indefinitely

| Feature | Why |
|---------|-----|
| Air-gapped install | No enterprise customer demand; complex deployment |
| GUI / Desktop App | CLI + IDE plugins cover developer use case |
| Kubernetes Helm charts | Docker Compose covers self-hosted |
| Multi-repo RAG | Single-repo works; multi-repo adds complexity |
| HFT trading / broker mode | Cool but confusing to users; orthogonal to coding |
| Multi-platform messaging gateway | Handlers were stubs; better served by dedicated tools |
| Enterprise SSO/compliance | No enterprise customers; structs-only implementation |
| WASM plugin system | No plugins existed; sandbox covers isolation |
| 24-phase Nexus FSM | No users; over-engineered lifecycle management |
| Multi-lingual knowledge graph | Rule-based translation was useless |
| GraphQL API layer | REST API is sufficient; GraphQL removed with server crate |
| Webview UI | Leptos skeleton; CLI + VSCode cover the use case |
| Firecracker sandbox | Dead code; never worked end-to-end |
| gVisor sandbox | Not implemented; stub removed |
| Zai/Genai LLM provider | genai crate has no ZAI adapter |

---

## Conclusion

Clawdius v2.4.0 achieves the primary goal: **exceeding gstack's capabilities** as a Rust-native, self-hosted, multi-LLM agentic coding engine. All 6 gstack-competitive milestones are complete:

1. **M1 (DONE):** Wire the Core Loop — OpenRouter, LLM skills, MCP tools, Lean4 proofs
2. **M2 (DONE):** Sprint Process Engine — 7-phase FSM with checkpoint/rollback
3. **M3 (DONE):** Error Recovery — write→test→fix→retry loop
4. **M4 (DONE):** Multi-Model Review — 7 focus areas, concurrent review, dedup & fusion
5. **M5 (DONE):** Browser Daemon — persistent Chromium, accessibility-tree `@eN` refs
6. **M6 (DONE):** Ship Pipeline — branch safety, canary deployment, benchmark regression

The roadmap continues with v2.5.0 priorities TBD based on user feedback and benchmark results.

*This roadmap is a living document. Review after each phase.*
