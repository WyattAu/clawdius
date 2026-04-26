# Full Competitive Comparison Matrix

**Document ID:** COMP-MATRIX-001
**Version:** 1.0.0
**Date:** 2026-04-26
**Methodology:** Source-code-level analysis of latest commits across 8 repositories

---

## 1. Edit Algorithm Comparison

| Competitor | Edit Strategies | Fuzzy Matching | Ambiguity Handling | Multi-Edit | Determinism |
|-----------|:--------------:|:--------------:|:-----------------:|:----------:|:-----------:|
| **Aider** | 8 formats (SEARCH/REPLACE, whole, udiff, patch, architect, context, editor variants) | Google DMP line-level with offset remapping; git cherry-pick as strategy; whitespace tolerance; quote normalization; ellipsis support | `str.count()` first-occurrence; `SearchTextNotUnique` error for udiff with more context; patch finds first match | No atomic multi-edit (sequential apply) | Post-processing fully deterministic; LLM + summarization non-deterministic |
| **Claude Code** | Edit (old→new), MultiEdit (atomic array), Write (full file) | **None** — exact string match only. LLM responsible for correct anchors. | Fails with "File content has changed" if old_string not found. | **Yes** — MultiEdit applies N edits to one file atomically | Deterministic edit application; non-deterministic LLM generation |
| **IronClaw** | write_file (full), apply_patch (search/replace) | Trailing whitespace normalization; smart/curly quote normalization; combined | **Rejects** if multiple matches and `replace_all=false`; read-before-edit guard; staleness detection (1s mtime) | No atomic multi-edit | Fully deterministic edit application |
| **OpenClaw** | Via agent tools (file ops through sandbox) | N/A — delegates to agent's tool system | N/A | N/A | N/A (general-purpose agent) |
| **OpenManus** | str_replace_editor (SWE-agent style), view, create, insert, undo | Tab expansion on comparisons; no fuzzy match | **Rejects** if old_str appears 0 or >1 times, reports all match line numbers | No atomic multi-edit | Deterministic |
| **Clawdius** | edit_cascade (5 strategies: exact, whitespace, semantic, regex, AST) | **Yes** — 5-strategy cascade with scoring | **Rejects** if multiple matches above threshold; reports locations | **Yes** — cascading multi-file edits in single tool call | Deterministic edit application; LLM non-deterministic |

### Analysis: Where Competitors Are Better

| Gap | Who Does It Better | What They Do | Clawdius Status |
|-----|:------------------:|--------------|:---------------:|
| **Atomic multi-edit** | Claude Code | `MultiEdit` applies N edits to one file in a single tool call, preventing partial application | Clawdius's `edit_cascade` handles multi-file but not atomic multi-edit within one file |
| **Git cherry-pick strategy** | Aider | Uses `git cherry-pick` to apply edits via git's merge algorithm — handles renames and moves | Clawdius does not use git merge for edit application |
| **Read-before-edit guard** | IronClaw | Hard requirement: must read file before editing; staleness detection with 1s mtime check | Clawdius does not enforce read-before-edit |
| **Staleness detection** | IronClaw | Rejects edit if file mtime changed >1s since read | Clawdius does not detect stale edits |

### Techniques Clawdius Should Adopt

1. **Atomic multi-edit per file** — A single tool call should accept `[{old, new}, {old, new}]` and apply all-or-nothing
2. **Read-before-edit guard** — Require file to have been read before allowing edits (prevent editing unseen files)
3. **Staleness detection** — Check file mtime before applying edits; reject if file changed since last read
4. **DMP with offset remapping** — When SEARCH text differs from file, compute diff between them and remap patch offsets (Aider's most sophisticated technique)

---

## 2. Repo-Map / Codebase Awareness

| Competitor | Algorithm | Scoring | Pruning | Caching | Incremental |
|-----------|:---------:|:-------:|:-------:|:-------:|:-----------:|
| **Aider** | tree-sitter AST extraction + NetworkX PageRank | Weighted personalized PageRank: identifier mentions ×10, descriptive names ×10, private ×0.1, common ×0.1, chat-file ×50, sqrt(num_refs) | Binary search on token count, 15% tolerance | 4-layer: SQLite tags cache (persistent), tree context cache, rendered tree cache, map result cache | mtime-based invalidation; tags cache keyed by file path |
| **Claude Code** | **None** — LLM-driven discovery via Glob/Grep tools | N/A | N/A | Prompt cache (1h TTL); Read deduplication | N/A — no index |
| **IronClaw** | N/A — LLM-driven via glob/grep tools | N/A | N/A | LRU response cache | N/A |
| **Clawdius** | tree-sitter AST extraction | Symbol scoring (definitions > references, shorter names > longer, recently modified > stale) | Token budget via binary search | In-memory map cache | mtime-based invalidation |

### Analysis

| Gap | Who Does It Better | What They Do | Clawdius Status |
|-----|:------------------:|--------------|:---------------:|
| **PageRank scoring** | Aider | Uses weighted personalized PageRank on file-level reference graph with 5 multipliers | Clawdius uses simpler symbol-level scoring without graph structure |
| **Identifier mention boosting** | Aider | Identifiers mentioned in user's chat message get ×10 weight; descriptive names (snake_case, ≥8 chars) get ×10 | Clawdius does not boost based on chat context |
| **Persistent cache** | Aider | SQLite-backed disk cache (`.aider.tags.cache.v{version}/`) survives process restarts | Clawdius's cache is in-memory only |
| **Token sampling** | Aider | For texts >200 chars, samples every Nth line and extrapolates token count (O(n/100)) | Clawdius tokenizes fully (O(n)) |
| **Special files priority** | Aider | ~175 known config files (pyproject.toml, Dockerfile, CI/CD) prepended before PageRank results | Clawdius does not have a special files list |

### Techniques Clawdius Should Adopt

1. **PageRank-based file ranking** — Build a file-level reference graph and use personalized PageRank (chat files get personalization boost)
2. **Chat-context boosting** — Identifiers mentioned in the user's message get scoring boost
3. **Persistent tag cache** — SQLite-backed cache for tree-sitter extractions (survives restarts)
4. **Token sampling** — For large files, sample every Nth line instead of full tokenization
5. **Special files list** — Prepend known config files (Dockerfile, Cargo.toml, .github/) to repo-map output

---

## 3. Context Management / Compaction

| Competitor | Strategy | Trigger | LLM Cost | Determinism | Preserves |
|-----------|:--------:|:-------:|:--------:|:-----------:|:---------:|
| **Aider** | Recursive head/tail summarization | `done_messages` exceed `max_chat_history_tokens` | Yes (weak model first, then main) | Non-deterministic (LLM-generated summary) | Tail N turns always preserved; head summarized |
| **Claude Code** | Auto-compaction with thrash detection | Context approaches limit | Yes (Haiku/fast model) | Non-deterministic | Images preserved in summarizer request; circuit breaker after 3 failures |
| **IronClaw** | Escalating: workspace dump → summarize → truncate | 80%/85%/95% of limit | Yes (for summarize) | Non-deterministic | 5 turns (summarize), 3 turns (truncate) |
| **OpenClaw** | Checkpointed compaction | `budget`, `overflow`, `manual`, `timeout-retry` | Yes (pluggable CompactionProvider) | Non-deterministic | Checkpoint files; max 25 per session; pluggable provider |
| **Clawdius** | LLM-based summarization via ContextCompactor | Configured threshold | Yes | Non-deterministic | Tail turns preserved |

### Analysis

| Gap | Who Does It Better | What They Do | Clawdius Status |
|-----|:------------------:|--------------|:---------------:|
| **Thrash detection** | Claude Code | Detects when context refills immediately after compaction (3 consecutive), stops instead of burning API calls | Clawdius does not detect compaction thrash loops |
| **Circuit breaker** | Claude Code | Stops after 3 consecutive compaction failures | Clawdius does not have a compaction circuit breaker |
| **Checkpointed snapshots** | OpenClaw | Saves pre-compaction JSONL to checkpoint files (max 25), enabling rollback | Clawdius does not checkpoint before compaction |
| **Pluggable compaction** | OpenClaw | `CompactionProvider` interface allows plugins to replace summarization pipeline | Clawdius has single compaction strategy |
| **Background summarization** | Aider | Runs in background thread to avoid blocking the main loop | Clawdius's compaction is synchronous |
| **Escalating severity** | IronClaw | 80% → workspace dump, 85% → summarize, 95% → truncate — graduated response | Clawdius has single threshold |

### Techniques Clawdius Should Adopt

1. **Compaction thrash detection** — If context refills to limit within N turns of compaction, stop and surface actionable error
2. **Compaction circuit breaker** — Stop after N consecutive compaction failures
3. **Background compaction** — Run summarization in a tokio task to avoid blocking
4. **Checkpointed snapshots** — Save pre-compaction messages to checkpoint files
5. **Escalating severity** — Try lighter strategies before heavy summarization

---

## 4. Model Routing / LLM Provider Chain

| Competitor | Routing | Failover | Circuit Breaker | Caching | Cost Tracking |
|-----------|:-------:|:--------:|:---------------:|:-------:|:-------------:|
| **IronClaw** | 13-dimension heuristic scorer (regex keyword matching) with 4-tier mapping (Flash/Standard/Pro/Frontier) + cascade escalation | Yes — `FailoverProvider` with cooldown (failure threshold + recovery timeout) | Yes — `CircuitBreakerProvider` (Closed/Open/HalfOpen, threshold=5, recovery=30s) | Yes — SHA-256 keyed in-memory LRU with TTL (tool calls never cached) | Per-invocation token counting |
| **Aider** | Per-model edit format assignment (`main_model.edit_format`) | `litellm` retry with exponential backoff | No explicit circuit breaker | Prompt cache warming via background thread (Anthropic-style `cache_control: ephemeral`) | Token counting per message |
| **Claude Code** | Single provider (Anthropic) with model selection | Bedrock/Vertex failover | Compaction circuit breaker (3 failures) | 1-hour prompt cache TTL; 5-minute option | Token tracking per session |
| **Clawdius** | ModelRouter with task-aware dispatch (7-model pricing table) | Yes — `with_retry_and_circuit` | Yes — `CircuitBreaker` | Yes — LLM response cache | `CostTracker` with per-model pricing |

### Analysis

| Gap | Who Does It Better | What They Do | Clawdius Status |
|-----|:------------------:|--------------|:---------------:|
| **13-dimension scoring** | IronClaw | Regex-based multi-dimensional prompt complexity analysis for model selection | Clawdius's ModelRouter is simpler — task-type dispatch, not prompt analysis |
| **Cascade escalation** | IronClaw | Try cheap model first; if response contains uncertainty signals ("I'm not sure"), escalate to primary | Clawdius does not have automatic escalation |
| **Prompt cache warming** | Aider | Background thread sends periodic pings to keep Anthropic prompt cache alive | Clawdius does not have cache warming |

### Techniques Clawdius Should Adopt

1. **Cascade escalation** — Try cheaper model first; if response contains uncertainty signals, escalate to primary
2. **Prompt cache warming** — Periodic background pings to keep provider-side prompt caches alive (especially Anthropic)

---

## 5. Security Model Comparison

| Competitor | Sandbox | Prompt Injection | Leak Detection | Permission Model | Formal Verification |
|-----------|:-------:|:----------------:|:--------------:|:----------------:|:------------------:|
| **IronClaw** | Docker (3 policies) + WASM (Wasmtime, capability-based) | Aho-Corasick multi-pattern + regex; boundary injection defense (zero-width space escaping) | `LeakDetector` scans outputs for secret patterns | Tool-level `ApprovalRequirement`; tool domain classification | **None** (fuzz targets only) |
| **Claude Code** | seccomp (Linux) + native macOS sandbox; network controls; credential scrubbing | N/A (relies on model alignment) | N/A | 3-tier settings hierarchy (enterprise > project > user); per-command Bash rules with wildcard support; auto-mode LLM classifier | **None** |
| **Aider** | **None** | **None** | **None** | **None** | **None** |
| **OpenClaw** | Docker sandbox (session/agent/shared scope) | N/A | N/A | DM pairing + allowlist + command gating + role-based | **None** |
| **Clawdius** | bubblewrap (13 RO mounts, no network, seccomp filter) | Aho-Corasick (30+ AC patterns) + regex (20+ patterns) | Regex-based (20+ leak patterns) | `can_generate/can_analyze/can_modify_files/can_execute/can_admin` 5-flag PermissionSet | **Lean4** (114 theorems, 8 proof domains) |

### Analysis

Clawdius is **significantly ahead** in security. No competitor has formal verification. Only IronClaw approaches Clawdius's defense-in-depth, but without proofs.

---

## 6. Tool System Comparison

| Competitor | Tool Count | Extension | MCP | Sandbox Integration | Atomic Multi-Edit |
|-----------|:----------:|:--------:|:---:|:-------------------:|:-----------------:|
| **IronClaw** | ~40+ | WASM tools (Wasmtime), MCP, LLM-driven tool creation | Yes (HTTP/SSE/stdio/Unix socket) | Docker + WASM capability-based | No |
| **Claude Code** | ~15 | Plugin system (commands/agents/skills/hooks); MCP with 4 transports | Yes (stdio/SSE/HTTP/WebSocket) | seccomp + macOS native | Yes (MultiEdit) |
| **Aider** | ~8 | No plugin system | No | No sandbox | No |
| **OpenClaw** | Plugin-driven | 118 plugins via npm; 32 lifecycle hooks; 35 register methods | Yes (stdio transport) | Docker (3 scope policies) | N/A |
| **OpenManus** | 18 | MCP dynamic tools | Yes (SSE + stdio) | Docker (basic) | No |
| **Clawdius** | 8 | No plugin system yet | Partial | bubblewrap (wired into ShellToolExecutor) | No |

### Techniques Clawdius Should Adopt

1. **MCP with 4 transports** — Claude Code supports stdio, SSE, HTTP, WebSocket; Clawdius has partial MCP
2. **Plugin lifecycle system** — OpenClaw's 32 hooks + 35 register methods is the gold standard
3. **LLM-driven tool creation** — IronClaw can create, compile, and install new WASM tools at runtime

---

## 7. Performance Optimization Comparison

| Competitor | Runtime | Caching Strategy | JSON Parsing | Regex | SIMD | io_uring |
|-----------|:-------:|:----------------:|:-----------:|:-----:|:----:|:--------:|
| **IronClaw** | Rust + Tokio | LRU response cache; Wasmtime compilation cache; LazyLock regex; per-tool rate limiting | Standard serde_json | LazyLock<Regex> | **None** | **None** |
| **Claude Code** | TypeScript + Node.js (native binary) | 1h prompt cache; Read dedup; Plugin cache; MCP deferred loading; LRU regex (128 patterns) | Standard JSON | LRU cache (128) | N/A | N/A |
| **Aider** | Python | 4-layer persistent cache (SQLite tags + tree context + rendered tree + map result); prompt cache warming; diskcache | Standard JSON | Standard re | N/A | N/A |
| **OpenClaw** | TypeScript + Node.js | WeakMap bindings cache (2000/4000 entries); inbound debouncing; throttled streaming | Standard JSON | Standard | N/A | N/A |
| **Clawdius** | Rust + Tokio | LLM response cache; LazyLock regex (5 instances) | Standard serde_json | LazyLock<Regex> | **None** | **None** |

### Techniques Clawdius Should Adopt

1. **simd-json or sonic-rs** — Zero-copy JSON parsing (IronClaw doesn't do this either, but it's a Rust-native optimization)
2. **Persistent tag cache** — SQLite-backed for tree-sitter extractions (survives restarts)
3. **Custom allocator** — `mimalloc` is already a dependency but not wired up

---

## 8. Session & Workspace Comparison

| Competitor | Multi-Codebase | Session Persistence | Resume | Cross-Platform Session | Unified Chat |
|-----------|:--------------:|:------------------:|:------:|:---------------------:|:------------:|
| **Aider** | No (single repo) | In-memory (no persistence) | No | N/A | N/A |
| **Claude Code** | No (single repo) | JSON transcript files on disk | Yes (UUID, `/resume`, fork, worktree-aware) | Remote bridge to claude.ai | N/A |
| **IronClaw** | No (single job) | PostgreSQL / libSQL | Yes | Via NearAI platform | N/A |
| **OpenClaw** | No (single workspace) | JSONL session files | Yes (4 DM scoping modes) | Yes (identity-linked cross-channel) | N/A |
| **Paperclip** | No (company→agent→project) | PostgreSQL (Drizzle ORM) | Yes | N/A | N/A |
| **Clawdius** | **Planned** (workspace with N projects) | HashMap (needs SQLite/Postgres/MariaDB) | Partial | **Planned** (via messaging gateway) | **Planned** (unified across projects) |

**Clawdius is the ONLY competitor planning multi-codebase workspace support with unified chat history.** This is a genuine differentiator.

---

## 9. Messaging / Remote Access

| Competitor | Platforms | Adapter Pattern | Session Binding | Response Chunking |
|-----------|:---------:|:---------------:|:---------------:|:-----------------:|
| **OpenClaw** | 25+ (Telegram, Discord, Slack, WhatsApp, Signal, Teams, Matrix, IRC, Nostr, iMessage, etc.) | Capability-based typed contract (~20 optional adapters per channel) | Composite key: `agent:{id}:{channel}:{peer}` with 4 DM scoping modes | Per-channel text limits; throttled streaming; chunkerMode |
| **Claude Code** | 1 (claude.ai bridge) | N/A | N/A | N/A |
| **Clawdius (planned)** | 9 (Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp, Rocket.Chat, Webhook) | `PlatformAdapter` trait | `platform:user_id` → session composite key | Per-platform max length; streaming chunks |

### Analysis

OpenClaw's 25+ platform support is the gold standard. Clawdius's 9-platform plan covers the most important ones. The `PlatformAdapter` trait design is sound. OpenClaw's capability-based contract is more sophisticated (each channel can optionally implement ~20 different adapters), but Clawdius's simpler trait is appropriate for a coding-focused tool.

---

## 10. Rigor & Determinism Summary

| Competitor | Edit Determinism | Context Determinism | Formal Verification | Test Count | Unsafe Code |
|-----------|:---------------:|:------------------:|:------------------:|:----------:|:-----------:|
| **Aider** | ✅ Post-processing deterministic | ❌ LLM summarization non-deterministic | ❌ None | 488 tests | N/A (Python) |
| **Claude Code** | ✅ Exact match deterministic | ❌ LLM compaction non-deterministic | ❌ None | Unknown (closed source) | N/A (TypeScript) |
| **IronClaw** | ✅ Fully deterministic | ❌ LLM summarization non-deterministic | ❌ None (fuzz only) | Unknown | Allows `unsafe` |
| **OpenClaw** | N/A | ❌ LLM-dependent | ❌ None | Unknown | N/A (TypeScript) |
| **Clawdius** | ✅ Post-processing deterministic | ❌ LLM compaction non-deterministic | ✅ **114 Lean4 theorems, 8 domains** | 902 tests | `#![deny(unsafe_code)]` |

**Clawdius is the ONLY competitor with formal verification.** This is the core differentiator.

---

## 11. Techniques to Adopt (Prioritized)

### High Priority (Directly improves quality)

| # | Technique | Source | Effort | Impact |
|---|----------|:------:|:------:|:------:|
| 1 | Atomic multi-edit per file | Claude Code MultiEdit | 2d | Prevents partial edit application |
| 2 | Read-before-edit guard | IronClaw | 1d | Prevents editing unseen files |
| 3 | Staleness detection (mtime) | IronClaw | 1d | Prevents editing stale files |
| 4 | Compaction thrash detection | Claude Code | 1d | Prevents API credit waste |
| 5 | Compaction circuit breaker | Claude Code | 0.5d | Prevents compaction failure loops |
| 6 | DMP with offset remapping | Aider | 3d | Handles LLM anchor drift |
| 7 | PageRank-based repo-map | Aider | 5d | Better file ranking |
| 8 | Chat-context boosting | Aider | 2d | Better context relevance |
| 9 | Persistent tag cache | Aider | 2d | Faster startup after restart |
| 10 | Background compaction | Aider | 2d | Non-blocking context management |

### Medium Priority (Polish / Differentiation)

| # | Technique | Source | Effort | Impact |
|---|----------|:------:|:------:|:------:|
| 11 | Cascade model escalation | IronClaw | 3d | Cost optimization |
| 12 | Prompt cache warming | Aider | 2d | Faster inference |
| 13 | Escalating compaction severity | IronClaw | 2d | Better compaction quality |
| 14 | Checkpointed compaction | OpenClaw | 2d | Compaction rollback |
| 15 | simd-json or sonic-rs | N/A (Rust optimization) | 1d | Faster JSON parsing |
| 16 | Special files priority | Aider | 1d | Config files always visible |
| 17 | Token sampling | Aider | 1d | Faster token estimation |

### Low Priority (Nice to Have)

| # | Technique | Source | Effort | Impact |
|---|----------|:------:|:------:|:------:|
| 18 | 13-dimension routing scorer | IronClaw | 5d | Smarter model selection |
| 19 | Plugin lifecycle system | OpenClaw | 3 weeks | Extensibility |
| 20 | WASM tool sandbox | IronClaw | 2 weeks | Plugin isolation |
| 21 | Dual-threshold budget system | Paperclip | 1 week | SaaS billing |
