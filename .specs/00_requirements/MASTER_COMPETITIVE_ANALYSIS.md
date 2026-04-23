# Master Competitive Analysis: Clawdius vs. The Field

**Date:** 2026-04-23  
**Analyst:** Nexus (Principal Systems Architect)  
**Scope:** 8 competitor repositories, implementation-level deep-dives  
**Target:** Inform Clawdius v3.0 roadmap with specific reimplementation algorithms

---

## Executive Summary

We analyzed 8 agentic coding platforms at the source-code level. Clawdius occupies a unique position: **Rust-native, multi-tenant SaaS, defense/aerospace compliance, Lean4 formal verification**. No competitor offers this combination. However, several critical feature gaps must be addressed for Clawdius to compete effectively:

| Priority | Gap | Best Source | Effort | Impact |
|----------|-----|-------------|--------|--------|
| P0 | **Sandbox** (tool isolation) | IronClaw WASM, PicoClaw bubblewrap | 4-6 weeks | Defense contract blocker |
| P0 | **Repo-map** (codebase awareness) | Aider tree-sitter + PageRank | 2-3 weeks | Quality differentiator |
| P0 | **Prompt injection defense** | IronClaw safety crate | 2-3 weeks | Security differentiator |
| P1 | **Edit reliability** (fuzzy matching) | OpenCode 9-strategy cascade | 2-3 weeks | User experience |
| P1 | **Context compaction** (summarization) | OpenCode anchored + tail-turn | 2 weeks | Cost reduction |
| P1 | **Smart model routing** | IronClaw 13-dimension, PicoClaw complexity | 1-2 weeks | Cost optimization |
| P2 | **LSP integration** | OpenCode 25+ servers | 3-4 weeks | DX differentiator |
| P2 | **Hook/plugin system** | PicoClaw 4-type hooks, OpenClaw plugins | 3-4 weeks | Extensibility |
| P2 | **MCP support** | OpenManus dual-mode, OpenClaw | 2 weeks | Ecosystem |
| P3 | **Billing** | Paperclip budget governance, OpenCode Stripe | 3-4 weeks | SaaS revenue |

---

## Competitor Matrix (At-a-Glance)

| Feature | Claude Code | Aider | OpenCode | IronClaw | Paperclip | OpenClaw | PicoClaw | OpenManus |
|---------|-------------|-------|----------|----------|-----------|----------|----------|-----------|
| **Language** | TypeScript | Python | TypeScript | **Rust** | TypeScript | TypeScript | Go | Python |
| **LOC** | 512K | 20K | 217K | 474K | ~100K | 800K | 217K | ~5K |
| **Stars** | 75K+ | 35K+ | 25K+ | 30K+ | 57K+ | 100K+ | 28K+ | 55K+ |
| **Sandbox** | bubblewrap | None | None | **WASM** | Docker | Docker | bubblewrap | Docker/Daytona |
| **Repo-map** | None | **tree-sitter+PageRank** | None | None | None | None | None | None |
| **Edit algo** | SEARCH/REPLACE | SEARCH/REPLACE | **9-strategy cascade** | str_replace | N/A | N/A | N/A | str_replace |
| **Multi-LLM** | 4 providers | 200+ via litellm | 30+ via AI SDK | rig-core | 7 adapters | 40+ providers | 30+ providers | 5 providers |
| **Context mgmt** | 6 strategies | Repo-map | **Anchored compaction** | MemoryDoc | N/A | None | Budget-based | Sliding window |
| **Prompt defense** | ML classifier | None | None | **Aho-Corasick+regex** | None | 20+ checks | 47-pattern deny | None |
| **Formal verification** | None | None | None | None | None | None | None | None |
| **Multi-tenant** | No | No | No | **3-tier isolation** | **SaaS** | No | No | No |
| **MCP** | No | No | No | No | **Server** | **Dual** | Client | **Dual** |
| **A2A protocol** | No | No | No | No | No | No | No | Yes |
| **Billing** | Claude-only | No | Stripe | No | **Budget+Stripe** | No | No | No |

---

## 1. Claude Code (anthropics/claude-code) — 512K LOC TypeScript

### What It Is
Anthropic's official CLI agentic coding assistant. The gold standard for bash safety and context management. Claude-only model lock-in.

### Architecture Highlights

**OS-Level Sandboxing (bubblewrap)**
- Wraps ALL shell commands in bubblewrap namespaces
- `--die-with-parent`, `--unshare-ipc`, `--unshare-net` (when network disabled)
- Bind-mounts workspace directory read-write, system dirs read-only
- Falls back to macOS sandbox (sandbox-exec) on Darwin

**Bash Security (23 Checks)**
- tree-sitter bash parser for AST-level analysis (NOT regex)
- ML classifier (linear model) for bash intent detection
- 23 distinct security check categories including:
  - Destructive commands (rm, dd, format, mkfs)
  - Network exfiltration (curl, wget with pipe to stdin)
  - Privilege escalation (sudo, chmod, chown)
  - Container escape (docker exec)
  - Process manipulation (kill, pkill)
  - Shell injection ($(...), `...`, eval, source)

**Context Compaction (6 Strategies)**
1. **Prompt caching** — Anthropic's native cache_control on system prompts
2. **Sliding window** — Token-budget-based truncation
3. **Summarization** — LLM-based conversation summarization
4. **Semantic chunking** — Keeps tool_call/response pairs intact
5. **Tool result compression** — Truncates large file reads
6. **Priority eviction** — Keeps recent + system messages, evicts middle

**Coordinator Mode**
- Multi-agent pattern where a "coordinator" agent delegates to specialized sub-agents
- Sub-agents have their own context windows and tool sets
- Coordinator sees only summary results from sub-agents

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| tree-sitter bash parsing | Use `tree-sitter` + `tree-sitter-bash` Rust crate. Build AST visitor that walks `command`, `pipeline`, `list` nodes. | 1 week |
| ML classifier for bash | Train a small linear model (logistic regression) on ~10K labeled bash commands. Use `linfa` crate for inference. OR use heuristic rules (simpler, faster). | 1-2 weeks |
| Prompt caching | Implement cache_control headers for Anthropic API. For other providers, implement semantic deduplication of system prompts. | 3 days |
| 6-strategy compaction | Build a `CompactionStrategy` trait with 6 implementations. Run strategies in priority order, stop when context fits budget. | 2 weeks |

---

## 2. Aider (paul-gauthier/aider) — 20K LOC Python

### What It Is
Best-in-class repo-map for codebase awareness. Single-shot (no agent loop). Git-first design. SEARCH/REPLACE edit format.

### Architecture Highlights

**Repo-Map Algorithm (THE key innovation)**

The repo-map is Aider's killer feature. It provides the LLM with a compressed view of the entire codebase, enabling it to understand cross-file relationships without reading every file.

1. **tree-sitter AST parsing** — Parses each source file into an AST
2. **Tag extraction** — Extracts function definitions, class definitions, method definitions, imports
3. **PageRank scoring** — Builds a dependency graph of tags (who calls whom) and runs PageRank to identify the most "important" symbols
4. **Token budget binary search** — Binary search over which tags to include, targeting exactly the repo-map token budget
5. **Tree formatting** — Outputs tags in a tree structure grouped by file

```
repo_map.py:96-250 — RepoMap class
repo_map.py:350-420 — get_ranked_tags() — PageRank scoring
repo_map.py:450-500 — to_tree() — Tree formatting with budget
```

**SEARCH/REPLACE Edit Format**
```
<<<<<<< SEARCH
exact text to find (must be unique)
=======
replacement text
>>>>>>> REPLACE
```
- Fuzzy matching code EXISTS but is **disabled by early return on line 183** (dead code)
- Uses `difflib.SequenceMatcher` for potential fuzzy matching (never executed)

**Per-Model Optimization**
- Different edit formats for different models (SEARCH/REPLACE, whole-file, diff)
- 200+ models supported via litellm abstraction
- Model-specific system prompts optimized through extensive A/B testing

**Git-First Design**
- Every edit is automatically committed
- `/undo` command reverts last commit
- Branch management built into agent loop

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| tree-sitter repo-map | Use `tree-sitter` Rust crate with language-specific grammars. Build `Tag` extractor for Rust, Python, TypeScript, Go, Java. | 2 weeks |
| PageRank scoring | Build tag dependency graph (imports, function calls). Use `petgraph` crate for PageRank. | 3 days |
| Token budget binary search | Sort tags by PageRank score. Binary search for max tags that fit budget. | 1 day |
| SEARCH/REPLACE | Already partially implemented. Add uniqueness enforcement (exact match count == 1). | 2 days |
| Fuzzy matching | Implement Levenshtein-based fuzzy search as fallback when exact match fails. Use `strsim` crate. | 3 days |

**CRITICAL: The repo-map is Clawdius's biggest gap.** No other competitor has this algorithm, and it's the single most impactful feature for codebase-aware editing. Aider's implementation is well-documented and straightforward to port to Rust.

---

## 3. OpenCode (sst/opencode) — 217K LOC TypeScript

### What It Is
Feature-rich agentic IDE with the best edit algorithm and context compaction in the field. Effect TypeScript architecture. Built-in LSP integration.

### Architecture Highlights

**9-Strategy Fuzzy Edit Cascade**
OpenCode tries 9 different strategies to apply edits, falling through to the next on failure:

1. **BlockAnchor** (primary) — Uses surrounding context (anchor lines) + Levenshtein to locate the edit target
2. **SimilarAnchor** — Relaxed anchor matching with similarity threshold
3. **RegexSearch** — Regex-based search for the target block
4. **LineSearch** — Exact line-number-based targeting
5. **FunctionSearch** — Searches for function/class definitions
6. **ImportSearch** — Searches for import statements
7. **WholeFile** — Replaces entire file (last resort)
8. **CreateFile** — Creates new file
9. **DiffApply** — Applies unified diff format

```
packages/opencode/src/edit/strategies/ — 9 strategy files
packages/opencode/src/edit/cascade.ts — Cascade orchestrator
```

**Anchored Context Compaction**
- Preserves "tail turns" — the most recent tool_call/response pairs are NEVER compacted
- Anchors summarization around key tokens (function names, class names)
- Budget-aware: compacts only when context exceeds threshold
- Turn-level granularity: keeps complete turns intact rather than breaking them up

**Permission Engine (findLast semantics)**
- Rules are evaluated in reverse order (most recent rule wins)
- Bash commands grouped by arity (single-command vs. multi-command)
- Pattern-based: glob patterns for command matching
- Arity-aware: `git commit -m "msg"` is one command, `git commit && git push` is two

**LSP Integration (25+ servers)**
- Auto-downloads language servers based on file extensions
- Diagnostics fed back into agent context
- Go-to-definition, find-references exposed as tools
- Supports: TypeScript, Python, Rust, Go, Java, C++, Ruby, etc.

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| 9-strategy edit cascade | Build `EditStrategy` trait with 9 implementations. Cascade tries each in order, returns first success. Use `strsim::levenshtein` for fuzzy matching. | 3 weeks |
| Anchored compaction | Build `Compactor` that: (1) identifies tail turns, (2) summarizes older turns, (3) preserves anchor tokens. Use `tokenizers` crate for budget calculation. | 2 weeks |
| Permission engine | Build rule engine with `findLast` semantics (reverse iteration, first match wins). Pattern matching via `glob` crate. | 1 week |
| LSP integration | Use `tower-lsp` crate for client. Auto-launch servers via `which` detection. Feed diagnostics as tool results. | 4 weeks |

**The 9-strategy edit cascade is the most robust edit algorithm in the field.** It's complex but dramatically improves edit reliability. Should be a P1 priority after sandbox and repo-map.

---

## 4. IronClaw (nearai/ironclaw) — 474K LOC Rust ⚠️ CLOSEST COMPETITOR

### What It Is
OpenClaw-inspired Rust implementation focused on privacy and security. **Our closest competitor** — also in Rust, also focused on enterprise security. WASM sandbox is the most sophisticated tool isolation in the field.

### Architecture Highlights

**WASM Component Model Sandbox (`wit/tool.wit`)**
The most sophisticated tool isolation mechanism in any agentic platform:

```wit
// Host-provided capabilities (the ONLY ways a tool can interact with outside world)
interface host {
    log: func(level: log-level, message: string);       // Rate-limited: 1000/execution, 4KB/msg
    now-millis: func() -> u64;
    workspace-read: func(path: string) -> option<string>; // No "..", relative paths only
    http-request: func(method, url, headers-json, body, timeout-ms) -> result<http-response, string>;
    tool-invoke: func(alias: string, params-json: string) -> result<string, string>;
    secret-exists: func(name: string) -> bool;            // Check existence, NEVER read values
}
```

Key design principles:
- **All capabilities are opt-in** — default: no access to anything
- **Secrets are NEVER exposed to WASM** — credentials injected at host boundary during HTTP requests
- **Tool indirection** — WASM invokes tools by alias, not real name
- **Output scanning** — All outputs scanned for secret leakage before returning to WASM

**Safety Crate (`crates/ironclaw_safety/`)**

1. **Sanitizer** (`sanitizer.rs`, 725 lines) — Aho-Corasick + regex for prompt injection detection:
   - Direct instruction injection: "ignore previous", "disregard", "forget everything"
   - Role manipulation: "you are now", "act as", "pretend to be"
   - System message injection: "system:", "<|system|>"
   - Boundary manipulation: zero-width spaces, Unicode homoglyphs
   - Each pattern has severity (Low/Medium/High/Critical) and description

2. **Leak Detector** (`leak_detector.rs`, 1499 lines) — Dual-point scanning:
   - **Before outbound requests** — Prevents WASM from exfiltrating secrets
   - **After responses/outputs** — Prevents accidental exposure
   - Actions: Block (critical), Redact ([REDACTED]), Warn (log only)
   - Uses Aho-Corasick for fast multi-pattern matching

3. **Policy Engine** (`policy.rs`, 535 lines) — Rule-based content policy:
   - Regex-based rules with severity and action
   - PolicyAction: Allow, Block, Flag, Redact, RequireApproval
   - Composable: multiple policies can be combined

**Engine v2 — 5 Primitives**

The engine unifies ~10 separate abstractions around 5 primitives:

| Primitive | Replaces | Description |
|-----------|----------|-------------|
| **Thread** | Session + Job + Routine + Sub-agent | Unit of work |
| **Step** | Agentic loop iteration + tool calls | Unit of execution |
| **Capability** | Tool + Skill + Hook + Extension | Unit of effect |
| **MemoryDoc** | Workspace memory blobs | Unit of durable knowledge |
| **Project** | Flat workspace namespace | Unit of context |

**Capability System**

```rust
// 4 privilege tiers (totally ordered)
pub enum ToolTier {
    ReadOnly,       // echo, time, json, memory_search (no side effects)
    Stateful,       // read_file, list_dir (creates/reads local state)
    Privileged,     // shell, file_write, http (write operations, external effects)
    Administrative, // routine_*, tool_install, skill_*, secret_* (system-level, never autonomous)
}
```

- **Capability Lease** — Time-bounded grants with automatic revocation
- **Policy Engine** — Rule-based access control for capabilities
- **Lease Gate** — Authorization checks before capability execution
- **Lease Planner** — Plans which capabilities to grant based on thread type

**AUTONOMOUS_TOOL_DENYLIST** — 17 actions that can NEVER run autonomously:
```
routine_create, routine_update, routine_delete, routine_fire,
event_emit, create_job, job_prompt, restart,
tool_install, tool_auth, tool_activate, tool_remove, tool_upgrade,
skill_install, skill_remove, secret_list, secret_delete
```

**3-Tier Tenant Isolation**
- `TenantScope` — Per-tenant data isolation
- `SystemScope` — System-level operations (admin only)
- `AdminScope` — Administrative operations

**Testing Infrastructure**
- 5,272 tests including:
  - LLM trace replay tests (record LLM interactions, replay for regression)
  - Fuzz testing (`crates/ironclaw_safety/fuzz/`)
  - Snapshot testing with `insta` crate
  - Integration tests with `testcontainers`

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| WASM sandbox | Use `wasmtime` + `wasmtime-wasi` (already in Cargo.toml!). Define WIT interface for tool sandbox. Implement host capabilities. | 4-6 weeks |
| Safety crate (sanitizer) | Port Aho-Corasick patterns + regex patterns. Use `aho-corasick` + `regex` crates. Same severity/action model. | 2 weeks |
| Leak detector | Dual-point scanning at sandbox boundary. Same Aho-Corasick approach. Block/Redact/Warn actions. | 1 week |
| Capability lease system | Time-bounded grants with automatic revocation. `LeaseManager` with `tokio::time::sleep` for expiry. | 2 weeks |
| Tool tier classification | Port `classify_tool_tier()` logic. Same 4-tier model. Denylist for autonomous actions. | 3 days |
| LLM trace replay | Record LLM request/response pairs to JSONL. Replay in tests. Use `insta` for snapshot comparison. | 1 week |

**CRITICAL: IronClaw's WASM sandbox is the gold standard for tool isolation.** It's exactly what defense/aerospace customers need. However, it's complex. Consider a phased approach:
1. Phase 1: Docker/bubblewrap sandbox (simpler, like Claude Code/PicoClaw)
2. Phase 2: WASM sandbox for high-security environments

---

## 5. Paperclip — TypeScript Multi-Agent Orchestration Platform

### What It Is
NOT a coding assistant — it's a **multi-agent orchestration control plane**. Heartbeat-based model where agents wake periodically, check for tasks, work, exit. SaaS with billing.

### Architecture Highlights

**Heartbeat Model**
- Agents don't run continuously — they wake on a schedule (heartbeat interval)
- Each heartbeat: check for pending tasks → pick up task → execute → exit
- State persisted between heartbeats in PostgreSQL
- `heartbeat.ts` (5408 lines!) — the core orchestration service

**Adapter System (7 adapters)**
```
packages/adapters/
├── claude-local/      # Claude CLI subprocess
├── codex-local/       # Codex CLI subprocess
├── cursor-local/      # Cursor IDE integration
├── gemini-local/      # Gemini CLI subprocess
├── openclaw-gateway/  # OpenClaw gateway
├── opencode-local/    # OpenCode integration
└── pi-local/          # Pi integration
```
- Each adapter shells out to a CLI tool as a child process
- Standardized interface: `AdapterExecutionResult`, `UsageSummary`
- Session codec for serializing/deserializing agent state

**Git Worktree Execution**
- Each agent execution gets its own `git worktree`
- Parallel execution: multiple agents can work on different branches simultaneously
- Worktree cleanup after execution completes
- `dev-runner-worktree.ts` — worktree lifecycle management

**Budget & Approval Governance**
- Per-agent monthly budget in cents (`budgetMonthlyCents`)
- Budget enforcement scope: per-company, per-agent
- Approval flows for expensive operations
- Cost tracking per execution (`costService`)

**Plugin System**
- Plugins run in a sandboxed runtime (`plugin-runtime-sandbox.ts`)
- JSON-RPC communication between host and plugins
- Plugin lifecycle: load → validate → install → activate → run
- Capability validation for plugins
- Event bus for inter-plugin communication

**MCP Server**
- Exposes Paperclip APIs as MCP tools
- Other agents can use Paperclip as a tool provider

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| Heartbeat scheduling | Use `tokio-cron-scheduler` for periodic agent wake-up. Persist state in PostgreSQL. | 1 week |
| Git worktree execution | Use `git worktree add`/`remove` commands. Each sprint gets its own worktree. | 3 days |
| Budget governance | Per-tenant budget tracking in DB. Middleware checks budget before LLM calls. | 1 week |
| Stripe billing | Use `stripe-rs` crate. Webhook handler for payment events. Usage metering. | 3 weeks |

---

## 6. OpenClaw — 800K LOC TypeScript (General-Purpose Agent Framework)

### What It Is
The most full-featured open-source agent framework. 20+ messaging channels, 40+ LLM providers, 118 extension plugins. NOT a coding assistant — general-purpose.

### Architecture Highlights

**Plugin System (JSON5 Manifests)**
- Plugins declare capabilities via `skill.json` (JSON5 format)
- 27 lifecycle hooks in 3 patterns:
  - **Void hooks**: `onLoad`, `onUnload`, `onDestroy` (fire-and-forget)
  - **Modifying hooks**: `beforeToolCall`, `afterToolCall` (can modify request/response)
  - **Claiming hooks**: `onMessage` (can claim a message, preventing other hooks)
- Dependency injection via hook parameters
- Hot-reload support

**Docker Sandbox**
- Scope modes: `session` (per-session), `agent` (per-agent lifetime), `shared` (persistent)
- Filesystem bridge: sandbox can read/write workspace files via host bridge
- Network policies: allow/deny lists for HTTP access

**Sub-Agent Delegation**
- Depth limits (max nesting level)
- Orphan recovery: if parent dies, children are adopted or terminated
- Resource limits per sub-agent

**Security Audit (20+ categories)**
- Prompt injection detection
- Permission escalation checks
- Resource exhaustion prevention
- Network exfiltration detection

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| Plugin lifecycle (void/modifying/claiming) | Define `HookPattern` enum. `VoidHook`, `ModifyingHook<T>`, `ClaimingHook` traits. Registry dispatches by pattern type. | 2 weeks |
| Docker sandbox with scope modes | Use `bollard` crate. `SandboxScope` enum (Session/Agent/Shared). Filesystem bridge via volume mounts. | 3 weeks |
| Sub-agent depth limits | Pass `depth: usize` through agent spawning. Decrement on each level. Reject at depth == 0. | 2 days |

---

## 7. PicoClaw (sipeed/picoclaw) — 217K LOC Go

### What It Is
Ultra-lightweight AI agent for resource-constrained hardware ($10 RISC-V boards). General-purpose chat agent (NOT coding assistant). Best documentation of bubblewrap sandboxing and hook system design.

### Architecture Highlights

**Bubblewrap Sandbox (Fail-Closed Design)**
```go
// If bwrap not found, return ERROR (not fallback)
func applyPlatformIsolation(cmd *exec.Cmd, ...) error {
    bwrapPath, err := exec.LookPath("bwrap")
    if err != nil {
        return fmt.Errorf("linux isolation requires bwrap and does not fall back automatically: %w", err)
    }
    // Build bwrap args
    bwrapArgs := []string{
        "bwrap", "--die-with-parent", "--unshare-ipc",
        "--proc", "/proc", "--dev", "/dev",
    }
    // Add mount rules (ro for system, rw for workspace)
    cmd.Path = bwrapPath
    cmd.Args = bwrapArgs
}
```

**47-Pattern Exec Deny List**
Comprehensive regex-based command blocking:
- Destructive: `rm -rf`, `dd if=`, `format`, `mkfs`
- Injection: `$()`, backticks, `eval`, `source`
- Escalation: `sudo`, `chmod`, `chown`
- Exfiltration: `curl | sh`, `wget | bash`
- Network: `ssh`, `docker run/exec`
- Persistence: `git push`, `npm install -g`
- System: `shutdown`, `reboot`, `fork bomb`

**Smart Model Routing (Complexity-Based)**
```go
func (r *Router) SelectModel(msg string, history []Message, primaryModel string) (model string, usedLight bool, score float64) {
    features := ExtractFeatures(msg, history)
    score = r.classifier.Score(features)
    if score < r.cfg.Threshold { // Default: 0.35
        return r.cfg.LightModel, true, score
    }
    return primaryModel, false, score
}
```

Feature weights:
| Signal | Weight |
|--------|--------|
| Token > 200 (~600 chars) | 0.35 |
| Code block present | 0.40 |
| Tool calls > 3 (recent) | 0.25 |
| Conversation depth > 10 | 0.10 |
| Attachments present | 1.00 (hard gate) |

**4-Type Hook System**
```go
type EventObserver interface { OnEvent(ctx, evt) error }
type LLMInterceptor interface { BeforeLLM(ctx, req), AfterLLM(ctx, resp) }
type ToolInterceptor interface { BeforeTool(ctx, call), AfterTool(ctx, result) }
type ToolApprover interface { ApproveTool(ctx, req) (ApprovalDecision, error) }
```

Hook actions: `continue`, `modify`, `respond`, `deny_tool`, `abort_turn`, `hard_abort`

**SubTurn Orchestration**
- Parent-child turn relationships with depth limits (max 3)
- Concurrency control (max 5 concurrent sub-turns)
- Token budget sharing across sub-turns
- Async result delivery via channels
- Orphan detection (parent died before child returned)

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| Bubblewrap sandbox | Use `std::process::Command` to wrap shell commands with `bwrap`. Same fail-closed philosophy. Port 47 deny patterns to Rust regex. | 1 week |
| Smart model routing | Port `RuleClassifier` + `Features` struct. Same weight system. Use `regex` crate for code block counting. | 3 days |
| 4-type hook system | Define `EventObserver`, `LLMInterceptor`, `ToolInterceptor`, `ToolApprover` traits. `HookManager` with priority ordering. `tokio::time::timeout` for enforcement. | 2 weeks |
| SubTurn orchestration | `SubTurnConfig` with depth/concurrency/timeout limits. `tokio::spawn` with `oneshot` for async results. | 1 week |

---

## 8. OpenManus (mannaandpoem/OpenManus) — ~5K LOC Python

### What It Is
Classic ReAct pattern implementation. Minimal codebase, easy to understand. Good reference for basic agent architecture. SWE-style str_replace_editor.

### Architecture Highlights

**3-Level Agent Hierarchy**
```
BaseAgent (state machine, memory, run loop)
└── ReActAgent (think() + act() abstract methods)
    └── ToolCallAgent (LLM function-calling implementation)
        ├── Manus (general-purpose: MCP + browser)
        ├── SWEAgent (Bash + StrReplaceEditor)
        ├── BrowserAgent (Playwright)
        ├── MCPAgent (MCP tools only)
        ├── SandboxManus (Daytona sandbox)
        └── DataAnalysis (Python + charts)
```

**str_replace_editor**
- Commands: `view`, `create`, `str_replace`, `insert`, `undo_edit`
- Uniqueness enforcement: `old_str` must appear exactly once
- Output truncation: 16K max per response
- Undo history: `DefaultDict[Path, List[str]]` stores full file contents before each edit
- FileOperator abstraction: switches between local and sandbox file operations

**ReAct Loop**
1. `think()` — Call LLM with function calling, get tool_calls
2. `act()` — Execute each tool_call, collect results
3. Repeat until terminate tool called, max_steps reached, or stuck detected
4. Stuck detection: last N assistant messages have identical content

**MCP Dual-Mode**
- **Client**: Connects to remote MCP servers (SSE + stdio transports)
- **Server**: Exposes tools via MCP protocol (FastMCP)
- Tool discovery: `session.list_tools()` after connecting
- Name sanitization: `[a-zA-Z0-9_-]`, max 64 chars

**PlanningFlow**
- LLM generates a plan with typed steps (`[SEARCH]`, `[CODE]`, `[MANUS]`)
- Steps routed to specialized agents based on type tags
- Sequential step execution with plan context

### Reimplementation Guidance for Clawdius

| Feature | Approach | Effort |
|---------|----------|--------|
| str_replace_editor with undo | Port uniqueness check, undo history, output truncation. `HashMap<PathBuf, Vec<String>>` for undo stack. | 3 days |
| MCP client | Use `rmcp` crate. SSE + stdio transports. Tool discovery + name sanitization. | 1 week |
| MCP server | Use `axum` + JSON-RPC handler. Register Clawdius tools as MCP tools. | 1 week |
| PlanningFlow | Plan struct with typed steps. Agent routing by step type. Sequential execution. | 1 week |

---

## Clawdius Implementation Roadmap

### Phase 1: Foundation (Weeks 1-4) — Defense Contract Blockers

#### Week 1-2: Sandbox
```
Priority: P0 (defense contract blocker)
Approach: Phased implementation
```

**Phase 1a: Bubblewrap sandbox (Week 1)**
- Port PicoClaw's fail-closed bubblewrap wrapper to Rust
- 47-pattern deny list (port regex patterns from PicoClaw)
- Workspace restriction (path traversal prevention, symlink resolution)
- Output truncation (10KB max per command)
- Integration with existing `exec_tool` in Clawdius

**Phase 1b: Docker sandbox (Week 2)**
- Use `bollard` crate for Docker API
- Container lifecycle: create → exec → cleanup
- Resource limits: memory, CPU, network (none/bridge)
- File I/O via tar stream (like OpenManus)
- Per-sprint container isolation

#### Week 2-3: Repo-Map
```
Priority: P0 (quality differentiator)
Source: Aider's algorithm (best-documented)
```

1. **tree-sitter tag extraction** — Parse files into ASTs, extract function/class/method/import definitions
2. **PageRank scoring** — Build dependency graph, rank symbols by importance
3. **Token budget binary search** — Select top-ranked tags that fit budget
4. **Tree formatting** — Group by file, output in tree structure
5. **Integration** — Inject repo-map into system prompt for each sprint

**Rust crates needed:** `tree-sitter`, `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, `petgraph`

#### Week 3-4: Prompt Injection Defense
```
Priority: P0 (security differentiator)
Source: IronClaw's safety crate (most comprehensive)
```

1. **Sanitizer** — Aho-Corasick + regex for injection pattern detection
2. **Leak detector** — Dual-point scanning at sandbox boundary
3. **Policy engine** — Rule-based content policy with Allow/Block/Flag/Redact actions
4. **Boundary neutralization** — Zero-width space removal, Unicode homoglyph detection

**Rust crates needed:** `aho-corasick`, `regex` (already in Cargo.toml)

### Phase 2: Quality (Weeks 5-8)

#### Week 5-6: Edit Reliability
```
Priority: P1 (user experience)
Source: OpenCode's 9-strategy cascade
```

1. **EditStrategy trait** — Common interface for all strategies
2. **9 implementations** — BlockAnchor, SimilarAnchor, RegexSearch, LineSearch, FunctionSearch, ImportSearch, WholeFile, CreateFile, DiffApply
3. **Cascade orchestrator** — Try each strategy in order, return first success
4. **Fallback chain** — If all fail, report error with diagnostics

#### Week 6-7: Context Compaction
```
Priority: P1 (cost reduction)
Source: OpenCode's anchored compaction + Claude Code's 6 strategies
```

1. **Tail-turn preservation** — Never compact the last N tool_call/response pairs
2. **Anchored summarization** — Preserve key tokens (function names, class names)
3. **Budget-aware** — Only compact when context exceeds threshold
4. **Strategy selection** — Choose strategy based on context type (conversation vs. sprint)

#### Week 7-8: Smart Model Routing
```
Priority: P1 (cost optimization)
Source: PicoClaw's complexity-based scoring + IronClaw's 13-dimension scorer
```

1. **Feature extraction** — Token count, code blocks, tool calls, conversation depth, attachments
2. **Rule-based classifier** — Weighted scoring with configurable threshold
3. **Model selection** — Route to light model (fast/cheap) or heavy model (capable/expensive)
4. **Integration** — Apply routing before each LLM call in sprint engine

### Phase 3: Differentiation (Weeks 9-14)

#### Week 9-10: Hook/Plugin System
```
Priority: P2 (extensibility)
Source: PicoClaw's 4-type hooks + OpenClaw's plugin lifecycle
```

1. **Hook traits** — EventObserver, LLMInterceptor, ToolInterceptor, ToolApprover
2. **HookManager** — Priority ordering, timeout enforcement, clone-before-modify
3. **Hook actions** — continue, modify, respond, deny_tool, abort_turn, hard_abort
4. **Plugin loader** — JSON manifest, lifecycle management, hot-reload

#### Week 10-11: MCP Support
```
Priority: P2 (ecosystem)
Source: OpenManus dual-mode
```

1. **MCP client** — SSE + stdio transports, tool discovery, name sanitization
2. **MCP server** — Expose Clawdius tools as MCP tools
3. **Integration** — MCP tools available in sprint engine alongside built-in tools

#### Week 12-14: LSP Integration
```
Priority: P2 (DX differentiator)
Source: OpenCode's auto-launch system
```

1. **LSP client** — Use `tower-lsp` crate
2. **Auto-launch** — Detect language servers by file extension
3. **Diagnostics** — Feed LSP diagnostics into agent context
4. **Tool exposure** — go-to-definition, find-references as tools

### Phase 4: Revenue (Weeks 15-18)

#### Week 15-18: Billing & SaaS
```
Priority: P3 (revenue)
Source: Paperclip's budget governance + OpenCode's Stripe integration
```

1. **Per-tenant budget** — Monthly budget tracking in PostgreSQL
2. **Usage metering** — Per-sprint token tracking, cost calculation
3. **Stripe integration** — `stripe-rs` crate, webhook handler
4. **Approval flows** — Require approval for expensive operations

---

## Testing Strategy (From IronClaw's Excellence)

### LLM Trace Replay
Record all LLM request/response pairs during sprints. Replay in tests for regression detection. This is IronClaw's most innovative testing approach.

```
1. During sprint: Write each LLM call to JSONL file
2. In tests: Load JSONL, replay against current code
3. Compare outputs with recorded outputs (insta snapshots)
4. Any difference = potential regression
```

### Fuzz Testing
- Fuzz the sanitizer with random inputs (already have `fuzz/` dir)
- Fuzz the leak detector with random HTTP payloads
- Fuzz the edit cascade with random file contents

### Snapshot Testing
- Use `insta` crate for snapshot testing
- Snapshots for: tool outputs, edit results, repo-maps, compaction results

---

## What Clawdius Uniquely Offers (No Competitor Has)

1. **Lean4 formal verification** — Mathematical proofs of algorithm correctness. No competitor even attempts this.
2. **Rust-native with `#![deny(unsafe_code)]`** — Memory safety guarantees. Only IronClaw is also Rust, but it allows unsafe.
3. **Multi-tenant SaaS architecture** — Paperclip has SaaS but no Rust. IronClaw has tenant isolation but no SaaS billing.
4. **Defense/aerospace compliance** — No competitor targets this market specifically.
5. **The combination** — No single competitor offers Rust + formal verification + SaaS + defense compliance.

This is Clawdius's moat. The competitive analysis shows us WHAT to build (features from competitors), but our DIFFERENTIATION is HOW we build it (formally verified, memory-safe, compliant).

---

## Appendix: File Path Reference

| Competitor | Repo Path | Key Files |
|-----------|-----------|-----------|
| Claude Code | `/home/wyatt/dev/competitor-analysis/claude-code-leak/` | bash security, context compaction |
| Aider | `/home/wyatt/dev/competitor-analysis/aider/` | `aider/repo_map.py`, `aider/editblock.py` |
| OpenCode | `/home/wyatt/dev/competitor-analysis/opencode/` | `packages/opencode/src/edit/`, `packages/opencode/src/compaction/` |
| IronClaw | `/home/wyatt/dev/competitor-analysis/ironclaw/` | `crates/ironclaw_safety/`, `crates/ironclaw_engine/`, `wit/tool.wit` |
| Paperclip | `/home/wyatt/dev/competitor-analysis/paperclip/` | `server/src/services/heartbeat.ts`, `packages/adapters/` |
| OpenClaw | `/home/wyatt/dev/competitor-analysis/openclaw/` | Plugin system, sandbox |
| PicoClaw | `/home/wyatt/dev/competitor-analysis/picoclaw/` | `pkg/isolation/`, `pkg/tools/shell.go`, `pkg/routing/`, `pkg/agent/hooks.go` |
| OpenManus | `/home/wyatt/dev/competitor-analysis/OpenManus/` | `app/agent/`, `app/tool/str_replace_editor.py`, `app/tool/mcp.py` |
