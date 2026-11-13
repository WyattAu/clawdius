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
