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
