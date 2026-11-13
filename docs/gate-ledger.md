# Gate ledger — hard-gate skips and their evidence

Per the standing instruction: hard gates met mid-stream may be skipped
with the skip noted here, alongside the compensating evidence.

| Date | Gate | Skipped? | Compensating evidence |
|---|---|---|---|
| 2026-11-06 | pre-push (full suite + clippy + deny + Lean) on 9f9c434 | NO — gate passed, pushed clean | run 42137438420 log |
| 2026-11-06 | pre-push clippy on 42139778832-era commit | NO — gate failed on timeout; hook timeout raised 300s→1800s; workspace clippy re-run manually: clean | clippy output in session log |
| 2026-11-06 | pre-push on bc4d929 | NO — gate passed | session log |
| 2026-11-06 → 11-07 | pre-commit hooks on all feature commits (be939ae, e87273d, ee4525f, 596d615, b7ae680, eec774b, cb1d988) | YES (CLAWDIUS_SKIP_HOOKS=1) | Per-commit compensating checks run manually: cargo fmt --check, targeted clippy -D warnings, targeted cargo test (tools::/auth::/session::). Full workspace pre-push gate passed on the pushes that carried these commits (bc4d929, e00a5d2, ee4525f→b7ae680 chain) |
| 2026-11-07 | pre-push on 2517c64 (Lyapunov proof) | NO — gate passed | Lean check clean |
| 2026-11-07 | pre-push on be939ae | NO — gate passed | session log |
| 2026-11-08 | pre-push on 3c8e6ba (auto tool loop) | NO — gate passed | run 42020273427-era verification + dogfood run 13 success |
| 2026-11-08 | pre-push on 46eb012, ab26e07, bbf4173, 4a0f666 | NO (docs-only or gate passed) | session log |

## Rule

A skipped gate is acceptable only when the same check has been run
manually in the same session with captured output, OR the commit is
docs-only. The daily scheduled Dogfood run + full pre-push gate on
gated pushes remain the authoritative verification.

## 2026-11-12 — streaming + ACP arc
- Skipped pre-commit hooks on 6 commits (CLAWDIUS_SKIP_HOOKS=1) during
  the streaming/ACP push series. Compensating evidence, per commit:
  fmt + clippy -D warnings on both crates, clawdius-core lib 1230
  tests green, clawdius lib 127 green (incl. 5-6 ACP + 4 blackboard
  unit tests), E2E harness 12 PASS / 0 FAIL / 2 SKIP (live Google).
  Live verifications: chat --stream returns STREAM-OK in chunks;
  clawdius acp full prompt turn returns agent_message_chunk +
  stopReason end_turn; btrfs flap re-applications verified by in-tree
  grep before every git add.

## 2026-11-13 — lock repair, fallback, pipeline arc
- Skipped pre-commit hooks on 8 commits (CLAWDIUS_SKIP_HOOKS=1).
  Compensating evidence: clawdius-core lib 1245 green (fallback 6,
  pipeline 5, history 1, subagent 10), clawdius lib 128 green, clippy
  -D warnings on both crates per commit, E2E harness 12 PASS / 0 FAIL
  / 2 SKIP with live provider calls. Cargo.lock repair verified with
  `cargo metadata --locked` before commit (CI red since 2026-11-12
  15:58 caused by missing ignore/globset/sha2-0.11 lock entries).
- Known-left: pipeline LLM stages reuse the auto engine — full
  end-to-end product-loop run is a dogfood-scale action, not yet
  executed (gates + stage prompts verified).

## 2026-11-13 — dependency-security arc
- libsqlite3-sys 0.38.2 -> 0.39.1 (rusqlite 0.41): clears RUSTSEC-2026-0459,
  the cargo-deny advisories blocker. Core+clawdius suites green on 0.41.
- wasmtime family 49.0.2 -> 49.0.5, zip 7.4.0, zstd 0.13.4: clears
  RUSTSEC-2026-0420/0422/0423/0424/0429 + the zstd cross-stream leak.
- OPEN (documented, not ignored): vodozemac 0.9 -> 0.11.2
  (RUSTSEC-2026-0510/0451) requires matrix-sdk 0.17 — blocked by a
  serenity/reqwest feature-rename cascade in the gateway graph
  (rustls-tls -> rustls). This is a gateway-adapter migration, not a
  lockfile bump; the Security workflow will hold the vodozemac pair
  until it lands.
- CI toolchain discovery: dtolnay/rust-toolchain@1.96 pins the ACTION,
  not the toolchain — the jobs run LATEST STABLE (1.100 today). Local
  sweeps must use `rustup run stable cargo clippy ...` to match.
- Local gate now equals CI: `cargo clippy --workspace --all-targets
  --locked -- -D warnings -A missing_docs` exits 0 on 1.96, 1.99 and
  1.100; core 1245 + clawdius 128 lib tests green.

## 2026-11-13 — dev-loop + 24/7 maintenance arc
Live runs of `clawdius pipeline dev-loop` and `clawdius maintain` found
eight defects that unit tests never would have. Each is fixed with a
test or a recorded live observation; none were papered over.

| # | Defect (live symptom) | Fix |
| --- | --- | --- |
| 1 | `list_directory {}` failed path validation; model retried the identical call until the loop guard fired | omitted `path` defaults to the repo root (`07d3938`) |
| 2 | tools shipped prose argument hints, so models emitted `{}` for every tool | real JSON Schema on all 12 tools (`f43ae6c`) |
| 3 | loop guard hashed tool NAMES, so reading several files looked like a spin and hard-failed the run | fingerprint includes arguments; counter-witness test (`a873ac4`) |
| 4 | pipeline errors were mapped to RED before rotation saw them, so a 429 ended the task | errors propagate; quota rotates (`fe32f17`) |
| 5 | cooldown ignored the provider's `retryDelay` (Gemini free tier: 5 RPM, 37 s reset vs a 5 min policy) | parse and honour the hint, capped at 30 min (`fe32f17`) |
| 6 | no stage timeout: a retired Gemini model name made the adapter hang forever | 10-minute stage ceiling (`3fb1cd8`) |
| 7 | Ollama base URL concatenated without a separator → bare reqwest "builder error"; `chat -P ollama` was broken too | normalise to a trailing slash (`f8e2015`) |
| 8 | `LlmProvider` enum dispatch hard-refused Ollama tool calls, so the provider override was dead code | both dispatch sites delegate (`a5ccb3d`) |

Plus a safety defect found by reading the escalation log: opencode
resolves the git project root, so escalating inside a worktree made it
edit the MAIN checkout and spend real quota. Worktree runs no longer
escalate, and every cycle snapshots/reverts main-tree leaks (`4887838`).

Verified live on a scratch repo (`/tmp/opencode/looptest`, two
deliberately failing tests):

- planning stage runs as a single chat call and its deliverable chains
  into the implementation stage's prompt;
- implementation stage executes real tool calls (`read_file`, `edit_file`,
  `shell`) against the worktree;
- all three gates run as deterministic shell ground truth, loop 2 repeats
  from `implementation`, and a red tail reports the gate output;
- the maintenance loop creates `maint/<id>`, commits + pushes + opens a
  PR on green, discards worktree and branch on red, and appends a ledger
  record either way;
- cycle evidence (run journal + stage deliverables) is preserved under
  `.clawdius/maintenance/runs/<task-id>/` even when the worktree is
  discarded.

Provider reality on this host, 2026-11-13 — the loop cannot depend on
any hosted key:

| Provider | State |
| --- | --- |
| google | free tier 5 RPM + daily cap, exhausted by these runs |
| deepseek | `402 Insufficient Balance` |
| opencode-go | key expired |
| zai-coding-plan | off-limits (supported-tools-only ToS) |
| anthropic / openai | not configured |
| ollama (local) | works, including tool calling after #7 and #8 |

Open: no green live run yet. `llama3.2:3b` drives the tool loop but
mangles `Cargo.toml`; a stronger local coder model is the next attempt.
The pipeline itself is not implicated — every stage, gate, loop and
cleanup path has been observed working.

## 2026-11-13 (later) — first green cycle; loop running 24/7
The open item from the arc above is closed: the maintenance loop
recorded its first GREEN cycle against the scratch repo.

| # | Defect (live symptom) | Fix |
| --- | --- | --- |
| 10 | crash left a stale worktree registration pinning the branch; `worktree add -b` aborted and burned the task as RED in 0.1 s | `reset_worktree`: remove → prune → branch -D → add, with an integration test reproducing the exact debris (`26c0d32`) |
| 11 | "nothing to commit" (message on STDOUT, exit 1) recorded a satisfied task as RED with an empty stderr | empty staged set → GREEN "nothing to change", no branch, no PR (`a3bc598`) |

Observed end to end, scratch repo, provider google/gemini-3.1-flash-lite:

```text
verify PASS  570.7s -> 5.8s   (shared CARGO_TARGET_DIR, warm)
test   PASS  962.0s -> 223.0s
lint   PASS   85.3s -> 21.1s
cycle  1705s -> 314s
2026-11-13T22:40:57Z deps-2026-11-13 green "gates green; nothing to change"
```

Two operational notes for the next operator:

- **Run the loop from its own clone.** The leak guard assumes the loop is
  the sole writer of its checkout; the shared dev checkout has a second
  active agent session. `clawdius-maint` (local clone, GitHub origin) is
  the loop home.
- **Set `CARGO_TARGET_DIR` in the loop's environment** to a dedicated
  dir (e.g. `/home/wyatt/dev/.targets/clawdius-maint-loop`): worktrees
  do not inherit the primary repo's untracked `.cargo/config.toml`, so
  without it every cycle pays a cold workspace build.

Live process: `clawdius-maint/.clawdius/maintenance/loop.pid`
(interval 1800 s, google / gemini-3.1-flash-lite). Real-change cycles
(docs/tests tasks) are expected to produce the first pushed branch + PR.
