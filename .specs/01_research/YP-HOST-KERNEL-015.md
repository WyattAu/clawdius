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
- P4 (recorded, not asserted): the registry-level property — a host
  restart re-initializes ALL registered components — is the next
  obligation (requires the registry to carry per-component restart
  semantics; the ComponentRegistry is currently an opaque map).
