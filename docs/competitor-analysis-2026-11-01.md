# Competitive Analysis: Clawdius vs the Field

**Date:** 2026-11-01
**Method:** Full clones of each competitor, per-module code reading by four
independent analysis passes. Snapshot dates below are the upstream commits
analyzed, not clawdius's HEAD.

**Verdict up front:** Clawdius is *not behind* on engine fundamentals. It is
behind in two specific areas: (1) gateway robustness engineering, where
OpenClaw's adapters encode years of transport-hardening patterns clawdius
lacks, and (2) provider-layer resilience (circuit breaking, cascading,
declarative registries), where ironclaw and picoclaw are ahead. opencode is
ahead on session durability and tool-safety engineering, not on features
clawdius users would notice first.

## Competitors analyzed

| Repo | Snapshot | Language | Scale | What it is |
|---|---|---|---|---|
| sst/opencode | v2.2.5, current | TS (Bun) | ~1.03M LOC, 5.5k files | Coding agent harness (TUI/GUI/server) |
| OpenClaw | 2026-04-23 | TS | ~2.5M LOC, 12.2k TS files | Personal AI gateway, 23 chat channels |
| ironclaw | 2026-09-09 | Rust | ~408k LOC, 7 crates | Rust personal assistant (NEAR AI) |
| picoclaw | 2026-04-22 | Go | ~91k LOC | <$10-hardware assistant, 17+ channels |
| OpenManus | 2026-01-04 | Python | ~12k LOC | ReAct agent framework reference |
| paperclip | 2026-04-22 | TS | ~236k LOC | Multi-agent org/governance layer |
| claude-code-leak | 2026-03-31 | TS | ~512k LOC | Leaked Claude Code internals |

---

## 1. opencode (sst/opencode)

### Architecture
Bun + TypeScript on Effect-TS 4 with a custom DI graph. No Go anymore. Key
packages by LOC: `packages/app` (219k, SolidStart GUI), `packages/core`
(206k, all domain logic), `packages/tui` (114k), `packages/ai` (87k,
hand-written LLM SDK: 44 providers, 48 wire protocols), `packages/codemode`
(33k, hand-written sandboxed JS interpreter).

### Per-module highlights (files clawdius should study)

| File | Why it matters |
|---|---|
| `packages/core/src/shell/scan.ts` (2,591 L) | Best-in-class bash/zsh/PowerShell decomposition into per-resource permission prefixes with an explicit `OpaqueReason` taxonomy. Direct model for clawdius sandbox command analysis. |
| `packages/codemode/src/interpreter/` | From-scratch sandboxed JS interpreter with inter-step resource budgets (string/array/promise/depth caps). Reference for multi-tool turns under budget. |
| `packages/core/src/session/compaction.ts` (1,246 L) | Production compaction: measured vs estimated tokens, background precompute before the limit, steering arbitration at safe boundaries. |
| `packages/core/src/session/inbox.ts` + `projector.ts` | Durable prompt admission (one row, one wake) and event-sourced read models. Crash-safe agent queues done right. |
| `packages/core/src/session/runner/step.ts` | "One LLM call per durable step" loop. The anti-pattern antidote to in-memory tool loops. |
| `packages/core/src/tool/plugin/subagent.ts` | Background subagent contract with follow-up-by-sessionID; no polling discipline. |
| `packages/core/src/mcp/oauth.ts` | OAuth 2.1 for remote MCP servers. clawdius's MCP client lacks this. |
| `packages/ai/src/route/executor.ts` | Uniform retry/rate-limit/failure classification across all 44 providers via `retry-after` header parsing. |
| `packages/core/src/schedule.ts` (882 L) | Durable cron scheduling of agent runs (Effect Cron, stored checks). |
| `packages/http-recorder/` | HTTP/WS cassette recorder for deterministic provider tests. Test-infra worth copying. |
| `packages/server/src/routes.ts` | Whole backend as a declarative LayerNode graph with runtime-swappable implementations (simulation, workerd). |
| `packages/gui-plugins/src/ssh/` | Remote dev over SSH with custom askpass sidechannel. |

### Parity vs clawdius

**opencode ahead:** 44 providers vs 11; provider OAuth (ChatGPT/Claude/
Copilot/GitLab/Vertex); web GUI + Electron desktop; Agent Client Protocol for
Zed + VS Code SDK; Code Mode interpreter; durable cron agent runs; crash-
recoverable sessions with steering; embedded CDP browser with Lighthouse;
session sharing/replay; voice (OpenAI Realtime), image-gen, transcription;
plugin SDK with typed hooks; cassette test infra.

**clawdius ahead:** chat gateways (opencode has ZERO platform adapters,
confirmed by exhaustive grep: no Telegram/Discord/Slack/Matrix/Signal/
WhatsApp/RocketChat anywhere); OS-level sandboxing (opencode's only true
sandbox is its in-process JS interpreter; the rest is permission asks +
shell scanning); MCP server + router (opencode is client-only); Lean4 proof
estate (0 formal methods there); LSP server; OIDC/SAML/RBAC vs password +
device pairing; embeddings/graph-RAG (they use ripgrep only); DAG parallel
agents (they have per-subagent child sessions, no dependency graphs).

---

## 2. OpenClaw

### Architecture
pnpm monorepo: `src/` (~1.39M LOC core), `extensions/` (~806k LOC, 117
bundled plugins: 23 channel adapters, ~60 LLM providers), `apps/` (macOS/iOS/
Android companions), `ui/` (web control UI). Core is extension-agnostic;
plugins touch core only through `openclaw/plugin-sdk/*` subpaths, enforced
by tsconfig boundaries + contract tests.

### The gateway layer (clawdius's most direct comparison)

The channel plugin contract (`src/channels/plugins/types.plugin.ts`, 96
lines, ~25 optional capability adapters: `meta`, `pairing`, `security`,
`outbound`, `streaming`, `threading`, `allowlist`, `doctor`, `setupWizard`,
`heartbeat`, ...) is the checklist of what a complete channel adapter must
offer. Every adapter follows one file liturgy: `channel.ts`, `monitor.ts`,
`send.ts`, `draft-stream.ts`, `accounts.ts`, `probe.ts`, `doctor.ts`,
`config-schema.ts`, `security-audit.ts`, contract tests.

**Telegram** (`extensions/telegram/src/`, 160 files, 29.1k LOC, grammY):
outer restart loop (2s-30s exp backoff, jitter) above grammY runner
(maxRetryTime 1h) above a 30s stall watchdog that force-recycles hung
pollers; 409 getUpdates-conflict marks the undici transport dirty so sockets
are rebuilt; update offsets persisted per account (versioned, atomic,
bot-id-bound) for at-least-once ingestion; `draft-stream.ts` uses Telegram's
native draft API with fallback to send+edit, 1s throttle, superseded-preview
snapshots, lost-send detection; exec approvals delivered as inline buttons.

**Discord** (`extensions/discord/src/`, 201 files, 39.8k LOC): gateway
supervisor classifies close reasons (fatal/disallowed-intents stop, others
retry), buffers late events during teardown so shutdown never crashes; 429
handling honoring `retryAfter` everywhere; 13 files of thread-bindings
machinery; PluralKit awareness; inbound dedupe + worker pool.

**WhatsApp** (`extensions/whatsapp/src/`, 126 files, 13.3k LOC, baileys):
creds persisted with atomic fsync'd 0600 writes, per-account save queues;
close-reason classifier (401 = wipe creds + re-QR; 515 = wait for save;
conflict = rebuild); inactivity watchdog per connection; QR login CLI.

**Shared machinery worth porting:** `src/infra/retry-policy.ts`
(`CHANNEL_API_RETRY_DEFAULTS`: 3 attempts, 400ms-30s, honors
`retry_after`); `src/channels/draft-stream-loop.ts` (104-line generic
throttle/edit loop); `src/channels/status-reactions.ts` (emoji-as-status:
queued/thinking/tool/done/stall/compacting with stall timers);
`src/auto-reply/` queue modes (steer/followup/collect/interrupt) letting a
chat message steer a running agent at tool boundaries; block-vs-preview
streaming with human-like pacing (`docs/concepts/streaming.md`).

### Parity vs clawdius

**OpenClaw ahead:** 23 channels vs 9; per-channel transport hardening (the
list above); media/voice depth (voice-note codec gating, image re-encoding
under caps, sticker vision, telephony via Twilio/Telnyx/Plivo); multi-account
everywhere; session persistence across gateway restarts (JSON+JSONL+disk
budgets); pairing/onboarding wizards; in-chat exec approvals; iOS/Android/
macOS companion nodes (camera/screen/location/voice); ~60 providers.

**clawdius ahead:** Rust single binary vs ~2.5M LOC of lazy-import TS;
formal verification (they have vitest only); enterprise auth (OIDC/SAML/RBAC
vs shared secret + device pairing); sandbox depth (8 backends vs
Docker-centric); coding-engine surface (LSP, agentic sprints, graph-RAG;
OpenClaw is an assistant, not an engine).

**Net:** for "works with Telegram and other platforms seamlessly",
OpenClaw's adapter layer is the reference implementation. Clawdius's 9
adapters cover the platforms but lack the robustness patterns: persisted
ingestion offsets, layered reconnect with stall watchdogs, transport-dirty
marking, close-reason classification, `retry_after`-honoring retry, and
at-least-once delivery.

---

## 3. ironclaw

### Architecture
Rust workspace: root crate (~299k) + `ironclaw_engine` (34.9k, engine-v2:
Thread/Step/Capability/MemoryDoc/Project primitives), `ironclaw_tui`
(11k), `ironclaw_skills` (6.5k), `ironclaw_safety` (5.5k). Channels run
through a WASM hop (`channels-src/{telegram,slack,discord,whatsapp,feishu}`
compiled to wasmtime components with signed bundles). 22 providers declared
in a declarative `providers.json`. ~7.5 weeks stale at analysis time.

### Files worth stealing

| File | Why |
|---|---|
| `ironclaw/src/llm/smart_routing.rs` | 13-dimension prompt-complexity scorer (0-100) cascading cheap-to-primary models. Direct feature-port candidate for clawdius's task-aware routing. |
| `ironclaw/src/llm/circuit_breaker.rs` (+`failover.rs`, `response_cache.rs`) | Provider resilience decorator stack clawdius's 11-provider layer lacks. |
| `ironclaw/providers.json` + `src/llm/registry.rs` | Declarative provider registry (env-var secrets, model filters, setup hints) vs clawdius's hard-wired providers. |
| `ironclaw/src/worker/claude_bridge.rs` + `acp_bridge.rs` | Drives Claude Code / any ACP agent as a worker with PTY handling. Composable with clawdius-code. |
| `ironclaw/src/gate/pending.rs` | Human-approval pause state; closest prior art to clawdius's planned gate dashboard. |
| `crates/ironclaw_safety/src/leak_detector.rs` | Secret-exfiltration detection to layer onto the sandbox firewall. |
| `ironclaw/src/agent/self_repair.rs` | Stuck-job detection and tool fixing for long runs. |
| `ironclaw/FEATURE_PARITY.md` | Parity-matrix discipline (status/priority/owner columns). Template for clawdius's own roadmap doc. |

### Parity vs clawdius
**ironclaw ahead:** WASM component-model extension ecosystem (tools,
channels, sandbox) with signed bundles; declarative 22-provider registry;
model cascading; provider resilience decorators; ACP coding-agent bridging;
estimation with a continuous-learning component; dual embedded DB (Postgres
+ libSQL zero-dep mode).

**clawdius ahead:** its own FEATURE_PARITY.md admits no streaming (all
three modes false), no multi-agent routing, Discord/WhatsApp/Matrix/Teams
missing or stubbed; 5 channels via WASM hop vs clawdius's 9 native with
streaming edits; no enterprise auth; no LSP; no graph-RAG; no MCP server;
zero formal verification. Stale since early September.

---

## 4. picoclaw, OpenManus, paperclip, claude-code-leak

**picoclaw** (Go, ~91k LOC): the only repo here competing head-on with
engine+gateway scope at 1/3 the surface, trading depth for embedded
deployment (<10MB RAM). Steal: `pkg/agent/steering.go` (mid-turn interrupt
queue with bounded dequeue modes), `pkg/providers/{cooldown,error_classifier,
fallback}.go` (cooldown windows + error classification + multi-key rotation),
and its 17+ channel breadth including the Chinese ecosystem (WeCom, DingTalk,
Feishu, QQ) clawdius lacks entirely.

**OpenManus** (Python, ~12k LOC): teaching-quality ReAct reference. Only
`app/flow/planning.py`'s plan-step status lifecycle (with `blocked` marks
and stale-step re-planning) and `app/tool/mcp.py`'s remote/local tool-name
sanitization merit a look.

**paperclip** (TS, ~236k LOC): not an engine competitor; the org/governance
layer above engines (companies, goals, issues, budgets, approvals). Steal:
`server/src/services/heartbeat.ts`'s synthetic task keys for timer-wake
session resume (the exact bug class clawdius's HOST-015 proof encodes),
`budgets.ts`/`costs.ts` per-agent cost ceilings, and approval-liveness
composition in `approvals.ts` + `issue-liveness.ts`.

**claude-code-leak** (TS, ~512k LOC): the category's reference
implementation; mine it, do not compete with it. Steal: unified Task model
with per-type runtimes (`src/Task.ts`, `src/tasks/`), subagent fork with
memory snapshots (`src/tools/AgentTool/forkSubagent.ts`,
`agentMemorySnapshot.ts`), relevance-ranked team memory
(`src/memdir/findRelevantMemories.ts`), and coordinator-mode worker tool
restriction (`src/coordinator/coordinatorMode.ts`) as a mirror for
clawdius's security_policy proofs.

---

## Gap matrix (where clawdius stands)

| Area | clawdius | opencode | OpenClaw | ironclaw | picoclaw |
|---|---|---|---|---|---|
| Chat platforms | 9 native, streaming edits | none | 23, hardened | 5 via WASM | 17+ |
| LLM providers | 11, hard-wired | 44 + OAuth | ~60 plugins | 22, declarative | multi-key rotation |
| Provider resilience | retry only | route executor | retry policies | breaker+cache+failover | cooldown+classifier |
| Session durability | sessions + auto-compact | event-sourced, crash-safe | disk-persisted | sessions | FTS5 memory |
| Sandboxing | 8 OS backends | permission+scan | Docker+fs-bridge | Docker+WASM | per-OS isolation |
| Formal verification | 318 Lean4 theorems | none | none | none | none |
| Enterprise auth | OIDC/SAML/RBAC | password+pairing | secret+pairing | single-owner | none |
| MCP | server+client+router | client+OAuth | server+bridge | client | client |
| LSP | yes | none | none | none | none |
| Retrieval | graph-RAG + embeddings | ripgrep | LanceDB memory | pgvector hybrid | FTS5 |
| Multi-agent | DAG parallel agents | background subagents | bindings+routing | none (admitted) | steering queue |
| Multi-repo orchestration | fleet (this repo, new) | worktrees+workspaces | none | none | none |

## Recommended priorities

1. **Gateway hardening (from OpenClaw)**: shared retry policy honoring
   `retry_after`; layered reconnect with stall watchdogs per adapter;
   persisted ingestion offsets (Telegram first); transport-dirty marking on
   409/conflict; close-reason classification. This is the difference
   between "works" and "works seamlessly".
2. **Provider resilience (from ironclaw/picoclaw)**: declarative provider
   registry, circuit breaker + response cache, cooldown-based fallback,
   complexity-scored model cascading.
3. **Session durability (from opencode)**: durable inbox admission,
   event-sourced session state, compaction precompute.
4. **Test infra (from opencode)**: HTTP cassette recording for provider
   tests; fake Telegram/Slack APIs (ironclaw's Python e2e harness is also a
   good pattern).
5. **Channel breadth (from picoclaw)**: WeCom/DingTalk/Feishu/QQ adapters if
   the Chinese ecosystem matters to the roadmap.
6. **Governance (from paperclip)**: per-agent cost ceilings composed with
   RBAC; approval workflows tied to run liveness.
