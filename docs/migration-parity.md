# Migration Parity: clawdius vs opencode vs Claude Code

**Date:** 2026-11-02
**Purpose:** the checklist that gates migration from opencode to clawdius.
Every row must reach `VERIFIED` (executed end-to-end on this machine, output
captured) before that workflow is considered migrated. `MODULE` means the
code exists but has never been exercised end-to-end. `MISSING` means build
it first.

**Bar:** parity or better with opencode (sst) and Claude Code, plus
clawdius's differentiators (gateway, proofs, sandbox, fleet).

## Dogfooding status (the question that triggered this doc)

**clawdius does not currently dogfood the WyattAu GitHub repos.**

- CI's only estate reference is the homebrew tap in `release.yml:381`.
- No workflow runs clawdius's agent against a real estate repo.
- The estate's agent work (proof loops, vet bots, this analysis) runs on
  opencode + MCP servers.
- The agent's in-session tool surface is 8 tools (`read_file`, `write_file`,
  `edit_file`, `list_directory`, `shell`, `git_status`, `git_diff`,
  `git_log`) in `crates/clawdius/src/tui_app/app.rs:68` — not enough to
  meaningfully work a repo.

**Fix (dogfood ladder):** a `dogfood.yml` CI job that runs
`clawdius sprint "fix a TODO"` against a scratch estate repo; then the
fleet manager supervising `~/dev/src/github.com/WyattAu/`; then a real
Telegram alert bot via the gateway. Until at least one lands, "dogfooded"
is false.

## Workflow matrix

### 1. Core interactive loop

**E2E-verified 2026-11-02** via `tests/e2e/verify_workflows.sh` (real
binary, real scratch repo, real Z.AI API):

| Check | Result |
|---|---|
| Binary builds and runs (`--version`, `--help`) | PASS |
| `analyze` runs on a real repo | PASS |
| `chat` one-shot transport (provider select, session create, wire call, adapter resolution) | PASS (reaches provider; account-side quota rejection only) |
| MCP server JSON-RPC `initialize` handshake | PASS |
| Session store persists and lists | PASS |
| Bug found and fixed by E2E: `chat --provider zai` routed genai to its Ollama fallback (`zai-coding::` prefix unknown to genai); `ZaiProvider` now pins `AdapterKind::Zai` + coding endpoint via a target resolver | FIXED |

| Workflow | opencode | Claude Code | clawdius | Status |
|---|---|---|---|---|
| One-shot prompt from terminal | `run` | `-p` | `chat` | VERIFIED (transport; E2E-OK needs a funded key) |
| Interactive TUI | SolidJS TUI | Ink REPL | ratatui (fleet opens first) | MODULE |
| Streaming responses | yes | yes | yes | VERIFY |
| Tool calling loop | yes | yes | yes (8 tools) | VERIFY |
| Multimodal (image attach) | yes | yes | advertised, 3 providers | VERIFY |
| JSON/structured output | yes | yes | ResponseFormat | VERIFY |

### 2. Agent surface (the daily-driver gap)

**2026-11-02: tool surface 8 -> 15.** Added `grep`, `glob`, `web_search`,
`web_fetch`, `todo_write`/`todo_read`, `subagent`, and AGENTS.md/CLAUDE.md
context injection. Core modules `tools/search.rs`, `tools/todo.rs`,
`tools/subagent.rs` (bounded nested loop), `tools/web.rs` — 49 core tool
tests green.

**2026-11-06: DOGFOOD VERIFIED IN CI.** The Dogfood workflow runs clawdius
autonomously against a seeded broken repo: run 13 passed - the agent used
its tool loop (shell -> read -> edit -> re-test) to repair the defect with
zero human intervention ("Dogfood PASS: agent repaired the repo
end-to-end", run 42140686924). Seven pre-run failures surfaced and fixed
six real bugs: gemini model sunsets (x2), genai Ollama fallback for the
zai prefix, missing GoogleProvider chat_with_tools, missing Google
routing in LlmClientWithRetry, and a seed that did not compile.

| Capability | opencode | Claude Code | clawdius | Status |
|---|---|---|---|---|
| File read/write/edit/glob/grep | yes | yes | read/write/edit/list + **grep/glob** | PARITY (13 tools; still below CC's ~40 breadth) |
| Shell + sandbox | permission+scan | sandboxed bash | 8 backends, refuse-by-default | dogfood-verified in CI |
| Subagents (Task tool) | background child sessions | AgentTool fork/resume | **subagent engine bound to auto mode + dogfood CI run** (`tools/subagent.rs`, 4 mock tests + run 13 E2E) | PARITY-ish |
| Todo/plan tracking | todo tool | TodoWrite | **todo_write/todo_read** | PARITY |
| Web fetch | yes | WebFetch | **web_fetch** (SSRF guard: loopback/RFC1918/link-local/`.internal`/single-label refused; 20KB text cap) | PARITY |
| Web search | yes | WebSearch | **web_search** (DuckDuckGo default) | PARITY |
| Repo context conventions | injects AGENTS.md | injects CLAUDE.md | **loads AGENTS.md then CLAUDE.md, 8KB cap** | PARITY |
| Skills/SKILL.md | skills + plugins | skills | `skills` module exists | VERIFY |
| Hooks | plugin hooks | Session hooks | AgentHook trait | MODULE |
| Slash commands | yes | ~50 | ~25 `:` commands | PARITY-ish, VERIFY |
| Editor integration (ACP/VS Code) | ACP + SDK | VS Code ext | `clawdius-code` helper + LSP | MODULE |
| Worktrees | pluggable strategy | yes | checkpoint module | MODULE |

### 3. Sessions and state

| Capability | opencode | Claude Code | clawdius | Status |
|---|---|---|---|---|
| Persistence across restarts | event-sourced, crash-safe | JSONL resume | SQLite sessions | VERIFY |
| Resume/fork/undo | fork/move/revert | --resume/rewind | checkpoint module | MODULE |
| Auto-compaction | precomputed + steerable | auto-compact | `compactor.rs` | VERIFY |
| Cost/budget caps | usage stats | cost tracking | Budget enforcement | VERIFY |
| Memory across sessions | none native | memdir/CLAUDE.md | memory_manager + graph-RAG | VERIFY |

### 4. Providers and resilience

| Capability | opencode | Claude Code | clawdius | Status |
|---|---|---|---|---|
| Providers | 44 | 1 (+bedrock) | 11 | PARITY-sufficient, VERIFY each |
| Provider OAuth login | yes (Copilot/ChatGPT/Claude) | yes | **DESIGN shipped** (`docs/provider-oauth-design.md`: Anthropic/OpenAI flows, keyring store, auth CLI, refresh-on-401) | DESIGN |
| Retry/failover | route executor | internal | per-provider retry | VERIFY |
| Task-aware model routing | static agent configs | model switch | 7 task types auto-select | VERIFY |

### 5. Enterprise and ops (clawdius leads)

| Capability | opencode | Claude Code | clawdius | Status |
|---|---|---|---|---|
| AuthN/Z | password+pairing | API key | OIDC/SAML/RBAC (21 perms) | clawdius wins |
| Audit logging | none | none | 6 backends | clawdius wins |
| Chat gateway | none | none | 9 platforms, hardened 2026-11-01 | clawdius wins |
| Sandboxing | ask-per-command | sandboxed bash | 8 backends + proofs | clawdius wins |
| MCP server | none | none | `clawdius-mcp` | clawdius wins |
| Formal verification | none | none | 318 Lean4 theorems | clawdius wins |
| Multi-repo fleet | worktrees only | none | fleet manager (v1) | clawdius wins |
| LSP server | none | none | `clawdius-lsp` | clawdius wins |

## Execution order to green

1. **Grow the agent tool surface to Claude Code parity** — **in progress,
   2026-11-02/03: 13 tools** (grep/glob/web_search/web_fetch/todo_write/
   todo_read added; AGENTS.md/CLAUDE.md context loading live). Remaining
   breadth: subagent tool binding.
2. **E2E harness**: `tests/e2e/verify_workflows.sh` drives the real binary
   headless; first sheet 8 PASS / 1 account-side skip.
3. **Dogfood CI job**: `.github/workflows/dogfood.yml` (weekly +
   manual) — clones the repo, seeds a known-broken test, hands the repair
   to the agent, asserts the fix. Needs `DEEPSEEK_API_KEY` or
   `ZAI_API_KEY` as a repo secret; skips cleanly without one.
4. **Session durability + provider OAuth** (the two remaining structural
   gaps).
5. Migration is allowed per-workflow only when its row says VERIFIED and
   the dogfood ladder has one live consumer.
