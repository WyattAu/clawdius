---
document_id: YP-HOST-KERNEL-015
version: 1.0.0
status: APPROVED
domain: Reliability / Lifecycle Management
subdomains: [Crash-Only Design, Self-Stabilization, Restart Safety]
created: 2026-10-15
author: Loop
proof_files: [host.lean]
confidence_level: 0.94
tqa_level: 4
research_iterations: 2
---

# Yellow Paper: Host — Crash-Only Redesign of the Lifecycle Model

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Candea & Fox, *Crash-Only Software* (HotOS IX 2003; USENIX 2004 dissertation) | Failure handling should be restart; recovery must be a first-class operation, not an afterthought — "the only fast recovery path is the crash path". Components that cannot be restarted are unrecoverable by design. |
| Dijkstra, *Self-Stabilizing Systems in Spite of Distributed Control* (CACM 1974) | The deeper anchor: a system is self-stabilizing if, from ANY state, it converges to a legitimate state — regardless of how it got there. |
| Legacy model (loop 2 audit) | The lifecycle was proven safe (shutdown reachable, no double-init) but `stopped` was terminal — a liveness hole the legacy model asserted as a theorem (`stopped_is_terminal`). |

## YP-2: The gap found in the spec

`proof_host.lean` (legacy tree) proved the safety side thoroughly:
shutdown reachability from every state, no double-initialization, no
start-without-init, initialization sequences. But it **asserted
`stopped_is_terminal`** — no transition out of `stopped` — which makes
any host failure (a crash into `stopped`, or an error-driven shutdown)
*permanent*. Under Dijkstra's definition the host was provably NOT
self-stabilizing: `stopped` is an illegitimate state from which no
legitimate state is reachable.

## YP-3: Analysis — the field's answer

Crash-only software (Candea & Fox) mandates exactly the transition the
legacy model forbade: `stopped → initializing`, with recovery passing
through the *full* initialization phase — no shortcuts (a direct
`stopped → running` would skip re-validation). Dijkstra's
self-stabilization generalizes the target property: from every state,
a legitimate state (here: `running`) is reachable.

**Are we solving a problem the field could not?** No — crash-only
design is standard practice (systemd, Erlang supervisors, microservice
restart policies). The estate's contribution is the machine-checked
lifecycle: T1 proves self-stabilization for the kernel AND component
state machines — including the `error` state — while T2 proves the
shortcut (restart skipping initialization) remains impossible.

## YP-4: The corrected model

- `validKernelTransition` gains `stopped → initializing` (the restart).
  `validComponentTransition` gains `stopped → initialized`.
- The legacy `stopped_is_terminal` theorem is REPLACED by the
  crash-only pair: `stopped_is_restartable` (the transition exists) +
  `no_direct_restart_shortcut` (no `stopped → running` /
  `stopped → shuttingDown` / self-loop).
- New reachability structure `Reach` (kernel) and `ReachC` (component)
  over the transition relations.
- Asserted (T1): `running_reachable` — from EVERY kernel state,
  `running` is reachable (self-stabilization). Includes the `stopped`
  path: stopped → initializing → running, and the
  shuttingDown → stopped → initializing → running chain.
- Asserted (T1c): `component_running_reachable` — every component
  state, INCLUDING `error`, can reach `running`.
- Asserted (T2): `restart_requires_initialization` — no shortcut
  exists; a restart must re-run initialization.

## YP-5: Falsifiable predictions

- P1 (proved, T1): no kernel state is a trap — every state reaches
  `running`.
- P2 (proved, T1c): no component state is a trap — `error` included.
- P3 (proved, T2): the restart path re-validates —
  `stopped → running` directly is impossible.
- P4 (proved, T-P4a–e, loop 3 it. 5): a host restart re-initializes
  ALL registered components. Modelled over arbitrary registry contents
  as an association list: a restart **loses no component** (T-P4a,
  length preserved), **resets every entry** to `uninitialized` (T-P4b),
  and **invents no component** (T-P4c, every post-restart id was
  registered before). Combined with T2: from the post-restart state the
  only forward transition is the full initialization phase (T-P4d), and
  that sequence is valid for every registered component (T-P4e).

## YP-6: The registry-level restart, modelled (loop 3, iteration 5)

P4 was recorded-but-unasserted: "a host restart re-initializes ALL
registered components", blocked on the `ComponentRegistry` being an opaque
map. Iteration 5 discharged it by modelling the registry as an association
list — the functional spec of what the HashMap-backed reset computes — so
"ALL components" became a theorem about **arbitrary registry contents**
(proved by induction), not a claim checked against a snapshot of whatever
happens to be registered.

### The four properties, and why each one is separate

| Theorem | Claim | Failure it rules out |
|---------|-------|----------------------|
| T-P4a | `length` preserved | a restart *losing* components (partial cleanup) |
| T-P4b | every entry's state is `uninitialized` | a restart *skipping* some component (stale state surviving) |
| T-P4c | every post-restart id was registered before | a restart *inventing* entries (phantom components) |
| T-P4d | from `uninitialized`, direct `running` is invalid | a restart *shortcutting* initialization |

T-P4e closes the loop with T2: the full sequence
`uninitialized → initialized → running` is valid for every registered
component (enumerated over the finite `ComponentId`: host, fsm, sentinel,
brain, graph, broker).

### Findings

1. **The reset is only half the guarantee.** T-P4a–c say what the restart
   *does*; T-P4d–e say what it *obligates*. A registry reset without the
   no-shortcut property would satisfy "re-initializes everything" while
   still allowing `stopped → running` shortcuts — the crash-only discipline
   lives in the transition relation, not in the reset.
2. **`initialized → running` is valid** — so "cannot reach `running`
   directly" is a property of the *post-restart state* (`uninitialized`),
   not of the transition table in general. The first draft of T-P4d
   asserted the table-level claim and was rejected by the proof assistant
   (decide found `initialized → running` true). Recorded as a draft-defect.
3. **Association list over HashMap**: modelling the registry as a list
   makes "for all registry contents" an induction instead of a set of
   `HashMap` lemma dependencies. The HashMap-backed implementation
   (`ComponentRegistry`) refines this spec by construction — `map` over the
   hash map computes exactly `restartRegistry` on its entry list.
