---
document_id: YP-CONCURRENT-PLACEMENT-007
version: 1.0.0
status: DRAFT
domain: Concurrency
subdomains: [Linearizability, JoinSet Collectors, Positional Agreement]
created: 2026-10-11
author: Loop
proof_files: [concurrent_execution.lean]
confidence_level: 0.95
tqa_level: 4
research_iterations: 7
---

# Yellow Paper: Concurrent Tool Execution — Placement and No-Cross-Talk

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Herlihy & Wing, *Linearizability: A Correctness Condition for Concurrent Abstract Data Types* (ACM TOPLAS 12(3), 1990) | The correctness condition for concurrent objects: each operation appears to take effect **atomically at some point** between its invocation and its response. The guarantee is per-operation *identity*, not just per-batch counting. |
| Herlihy & Shavit, *The Art of Multiprocessor Programming* (2008/2020) | Bounded work-free queues and index-based slot publishing: a writer publishes at its own index; readers trust slot identity. |
| tokio `JoinSet` semantics (docs) | Tasks are spawned independently and *joined in completion order* — completion order is not submission order, which is why the collector must re-index by original position. |

## YP-2: The gap found in the spec

`concurrent_execution.lean` proved list-length properties (zip lengths, monotone
counters, termination) but never modelled the *placement* of a completion into its
original slot — the exact operation whose misuse would cross results between tool calls.
A wrong-index placement would give the agent tool A's answer for tool B's call, and every
theorem in the file would still hold: length invariants cannot see cross-talk.

## YP-3: Analysis — the field's answer

Linearizability is precisely the property "the response you got is the outcome of *your*
invocation, ordered at some point in real time". For the JoinSet collector the
linearization point of each completion is its placement into the slot indexed by the
call's original position. The research-grade claim decomposes into:

1. *Self*: the slot you placed at reads back your result.
2. *No-cross-talk*: every other slot reads back whatever it held — a completion never
   lands in a neighbour's slot.

Both are per-placement properties; composition over all placements follows by induction.
This is the same decomposition the linearizability proofs use (sequential specification +
ordering argument).

**Are we solving a problem the field could not?** No — 1990 solved it. The estate's value
is applying it to the one operation the agent loop actually performs concurrently, and
recording that *length theorems alone cannot see cross-talk* — which is why the file's
25 counting theorems were insufficient.

## YP-4: The corrected model

- `place slots i r` writes `some r` at index `i` of a slot list, decreasing the index
  through the tail — the read-copy-update shape.
- `place_length`: placement preserves length (no growth, no truncation).
- `place_self`: the placed slot reads back `some r`.
- `place_other`: every other index reads back what it held. Composed, these are
  positional agreement — result i is the outcome of call i.

## YP-5: Falsifiable predictions

- P1: placing at any index leaves the slot list's length unchanged.
- P2: `(place l i r).getElem? i = some r` whenever `i < l.length`.
- P3: for `i ≠ j`, `(place l i r).getElem? j = l.getElem? j` (within bounds) — no cross-talk.
