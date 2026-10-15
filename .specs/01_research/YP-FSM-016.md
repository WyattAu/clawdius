---
document_id: YP-FSM-016
version: 1.0.0
status: APPROVED
domain: Reliability / Workflow Engine
subdomains: [Failure Recovery, Deadlock Freedom, Retry Budgets]
created: 2026-10-15
author: Loop
proof_files: [fsm.lean]
confidence_level: 0.92
tqa_level: 4
research_iterations: 2
---

# Yellow Paper: FSM — Recovery Is Deadlock-Free but Requires a Retry Budget

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Harel, *Statecharts: A Visual Formalism for Complex Systems* (IEEE CSE 1987) | Failure transitions are first-class statechart citizens; depth/clustering handle recovery without state explosion. |
| Drusinsky, *Run-time Monitoring and Recovery of Harel Statecharts* (Deep Impact flight software) | Temporal-logic run-time verification + recovery transitions validated on flight software. |
| Kestra / industry retry patterns | Retry budgets and exponential backoff: recovery WITHOUT a bound becomes the livelock. |
| Loop 2 it. 7 (YP-HOST-KERNEL-015) | The crash-only discipline: restart as a first-class transition, with self-stabilization proven. |

## YP-2: The gap found in the spec

The 24-phase Nexus FSM is a strictly linear pipeline with **zero failure
semantics**: `next` never offers a recovery path, so a failed phase
deadlocks the entire run. The legacy proofs (termination, deadlock
freedom, monotonic progress, no cycles) hold only for the
failure-free path — the moment any phase fails, every guarantee is
void.

## YP-3: Analysis — the field's answer

Statechart practice makes failure transitions first-class; production
workflow engines pair recovery with **retry budgets** because recovery
without a bound becomes the livelock. The formally interesting split:

- **Deadlock-freedom under failure** is achievable unconditionally —
  recovery is total (every phase has one).
- **Termination under failure** requires a retry budget — with
  unbounded failures, the recovery pair
  (knowledgeTransfer ↔ continuousMonitoring) cycles forever, and that
  two-phase livelock is *machine-checked* (T3b).

**Are we solving a problem the field could not?** No — retry budgets
are standard practice. The machine-checked pairing (deadlock-freedom
unconditional; termination conditional on the budget; the livelock
cycle constructed) is the estate's formal version of the field's
informal rule.

## YP-4: The corrected model

- `recover p`: a failed phase drops to `continuousMonitoring` (the
  observer), which re-runs knowledge transfer + archive. Recovery is
  total and never yields `archive` directly (`recover_ne_archive`).
- `advance p := (next p).getD (recover p)` — the effective transition:
  normal `next`, falling back to recovery.
- Asserted (T-pair): `recover_in_pair` — recovery always lands in the
  monitoring pair; `recover_ne_archive` — recovery never jumps to the
  terminal.
- Asserted (T-progress): `always_progresses` — deadlock-freedom under
  failure: normal transition exists or recovery moves the FSM.
- Asserted (T-recovery): `recovery_reaches_archive` — from every phase,
  ONE recovery step + at most 2 normal transitions reach `archive`.
- Asserted (T3b): `unbounded_retry_livelock` — the knowledgeTransfer ↔
  continuousMonitoring cycle exists; the retry budget is therefore
  mandatory.

## YP-5: Falsifiable predictions

- P1 (proved): `advance` is total — no phase is stuck under failure.
- P2 (proved): recovery never yields `archive` directly — the failed
  run is observed before it is archived.
- P3 (proved): recovery + ≤ 2 normal transitions reach `archive` from
  every phase.
- P4 (proved): the livelock cycle exists without a retry budget — the
  budget is mandatory, recorded as the spec's justification.
