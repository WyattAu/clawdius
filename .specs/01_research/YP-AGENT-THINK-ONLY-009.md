---
document_id: YP-AGENT-THINK-ONLY-009
version: 1.0.0
status: DRAFT
domain: Agent Systems / Liveness
subdomains: [Agent Loop, Termination, ReAct]
created: 2026-10-11
author: Loop
proof_files: [agent_loop.lean]
confidence_level: 0.95
tqa_level: 4
research_iterations: 9
---

# Yellow Paper: Thought-Only Runs — Non-Termination by Design

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Yao et al., *ReAct: Synergizing Reasoning and Acting in Language Models* (ICLR 2023) | The agent loop interleaves a **reasoning** step (which never ends the loop) with an **acting** step (which can end it) — reasoning produces the next thought, never a terminal. |
| The loop's own Rust invariant (api/agent_loop.rs) | Terminal exits are produced only by completion, budget exhaustion, or the iteration cap — never by a reasoning step. |

## YP-2: The gap found in the spec

`thinking_is_not_terminal` proved that *one* thought is non-terminal. The property the
loop relies on is stronger and was unproven: **any number of consecutive thoughts** is
non-terminal. An agent that only reasons loops forever — by design, since thoughts carry
no side effects and cannot produce a completion.

## YP-3: Analysis — the field's answer

ReAct's design has exactly this shape: the reason step is a pure transition, and
liveness (the loop *eventually* terminates productively) comes from the acting step plus
the iteration/budget caps — the same caps this spec proves enforce termination
(`agent_loop_terminates`, `loop_terminates`). The thought-only chain theorem is the
safety side of that pairing: thoughts alone neither help nor terminate.

**Are we solving a problem the field could not?** No — the field builds it this way.
The estate's addition is that the "thoughts alone never terminate" posture is now a
theorem for every chain length, rather than a reading of the state machine.

## YP-4: The model

`thoughts k s` applies `k` consecutive reason transitions to `s`; the theorem
`thoughts_not_terminal` proves non-termination by induction on `k`, with the base case
being the existing per-step theorem. Note the induction base is `k = 0` — which lands on
`thinking_is_not_terminal s` because the match folds `thoughts 0 s = s` through a
`thinking` wrapper only for `k ≥ 1`; the statement holds for all `k` as compiled.

## YP-5: Falsifiable predictions

- P1: for every `k` and every `s`, `isTerminal (thoughts k s) = false` — checked by
  induction, compiled.
- P2 (the design reading): a thought-only run ends only via the iteration cap — which is
  `maxIterationsReached_is_terminal`, the *intended* exit for a reasoning-only agent.
