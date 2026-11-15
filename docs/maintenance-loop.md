# 24/7 maintenance loop

`clawdius maintain` keeps a repository improving unattended. It takes a
task from a durable backlog, runs the dev pipeline against it in an
isolated git worktree, and either opens a PR (gates green) or throws the
work away (gates red). The main working tree is never touched, so a bad
cycle cannot leave the checkout dirty or half-merged.

## State

| Path | Kind | Purpose |
| --- | --- | --- |
| `.clawdius/maintenance/backlog.json` | mutable | tasks, kinds, priorities, statuses |
| `.clawdius/maintenance/ledger.jsonl` | append-only | one line per cycle, never rewritten |
| `.clawdius/worktrees/<task-id>` | ephemeral | per-cycle git worktree, removed after the cycle |
| `maint/<task-id>` | git branch | survives only when the cycle went green |

`/.clawdius/maintenance/` is gitignored by default: the ledger is
runtime state, and the reviewable artifact is the pull request, not the
queue. A repository that wants the ledger in history can un-ignore it
and commit both files — they are plain JSON/JSONL.

Backlog writes are atomic (temp file + rename), so a crash mid-write
cannot truncate the queue. A corrupt backlog is reported as an error
rather than silently replaced — a lost queue is worse than a loud one.

## One cycle

1. Re-read the backlog (humans and other agents edit it).
2. Pick the highest-priority `pending` task; ties break by file order.
3. Mark it `running` and save.
4. `git worktree add -b maint/<id> .clawdius/worktrees/<id> HEAD`.
5. Run the dev pipeline there: architecture-planning, implementation
   (full tool loop), then the deterministic gates — `cargo check`,
   `cargo test`, and `cargo fmt --check && cargo clippy -D warnings`.
6. Green: commit on the branch, push, `gh pr create`. Red: remove the
   worktree and delete the branch.
7. Append a `CycleRecord` to the ledger and set the final status.

Nothing is merged automatically. The loop's product is reviewable pull
requests.

## Task kinds

Each kind carries a standing goal that is used when the task's own
`goal` field is empty.

| Kind | Standing goal |
| --- | --- |
| `lint` | Make the gates green; fix causes, never weaken lints or delete tests |
| `deps` | Upgrade patch/minor dependencies one at a time, gates green, no major jumps |
| `docs` | Fix stale commands, missing flags, broken cross-references |
| `tests` | Add focused tests for untested public behaviour |
| `issue` | Fix a reported issue at its root cause, with a regression test |
| `custom` | Whatever the task's goal says, ending green |

When the backlog runs dry and replenish is on (the default; disable with
`--no-replenish`), the loop
enqueues one standing task per kind per day, so a quiet repository still
gets lint/deps/docs/tests attention on a rotation.

## Provider rotation

A quota or rate-limit failure is not a task failure. The loop reuses
`FallbackCoordinator` and distinguishes three cases:

| Error carries a reset hint | Behaviour |
| --- | --- |
| yes, ≤ 10 min (`retryDelay: 37s`, `retry in 37`) | wait that long, then retry the same provider |
| yes, > 10 min (free-tier daily resets: `retryDelay: 57923s`) | fail the cycle fast (≈1 s), requeue the task as `pending`, park until the reset |
| no | mark it cooling down and try the next provider in the chain |

Waiting beats rotating when the provider says when it will be back: the
chain's other entries may be unconfigured or out of credit, so failing
over can turn a 40-second wait into a dead task. Sleeping beats
failing for short hints; for a multi-hour hint it wedged a task in
`running` for hours while the ledger said idle — hence the 10-minute
threshold (`MAX_HONORED_WAIT`). The parsed hint is capped at 30
minutes so a bogus value cannot park the loop for a day.

A quota-blocked cycle never marks the task `failed` and never lets the
rotation reach the rest of the chain (live: a quota-red followed by
"API key not set for Anthropic" recorded as the task's failure).
`status_for` decides: red/error with `quota_blocked` → `pending`;
otherwise red/error → `failed`.

After `MAX_PROVIDER_ROTATIONS` attempts the task is recorded as `error`.
When every provider in the chain is cooling down, the loop parks until
the soonest reset instead of burning the interval on guaranteed
failures.

Rate-limit shapes recognised: HTTP 429, `rate limit`, `quota`,
`insufficient balance`, `RESOURCE_EXHAUSTED`, `too many requests`,
`overloaded`, `try again later`.

The chain itself only contains providers the stages can authenticate
against: a fallback with no key is not a rescue, it is a dead end
(live: a hint-less Z.AI throttle rotated into a keyless Anthropic and
the cycle reported "API key not set for Anthropic" as the task's
failure). `provider_chain` resolves each candidate through the same
`ResolvedLlmConfig` the stages use, so "configured" means exactly what
the stage will find. The `--provider` you pass stays first regardless —
choosing an unkeyed provider is operator error and should fail loudly.

## Free-tier models (measured 2026-11-15)

Provider capacity is the binding constraint on how much the loop can do.
Probe a model before pinning it: a 429 with `limit: 0` means **no
allocation at all**, not "spent today", and Google attaches a bogus
~16 h `retryDelay` to those denials.

| Provider / model | Free-tier reality |
| --- | --- |
| Google `gemini-4-argon`, `gemini-4-carbon`, `gemini-3.1-pro-preview` | `limit: 0` on every metric — never usable on a free key |
| Google `gemini-2.5-pro`, `gemini-2.5-flash` | closed to new users |
| Google `gemini-3-flash-preview` | works, 20 requests/day — one cycle needs ~15 |
| Google `gemini-3.1-flash-lite` | real daily headroom; weakest output quality (writes empty artifacts) |
| Z.AI `glm-4.5-flash` (metered `api/paas/v4`) | **free, no request ceiling, 131k context, native tool calling** |
| Z.AI `glm-4.7-flash` (metered) | free too, but 429 `code 1305` overload more often |

Z.AI free Flash models are reached on the **metered** endpoint, not the
coding-plan one: leave the model unprefixed (`--model glm-4.5-flash`).
`zai-coding::` / `zai::` prefixes select `api/coding/paas/v4/`, which is
the Coding Plan entitlement and must not be used by CI or unsupported
tools. A `code 1305` "temporarily overloaded" is a transient throttle,
already covered by the rate-limit classifier; with no reset hint it
rotates, so keep the chain honest (above).

## Loop context

Every loop stage (implementation tool loop and single-call text stages)
gets the same context the TUI and opencode give their models:

- the repo's `AGENTS.md` (else `CLAUDE.md`), capped at 8k chars;
- up to 2 skills whose name/description tokens overlap the task text,
  bodies inlined (3k chars each) — inlined rather than referenced,
  because the loop's `read_file` rejects paths outside the workspace.

Skill roots: `<repo>/.opencode/skills`, `<repo>/.claude/skills`,
`~/.agents/skills`, `~/.config/opencode/skills`, `~/.clawdius/skills`
(repo roots first, first root wins on name collisions). Selection is
deterministic whole-word token matching — no LLM, no randomness; see
`crates/clawdius-core/src/context/loop_context.rs`.

### MCP tools

The loop's tool list also extends with opencode's MCP fleet (same
`servers` map in `~/.config/opencode/opencode.jsonc`), exposed as
`mcp__<server>__<tool>`. Local stdio servers only in v1 — remote
(`url`) entries need an HTTP transport. A server that fails to launch,
initialize, or answer within 8s is skipped with the reason in the
cycle's `MCP:` line; a hung *call* costs 30s, not the cycle.

| Environment variable | Effect |
| --- | --- |
| `CLAWDIUS_MCP=off` | disable the bridge |
| `CLAWDIUS_MCP_SERVERS=a,b` | restrict to those servers |
| `CLAWDIUS_MCP_CONFIG` | override the config path |

See `crates/clawdius-core/src/mcp/bridge.rs`.

## FreeLLMAPI router (free-tier pool)

A single free model is a hard ceiling: Google's free tier is 20
requests/day on the Flash models, and one cycle needs ~15. FreeLLMAPI
(`github.com/tashfeenahmed/freellmapi`) is a self-hosted router that puts
every free tier behind one OpenAI-compatible endpoint, tracks per-key
RPM/RPD/TPM/TPD caps, and fails over on 429/5xx. The loop talks to it as
an ordinary OpenAI-compatible provider — no code path is special-cased.

```bash
# install / update (Docker; ~/freellmapi, dashboard on :3001)
curl -fsSL https://freellmapi.co/install.sh | bash

# loop against the pool; the trailing slash is load-bearing
OPENAI_BASE_URL=http://localhost:3001/v1/ \
OPENAI_API_KEY=$(cat ~/.config/opencode/keys/freellmapi) \
  clawdius maintain --interval 1800 -P openai-compatible --model auto
```

`auto` lets the router pick a model that still has quota; pin a model id
(`glm-4.5-flash`, `gemma-4-26b-a4b-it`) when output quality matters more
than capacity. Two wiring facts cost a debugging session each:

- **The base URL needs its trailing slash.** genai appends
  `chat/completions` to whatever endpoint is configured, so
  `http://localhost:3001/v1` becomes `/v1chat/completions` (the SPA
  fallback answers with HTML).
- **Custom endpoints must pin `AdapterKind::OpenAI`.** genai infers the
  adapter from the model name, and an unknown name — a router alias like
  `auto`, or any local model — falls back to the Ollama adapter and posts
  to `/v1/api/chat`. `OpenAIProvider::with_base_url` pins it.

Credentials live outside the repo: `~/.config/opencode/keys/freellmapi`
(the router's unified key, mode 0600) and
`~/.config/opencode/keys/freellmapi-admin` (dashboard login). Provider
keys are entered on the dashboard's Keys page; the router stores them
AES-256-GCM encrypted in its own volume. Upstream terms still apply when
traffic is proxied, so the Z.AI entry is only reachable through the three
free Flash models the catalog lists for it (`glm-4.5-flash`,
`glm-4.7-flash`, `glm-4.6v-flash`) — no paid GLM model can be selected,
and no plan quota is consumed. Strict separation means removing that key
and leaving Z.AI as a direct fallback instead.

## Commands

```bash
# Inspect the queue without running anything
clawdius maintain --dry-run

# One cycle (useful in CI or for a smoke test)
clawdius maintain --once -P google --model gemini-3.5-flash

# Continuous, 15 minutes between cycles, no PRs (commit to branch only)
clawdius maintain --interval 900 --no-pr

# Bounded soak: 20 cycles then exit
clawdius maintain --max-cycles 20
```

Model names age: `gemini-2.5-flash` was retired (404) and the API's own
migration hint pointed at a model that does not exist yet, so verify the
name against the provider's model list before pinning it in a unit file.
`gemini-3.5-flash` is current at the time of writing.

Flags: `--backlog`, `--worktree-root`, `--provider`, `--model`,
`--interval`, `--once`, `--max-cycles`, `--dry-run`, `--no-pr`,
`--no-replenish` (replenish is on by default).

## Running it detached

`setsid nohup` with a PID file is enough for a single host:

```bash
cd /path/to/repo
setsid nohup clawdius maintain --interval 900 \
  > .clawdius/maintenance/loop.log 2>&1 &
echo $! > .clawdius/maintenance/loop.pid
```

A systemd user unit is the durable option — it survives logout and
restarts on failure:

```ini
# ~/.config/systemd/user/clawdius-maint.service
[Unit]
Description=clawdius 24/7 maintenance loop
After=network-online.target

[Service]
Type=simple
WorkingDirectory=%h/dev/src/github.com/WyattAu/clawdius
EnvironmentFile=%h/.config/clawdius/env
ExecStart=%h/.cargo/bin/clawdius maintain --interval 900 -P google --model gemini-3.5-flash
Restart=on-failure
RestartSec=60

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now clawdius-maint.service
journalctl --user -u clawdius-maint -f
```

Keep provider keys in `~/.config/clawdius/env` (mode 0600), not in the
unit file.

## Choosing a local model

Ollama support is per model template, not per server. A model whose
template does not parse tool calls returns them as **text**:

```text
content: '{"name": "read_file", "arguments": {"path": "src/lib.rs"}}'
tool_calls: null
```

clawdius sees no tool call, the stage "completes" without acting, and the
gates catch it — correct, but a wasted cycle. Probe before trusting a
model:

```bash
curl -s http://localhost:11434/api/chat -d '{
  "model": "llama3.2:3b", "stream": false,
  "messages": [{"role":"user","content":"Read src/lib.rs with read_file."}],
  "tools": [{"type":"function","function":{"name":"read_file",
    "parameters":{"type":"object","properties":{"path":{"type":"string"}},
    "required":["path"]}}}]
}' | jq -c '.message.tool_calls'
```

`null` means the model cannot drive the loop. Observed 2026-11-13:
`llama3.2:3b` returns a structured call; `qwen2.5-coder:7b` returns text.

## Reading the ledger

```bash
# Last 10 cycles, one line each
tail -10 .clawdius/maintenance/ledger.jsonl | jq -c \
  '{at, task_id, outcome, provider, pr_url, note}'

# Success rate by kind
jq -r '[.kind, .outcome] | @tsv' .clawdius/maintenance/ledger.jsonl \
  | sort | uniq -c | sort -rn
```

## Invariants

- The loop never edits the primary working tree; all work happens in a
  worktree that is removed at the end of the cycle.
- A red cycle leaves no branch behind.
- Gates are deterministic shell ground truth, not model judgement.
- The ledger is append-only; history is never rewritten.
- Provider quota errors rotate; they do not fail the task.
- No stage may write to the primary checkout. The loop snapshots
  `git status --porcelain` before each cycle and reverts any tracked file
  that becomes dirty during it, recording the paths in the ledger. A
  green cycle downgrades to `error` if a leak was detected.

The leak guard assumes the loop is the only writer to the checkout while
a cycle runs. If another agent or a human edits the same working tree
concurrently, their tracked-file edits can be reverted as leaks — run
the loop from its own clone, or do not edit the checkout during a cycle.
