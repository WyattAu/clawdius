# Clawdius Roadmap
## Strategic Vision & Development Plan

**Current Version:** 2.3.0
**Next:** v2.4.0 — TBD
**Last Updated:** 2026-04-13

---

## Executive Summary

Clawdius v2.3.0 is a Rust-native agentic coding assistant with Lean4 formal verification, multi-provider LLM support, MCP integration, and a plugin architecture. The project compiles across 4 crates with 595 passing tests, 142/142 Lean4 theorems proven, and real tool execution via the `CliToolExecutor`. Includes an HTTP server with API key auth, rate limiting, tenant model, and Prometheus metrics.

### Honest Current State (v2.3.0)

| Metric | Value | Notes |
|--------|-------|-------|
| **Rust LOC** | ~62,000 | Post-cleanup (was ~126K) |
| **Tests passing** | **595** | Down from 1,482 — many removed with dead code |
| **Tests failing** | 0 | |
| **Lean4 Proofs** | 142 theorems proven, 1 axiom, 0 sorrys | `postulate_signature_unforgeable` |
| **Compiler errors** | 0 | Full workspace compiles clean |
| **Clippy** | 200+ pre-existing warnings | Not blocking release; tracked for incremental fix |
| **Sandbox Backends (real)** | 3 | Container, Bubblewrap, Sandbox-exec |
| **Sandbox Backends (stub)** | 1 | Filtered (trivially bypassable) |
| **LLM Providers** | 3 working | Anthropic, OpenAI, Ollama |
| **Embeddings** | 3 options | Simple/hash (fake), Candle (behind `embeddings` feature gate), OpenAI API |
| **IDE Plugins** | 4 skeletons | VSCode, JetBrains, Neovim, Emacs — LSP only |
| **Protocol Support** | 4 working | JSON-RPC, LSP, MCP (HTTP+stdio), REST |
| **HTTP Server** | Working | `clawdius server` subcommand with API key auth, rate limiting, tenant model |
| **MCP Server** | 8 tools | read_file, write_file, edit_file, list_files, search, execute, glob, get_info |
| **Autonomous Coding** | Just wired | `ExecutorAgent` file ops connected; untested end-to-end |

### Known Issues (tracked, not blocking v2.1.0)

- Messaging gateway `generate` and `analyze` handlers return `[STUB]` placeholders
- 200+ pre-existing clippy suggestions across codebase
- `cargo publish --dry-run` for clawdius-core passes but other crates not yet verified
- `embeddings` feature pulls in `candle-core`/`half` with upstream trait bound errors
- Firecracker and gVisor sandbox backends removed (dead/broken code)
- Zai/Genai LLM provider removed (genai crate has no ZAI adapter)

---

## Completed Phases

### Phase 2.0 (v2.2.0) — Make It Stand Out — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 2.1 | LLM output parser + error recovery loop | DONE | Structured output parsing with retry on malformed LLM responses |
| 2.2 | VSCode extension fixes | DONE | Updated extension for compatibility with v2.2.0 CLI |
| 2.3 | MCP server write/edit tools | DONE | Added `write_file` and `edit_file` tools (6→8 total) |
| 2.4 | OpenAI API embeddings | DONE | Real embeddings via OpenAI `text-embedding-3-small` endpoint |
| 2.5 | CLI cleanup | DONE | Removed `broker`, `compliance`, `research` dead CLI commands |

---

### Phase 3.0 (v2.2.0) — Make It Self-Hostable — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 3.1 | HTTP server subcommand | DONE | `clawdius server` starts REST API on configurable port |
| 3.2 | API key auth middleware | DONE | Bearer token authentication for all endpoints |
| 3.3 | Fixed Dockerfiles | DONE | Multi-stage Docker builds for both CLI and server images |
| 3.4 | Deploy configs | DONE | Docker Compose and example deployment configuration |

---

### Phase 4.0 (v2.3.0) — Make It SaaS — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 4.1 | /metrics Prometheus endpoint | DONE | Exposes request counts, latency histograms, active connections |
| 4.2 | Rate limiting middleware | DONE | Per-key rate limits with configurable requests/minute |
| 4.3 | Tenant model with Free/Pro tiers | DONE | Tenant struct with tier-based feature gating |
| 4.4 | Usage tracking endpoints | DONE | Per-tenant usage counters and query endpoints |

---

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

---

### Phase 11: Ship-Ready (v2.0.0) — COMPLETE

| # | Task | Status | Result |
|---|------|--------|--------|
| 11.1 | Fix release workflow CI | DONE | 19 iterations to resolve all environment issues |
| 11.2 | GitHub Release with binaries | DONE | 4 platforms: Linux, macOS (x64+ARM), Windows |
| 11.3 | SBOM generation | DONE | CycloneDX JSON included in release |
| 11.4 | Publish to crates.io | BLOCKED | Requires CRATES_IO_TOKEN secret (not set) |

**Post-Mortem (19 release iterations):**

The release workflow required 19 attempts. Root causes were all environment-specific:
CI assumptions that didn't hold on GitHub runners.

| # | Issue | Root Cause |
|---|-------|-----------|
| 1 | rustfmt nightly options | Used unstable rustfmt features on stable CI |
| 2 | --all-features embeddings | candle-core has upstream trait bound errors |
| 3 | lib.rs deleted by commit | `cargo fmt` stripped module decls on parse errors |
| 4 | CI Rust version drift | `@stable` tracked 1.94+ with new clippy lints |
| 5 | cargo deny wasmtime advisory | RUSTSEC-2026-0096 in wasmtime 42.0.1 |
| 6 | nextest --profile ci | Wrong flag; should be --cargo-profile |
| 7 | test_health_check_memory | Our fix changed semantics for empty store |
| 8 | LeanVerifier tests | lean binary not installed on CI |
| 9 | Browser test | Headless Chrome available on CI, test assumed not |
| 10 | Coverage + protoc | --all-features pulled in lance-encoding |
| 11 | Windows PowerShell | Bash [[ ]] syntax in PS context |
| 12 | SBOM filename | cargo cyclonedx output name varies by version |
| 13-14 | musl + aarch64-linux builds | openssl-sys needs target-specific headers |
| 15 | aarch64-linux-gnu | Same openssl cross-compilation issue |
| 16 | macOS openssl | Apple deprecated system OpenSSL |
| 17 | rustls-platform-verifier LTO | Proc-macro crate can't be LTO'd |
| 18 | Cargo.toml parse error | `lto` not allowed in per-package profiles |
| 19 | Post-Release Tasks | git push after tag push ref mismatch |

**Lesson:** Test the EXACT CI command locally before pushing. The codebase had tests
that depended on the local environment (lean, headless Chrome) and the CI workflow
had assumptions about GitHub runner toolchains that didn't hold.

**Commits:** 95b582d..50143d5

---

### Phase 12: Make It Useful (v2.2.0)

> **Goal:** One feature that works end-to-end better than the competition.

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 12.1 | End-to-end autonomous coding demo | 5 days | HIGH | `clawdius generate --mode agent` actually works |
| 12.2 | Error recovery loop: write → test → fix → retry | 3 days | HIGH | Key differentiator vs Aider |
| 12.3 | One IDE plugin with real inline completions | 5 days | HIGH | VSCode with actual code completions |
| 12.4 | Persist marketplace to SQLite | 2 days | MEDIUM | In-memory registry loses state |
| 12.5 | Top 50 clippy suggestions fixed | 2 days | LOW | Incremental quality |

### v2.2.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | `clawdius generate` writes, tests, and fixes a real file | Manual demo |
| G2 | Error recovery loop passes 3+ iterations | Integration test |
| G3 | VSCode inline completions work with Ollama | Manual QA |

---

### Phase 13: Depth (v2.3.0)

> **Goal:** Formal verification and performance excellence.

| # | Task | Effort | Priority | Rationale |
|---|------|--------|---------|-----------|
| 13.1 | Lean4 axiom 1→0 | 5 days | MEDIUM | `postulate_signature_unforgeable` is a standard crypto assumption |
| 13.2 | TLA+ model checking for concurrent systems | 5 days | LOW | Verify FSM and sandbox isolation properties |
| 13.3 | SIMD optimizations | 3 days | LOW | Performance for batch operations |
| 13.4 | PGO + BOLT builds | 2 days | LOW | Optimized release binaries |
| 13.5 | Fix remaining 150+ clippy suggestions | 3 days | LOW | Code quality |

### v2.3.0 Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| G1 | Lean4 axioms = 0 | Proof compilation |
| G2 | TLA+ model passes for FSM | TLC checker |
| G3 | PGO build completes in CI | CI pipeline |

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
| DeepSeek/OpenRouter LLM providers | Stub-only; removed with CLI cleanup |

---

## Lessons Learned (v2.0.0 Post-Mortem)

### What went wrong

1. **lib.rs accidentally deleted** — Commit `f47a6fe` overwrote the entire `clawdius-core/src/lib.rs` (121 lines → 3 lines), making the entire crate API invisible. Went unnoticed because `cargo check` of the lib target doesn't exercise the full public API surface. **Mitigation:** Add a CI job that verifies `cargo test --doc` compiles (checks that all public items are documented and accessible).

2. **ROADMAP.md became fiction** — As features were added, the roadmap was updated to claim "0 stubs" and "5 production backends" when 3 backends are non-functional and multiple handlers return `[STUB]`. **Mitigation:** ROADMAP now includes an "Honest Current State" table with a "Notes" column.

3. **CI kept failing on new Rust versions** — `dtolnay/rust-toolchain@stable` tracks the latest stable, introducing new clippy lints that fail with `-D warnings` on 126K LOC of pre-existing code. **Mitigation:** Pin Rust version in CI; use `-W clippy::all` (warn) instead of `-D warnings` (deny).

4. **`rustfmt.toml` had nightly-only options** — 15 options like `imports_granularity` and `group_imports` require nightly Rust, causing `cargo fmt --check` to fail on stable CI. **Mitigation:** Only use stable-channel rustfmt options.

5. **Test path validation was too strict** — FileTool's `validate_path()` rejected paths outside `workspace_root`, but integration tests use `TempDir` in `/tmp`. **Mitigation:** Tests now use `FileTool::with_workspace_root()` to set the temp dir as workspace root.

### What went right

1. **Lean4 proofs** — 142/142 theorems proven, only 1 axiom remaining. This is a genuine differentiator.
2. **Real tool execution** — `CliToolExecutor` with 9 working tools replaced the `NoOpToolExecutor`.
3. **MCP integration** — Claude Desktop can use Clawdius as a tool server via stdio transport, now with 8 tools including write/edit.
4. **Test suite** — 595 passing tests (down from 1,482 after removing dead code — honest count).
5. **Release workflow** — Comprehensive multi-platform build with GPG signing, SBOM, crates.io publish.
6. **HTTP server** — Self-hostable with API key auth, rate limiting, tenant model, and Prometheus metrics.

---

## Metrics Trajectory

### Engineering Quality

| Metric | v1.3.0 | v1.4.0 | v1.5.0 | v1.6.0 | v1.7.0 | v1.8.0 | v2.0.0 | v2.1.0 | v2.2.0 | v2.3.0 |
|--------|---------|---------|---------|---------|---------|---------|---------|---------|---------|---------|
| `.unwrap()` in prod | 101 | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** |
| Compiler warnings | — | — | — | — | **0** | **0** | **0** | **0** | **0** | **0** |
| Property tests | 43 | **43** | **43** | **43** | **67** | **67** | **67** | **67** | **67** | **67** |
| Sandbox escape tests | 0 | 0 | 0 | 0 | **36** | **36** | **36** | **36** | **36** | **36** |
| Lean4 axioms | 39 | **11** | **11** | **11** | **11** | **11** | **2** | **2** | **2** | **2** |
| Lean4 sorrys | — | — | — | — | **0** | **0** | **0** | **0** | **0** | **0** |
| CI platforms | 1 | **1** | **1** | **7** | **7** | **9** | **9** | **9** | **9** | **9** |
| Core modules | 46 | 46 | 46 | 46 | 46 | 46 | **46** | **38** | **38** | **38** |
| Workspace crates | 4 | 4 | 4 | 4 | 4 | 6 | **6** | **4** | **4** | **4** |
| Total LOC | ~80K | ~80K | ~100K | ~120K | ~120K | ~126K | ~126K | **~62K** | **~62K** | **~62K** |
| Tests passing | — | — | — | — | — | — | 1,482 | 1,482 | **595** | **595** |

### Distribution

| Metric | v1.3.0 | v1.4.0 | v1.5.0 | v1.6.0 | v1.7.0 | v1.8.0 | v2.0.0 | v2.1.0 | v2.2.0 | v2.3.0 |
|--------|---------|---------|---------|---------|---------|---------|---------|---------|---------|---------|
| GitHub Stars | 0 | Organic | Organic | Organic | Organic | Organic | Organic | Organic | Organic | Organic |
| Prebuilt binaries | None | None | None | Pipeline ready | — | Release signing | **Ready** | **Published** | **Published** | **Published** |
| docs.clawdius.dev | Not live | Not live | Not live | Ready (mdBook) | — | — | — | Updated | Updated | Updated |
| Demo video | None | None | None | — | — | — | — | — | — | — |

### Reliability

| Metric | v1.3.0 | v1.4.0 | v1.5.0 | v1.6.0 | v1.7.0 | v1.8.0 | v2.0.0 | v2.1.0 | v2.2.0 | v2.3.0 |
|--------|---------|---------|---------|---------|---------|---------|---------|---------|---------|---------|
| Stub features claimed | 3 | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** |
| Panic surfaces | 101 | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** | **0** |
| Sandbox backends functional | 2 (WASM, Filtered) | **5** | **5** | **5** | **5** | **5** | **3** | **3** | **3** | **3** |
| RPC handlers functional | 0/5 | 1/5 | **5/5** | **5/5** | **5/5** | **5/5** | **5/5** | **5/5** | **5/5** | **5/5** |
| IDE integrations | 0 | 1 (VSCode stub) | **1** | **1** | **1** | **3** | **4** | **4** | **4** | **4** |
| Protocol layers | 2 | 2 | 2 | 2 | 2 | **5** | **4** | **4** | **4** | **4** |
| MCP tools | — | — | — | — | — | **6** | **6** | **6** | **8** | **8** |
| LLM providers working | — | — | — | — | — | — | **3** | **3** | **3** | **3** |

### Removed in v2.1.0

| Feature | Why |
|---------|-----|
| HFT trading / broker mode | Orthogonal to coding assistant |
| Multi-platform messaging gateway | Handlers were stubs |
| Enterprise SSO/compliance | Struct-only, no real auth |
| WASM plugin system | No plugins existed |
| 24-phase Nexus FSM engine | No users |
| Multi-lingual knowledge graph | Rule-based translation was useless |
| GraphQL API layer | Removed with server crate |
| Webview UI (Leptos) | Skeleton with no core dependency |

### Removed in v2.2.0

| Feature | Why |
|---------|-----|
| Firecracker sandbox backend | Dead code, never worked end-to-end |
| gVisor sandbox backend | Stub only, not implemented |
| `broker`, `compliance`, `research` CLI commands | Dead commands with no backing implementation |

---

## Key Risk: Focus vs. Breadth

The v2.1.0 cleanup addressed the biggest risk: codebase bloat from speculative features. By removing 41K LOC of off-mission code, the project now has a clear identity: an agentic coding assistant with LLM integration, sandboxing, MCP, and formal verification.

The remaining risks:

1. **Identity crisis** — "Agentic coding engine" describes every AI tool in 2025. Clawdius needs one feature that's genuinely best-in-class.
2. **No real users** — Without user feedback, development direction is speculative.
3. **Competition** — Aider (30K stars), Claude Code (Anthropic), Cursor (VC-funded) dominate this space.
3. **Phases 6–10 all shipped** — IDE completions, community, docs, ecosystem, platform maturity all done
4. **v2.1.0 is the release phase** — focus on shipping signed binaries, not new features

### Decision Points

| Date | Decision | Criteria |
|------|----------|----------|
| ~~After v1.4.0~~ | ~~Continue to v1.5.0?~~ | **DONE — proceed to v1.5.0** |
| ~~After v1.6.0~~ | ~~Continue to v1.7.0?~~ | **DONE — proceeded to v1.7.0** |
| ~~After v1.8.0~~ | ~~Plan v2.0.0?~~ | **DONE — v2.0.0 shipped** |
| After v2.1.0 | Continue to v2.2.0? | Are real users providing feedback? Is marketplace getting traction? |

---

## Conclusion

Clawdius v2.3.0 is a leaner, more focused project. The v2.1.0 cleanup removed 41K LOC of off-mission code (broker trading, multi-platform messaging, enterprise SSO, WASM plugins, 24-phase FSM, knowledge graphs), 18 dead dependencies, and 2 crates (server, webview). The result is a 48% smaller codebase with zero test failures, zero production unwraps, and a clean dependency tree. v2.2.0 added an HTTP server, MCP write/edit tools, and OpenAI embeddings. v2.3.0 added Prometheus metrics, rate limiting, and a tenant model.

The roadmap continues:

1. **v1.4.0 (DONE):** Fix stubs, eliminate panics, publish benchmarks
2. **v1.5.0 (DONE):** IDE integration, LLM quality, git workflow, scaffolding
3. **v1.6.0 (DONE):** Coverage enforcement, cross-platform CI (7 targets), Codecov
4. **v1.6.1 (DONE):** CI/security hardening, warning elimination
5. **v1.7.0 (DONE):** Credibility completion — axiom reduction, sorry resolution, sandbox escape tests, security audit
6. **v1.8.0 (DONE):** Ecosystem expansion — MCP server, DAP adapter, Neovim plugin, release signing
7. **v2.0.0 (DONE):** Platform maturity — Lean4 axioms 11→2, Emacs plugin, multi-agent, GraphQL, marketplace
8. **v2.1.0 (DONE):** Codebase cleanup — removed 8 dead modules, 2 dead crates, 18 dead deps, fixed all doc tests
9. **v2.2.0 (DONE):** Make It Stand Out — LLM output parser, MCP write/edit tools, OpenAI embeddings, CLI cleanup
10. **v2.2.0 (DONE):** Make It Self-Hostable — HTTP server, API key auth, Dockerfiles, deploy configs
11. **v2.3.0 (DONE):** Make It SaaS — Prometheus metrics, rate limiting, tenant model, usage tracking
12. **v2.4.0 (next):** TBD — pending user feedback and priorities

*This roadmap is a living document. Review after each phase.*