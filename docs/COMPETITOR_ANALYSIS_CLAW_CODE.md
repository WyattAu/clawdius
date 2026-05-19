# Competitor Analysis: Clawdius vs Claw Code

**Repository:** [ultraworkers/claw-code](https://github.com/ultraworkers/claw-code)
**Analysis Date:** 2026-05-19
**Clawdius Version:** 1.0.0-rc.1
**Claw Code Version:** 0.1.0 (early alpha)

---

## Executive Summary

Claw Code is a Rust-native AI coding assistant from UltraWorkers that shares Clawdius's philosophy of building in Rust for performance and safety. However, the two projects diverge significantly in architecture, maturity, target users, and design philosophy. Claw Code is positioned as a Claude Code competitor with multi-provider support and autonomous agent coordination, while Clawdius targets enterprise-grade deployments with formal verification, multi-platform messaging, and high-assurance sandboxing.

---

## Architecture Comparison

| Dimension | Clawdius | Claw Code |
|-----------|----------|-----------|
| **Workspace crates** | 5 (core, cli, gateway, mcp, code) | 9 (cli, api, runtime, tools, commands, plugins, telemetry, compat-harness, mock-service) |
| **Rust LOC** | ~130K (344 files) | ~92K |
| **Architecture pattern** | Modular library + CLI + gateway server | Agent-harness REPL runtime |
| **Async runtime** | Tokio | Tokio |
| **Configuration format** | TOML | JSON (5-level cascade) |
| **Session storage** | SQLite + JSONL | JSONL only (file-based) |
| **Vector/Graph DB** | LanceDB + SQLite hybrid | None |
| **TLS** | rustls | rustls |
| **unsafe code** | 3 files (simd, proof templates, drift analysis) | `forbid` globally |
| **Formal verification** | Lean4 (209 theorems, 15 proof files) | None |

---

## Feature Matrix

### Core Agent Capabilities

| Feature | Clawdius | Claw Code | Assessment |
|---------|----------|-----------|------------|
| Interactive REPL/TUI | Yes (ratatui, 60 FPS) | Yes (rustyline) | Clawdius has richer TUI; Claw Code has tab completion |
| One-shot prompt | Yes | Yes | Parity |
| JSON output mode | Yes | Yes | Parity |
| Session resume | Yes (SQLite-backed) | Yes (JSONL files) | Clawdius more robust |
| Session compaction | Yes | Yes (auto-summarization) | Parity |
| File context (`@path`) | Yes | Yes | Parity |
| Slash commands | Yes (25+) | Yes (30+) | Claw Code has more commands |
| Streaming display | Yes | Yes (SSE with backpressure) | Claw Code more sophisticated streaming |
| Markdown rendering | Yes (syntect) | Yes (syntect + pulldown-cmark) | Claw Code slightly richer |
| Tab completion | No | Yes (commands, aliases, modes) | Claw Code wins |

### LLM Provider Support

| Provider | Clawdius | Claw Code |
|----------|----------|-----------|
| Anthropic Claude | Yes | Yes (direct API) |
| OpenAI GPT | Yes | Yes (direct API) |
| DeepSeek | Yes | No |
| OpenRouter | Yes | Via OpenAI-compatible endpoint |
| Ollama (Local) | Yes | Yes (via OpenAI-compatible) |
| ZAI | Yes | No |
| xAI Grok | No | Yes |
| DashScope/Alibaba | No | Yes |
| vLLM | No | Via OpenAI-compatible |
| llama.cpp | No | Via OpenAI-compatible |
| Model-specific handling | No | Yes (per-model parameter adaptation) |
| Prompt caching | No | Yes (fingerprint-based with TTL) |
| OAuth PKCE flow | No | Yes |
| Automatic model routing | No | Yes (prefix-based) |

**Assessment:** Claw Code has broader provider coverage and more sophisticated model management. Clawdius covers the major providers but lacks xAI and DashScope.

### Security

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| Sandboxing | 5 backends (WASM, Filtered, Bubblewrap, Container, Sandbox-exec) + 2 planned (gVisor, Firecracker) | Container detection only (no real isolation) |
| Permission system | Capability tokens, 23 permissions | 3 modes (read-only, workspace-write, danger-full-access) |
| Path traversal prevention | Canonical path validation | String-prefix + canonical (documented gaps) |
| Command filtering | Blocks dangerous commands | Bash heuristic validation |
| Network isolation | Per-execution toggle | Container-level only |
| Secret storage | OS keyring (keyring crate) | File-based with redaction |
| Secret redaction | Yes | Yes (MCP env, session JSONL) |
| Workspace boundary | Full | Basic (documented symlink gaps) |
| Encryption at rest | AES-256-GCM | No |
| Formal proofs | 209 Lean4 theorems | None |
| CVE tracking | 6 tracked in deny.toml | No deny.toml |

**Assessment:** Clawdius is significantly stronger in security. Five real sandbox backends vs container detection heuristic. Formal verification provides mathematical guarantees absent in Claw Code.

### Messaging / Gateway

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| Messaging platforms | 9 adapters (Discord, Slack, Telegram, Matrix, IRC, Teams, Webex, Email, HTTP) | None (CLI-only) |
| Multi-tenant | Yes | No |
| Rate limiting | Per-user, per-platform | No |
| Webhook gateway | Yes (axum-based) | No |

**Assessment:** Clawdius has a complete messaging gateway; Claw Code is CLI-only.

### Code Intelligence

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| Tree-sitter parsers | 5 (Rust, Python, JS, TS, Go) | No |
| Vector search | LanceDB hybrid | No |
| Symbol extraction | Yes (functions, classes, imports) | No |
| Semantic code graph | Yes (Graph-RAG) | No |
| LSP integration | No | Yes (registry-level, not full) |
| Multi-lingual research | 16 languages | No |

**Assessment:** Clawdius has full code intelligence pipeline; Claw Code has LSP stub only.

### MCP Support

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| MCP server | Yes (jsonrpsee, full implementation) | Registry bridge only (incomplete) |
| MCP client | Yes | Yes (registry-level) |
| Required vs optional servers | Yes | Yes |
| Tool name normalization | Yes | Yes (`mcp__<server>__<tool>`) |
| Degraded startup | No | Yes |

**Assessment:** Clawdius has a complete, production MCP server. Claw Code's MCP is registry-level scaffolding.

### Testing

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| Total tests | 1,527 (1,420 lib + 97 integration + 10 adapter) | ~1,093 unit + integration |
| Property-based tests | 27 (proptest) | No |
| Adapter tests | 136 | No |
| Mock service | No | Yes (deterministic Anthropic mock) |
| Fuzz testing | 5 fuzz targets (AFL++) | No |
| CI test matrix | Linux, macOS, Windows, Lean4 | Linux, Windows |
| Code coverage | ~60% (100% on mcp, code) | Not measured |
| Ignored tests | 0 | 0 |

**Assessment:** Comparable test counts. Clawdius has property-based tests, fuzz testing, adapter tests, and cross-platform CI. Claw Code has a superior mock service for deterministic E2E testing.

### CI/CD

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| CI provider | GitHub Actions | GitHub Actions |
| Lint | clippy -D warnings | clippy -D warnings |
| Format | cargo fmt --check | cargo fmt --check |
| Security audit | cargo audit + cargo deny + CodeQL + Gitleaks + TruffleHog | cargo clippy only |
| Fuzz testing | Yes (5 targets, 2min each) | No |
| SBOM generation | CycloneDX | No |
| Release automation | Multi-platform build + GPG signing + GitHub Release + crates.io publish | Multi-platform build + checksums |
| Lean4 verification | Yes (elan + lake build) | N/A |
| PGO/BOLT builds | Yes (weekly) | No |
| Documentation deploy | mdBook to GitHub Pages | No |
| Gitea CI | Yes (Forgejo-compatible) | No |

**Assessment:** Clawdius has a significantly more comprehensive CI/CD pipeline with security auditing, SBOM, formal verification, and PGO.

### Documentation

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| mdBook documentation | Yes (119 pages in SUMMARY) | No (markdown files only) |
| API reference | Yes (JSON-RPC, Rust API, Plugin API) | No |
| Enterprise docs | Yes (SSO, audit, compliance) | Yes (security maps) |
| Competitor comparison | Yes | Yes (PARITY.md) |
| Roadmap | Yes | Yes (6,430-line detailed roadmap) |
| Landing page | Yes (index.html) | No |

**Assessment:** Clawdius has a proper documentation site with mdBook. Claw Code has extensive but unorganized markdown docs.

### Enterprise

| Feature | Clawdius | Claw Code |
|---------|----------|-----------|
| SSO (SAML/OIDC) | Yes | No |
| Audit logging | Yes (multi-backend) | No |
| Compliance templates | Yes (SOC 2, HIPAA, GDPR) | No |
| Team management | Yes (23 permissions) | No (in-memory registries) |
| Self-hosted | Yes (Docker + bare metal) | Yes (Docker/Podman) |
| License | Apache 2.0 | MIT |

---

## Strengths and Weaknesses

### Clawdius Strengths vs Claw Code

1. **Formal verification** -- 209 Lean4 theorems provide mathematical guarantees
2. **Real sandboxing** -- 5 production backends vs heuristic container detection
3. **Messaging gateway** -- 9 platform adapters; Claw Code is CLI-only
4. **Code intelligence** -- Tree-sitter + LanceDB Graph-RAG; Claw Code has none
5. **Complete MCP server** -- Production jsonrpsee implementation vs registry stub
6. **Enterprise features** -- SSO, audit logging, compliance templates, 23 permissions
7. **CI/CD maturity** -- Security auditing, SBOM, fuzzing, PGO, Lean4 verification
8. **Documentation site** -- mdBook with 119 pages vs scattered markdown
9. **Encryption at rest** -- AES-256-GCM; Claw Code has none
10. **Vector database** -- LanceDB for semantic search; Claw Code has none

### Claw Code Strengths vs Clawdius

1. **Broader LLM support** -- xAI, DashScope, Ollama, OpenRouter, vLLM, llama.cpp
2. **Model-specific handling** -- Per-model parameter adaptation, reasoning stripping
3. **Prompt caching** -- Fingerprint-based with TTL and cache-break detection
4. **OAuth PKCE** -- Full flow with token refresh
5. **Tab completion** -- REPL has tab completion for commands and aliases
6. **Deterministic mock service** -- Reproducible E2E test scenarios
7. **Autonomous agent coordination** -- Lane events, policy engine, worker lifecycle
8. **Task/Team/Cron registries** -- Multi-agent orchestration framework
9. **Recovery recipes** -- Auto-recovery for common failure modes
10. **`unsafe_code = "forbid"`** -- Globally; Clawdius has 3 files with unsafe

### Shared Weaknesses

1. **Neither has browser automation** -- No Playwright/Puppeteer/headless
2. **Both lack published crates** -- Clawdius blocked on core publish; Claw Code `publish = false`
3. **Both have stale metrics in docs** -- Test counts, version numbers drift

---

## Target User Fit

| User Profile | Clawdius | Claw Code |
|-------------|----------|-----------|
| Enterprise security team | Strong fit -- sandboxing, SSO, audit | Weak -- no enterprise features |
| Solo developer | Good fit -- CLI + TUI | Strong fit -- lightweight, fast setup |
| Discord/Slack bot operator | Strong fit -- 9 platform adapters | Not supported |
| Multi-provider power user | Moderate -- missing xAI, DashScope | Strong fit -- broadest provider support |
| Formal verification researcher | Strong fit -- Lean4 proofs | Not applicable |
| Autonomous agent team | Weak -- no multi-agent coordination | Strong fit -- lane events, policy engine |
| Open source contributor | Good fit -- Apache 2.0, well-documented | Good fit -- MIT, good docs |

---

## Conclusion

Claw Code and Clawdius target overlapping but distinct segments of the AI coding assistant market. Claw Code excels as a Claude Code replacement with broadest-in-class LLM provider support and sophisticated autonomous agent coordination. Clawdius excels as an enterprise-grade, formally verified coding engine with real sandboxing, messaging gateway integration, and code intelligence.

The projects are complementary rather than directly competitive in most dimensions. A team needing Slack/Discord integration with sandboxed code execution would choose Clawdius; a solo developer wanting the broadest model access with autonomous agent workflows would choose Claw Code.

---

*Last updated: 2026-05-19*
