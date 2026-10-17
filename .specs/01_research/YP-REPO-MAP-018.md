---
document_id: YP-REPO-MAP-018
version: 1.1.0
status: APPROVED
domain: Retrieval / Repo Mapping
subdomains: [Graph-Ranking Retrieval, Accumulation Monotonicity, Selection Soundness]
created: 2026-10-15
author: Loop
proof_files: [repo_map_scoring.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 2
---

# Yellow Paper: Repo Map — Accumulation Is Monotone and Selection Is Sound

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Aider repo map (Gauthier) — tree-sitter tags + graph ranking | The production pattern: symbols as graph nodes, references as edges, retrieval by accumulated importance (PageRank-style in networkx). |
| Brin & Page, *The Anatomy of a Large-Scale Hypertextual Web Search Engine* (1998) | PageRank convergence: the damping-factor surfer converges to a stationary rank vector — the theoretical basis for graph ranking. |
| Page & Brin, P3: selection soundness | Retrieval correctness = every selected element dominates every unselected one under the score order. |
| The estate's loop-1 framing | Float semantics are not decidable in Lean — scoring is modeled in Nat scaled by 100. |

## YP-2: The gap found in the spec

`repo_map_scoring.lean` proved score normalization bounds ([0, 100])
— solid — but nothing about the *retrieval semantics*: the estate had
no machine-checked statement that (a) accumulating reference weights is
order-independent and monotone, or (b) top-k selection returns scores
that dominate the unselected ones. A ranking layer without those two
properties can silently rank lower scores above higher ones.

## YP-3: Analysis — the field's answer

Additive accumulation (a sum) is trivially order-independent and
monotone — the properties are free. PageRank-style damping introduces
NON-monotone interactions (a new reference can redistribute rank away
from neighbors), which is why the estate's Nat-scaled additive model is
the right proof target: the monotonicity theorems hold by
arithmetic, and the damping generalization is recorded as future work
(Float semantics again).

**Are we solving a problem the field could not?** No — additive
accumulation is elementary. The machine-checked selection-soundness
witness (`best_scores_dominate`) and the append-monotonicity are the
formal floor for retrieval, matching Aider's practice.

## YP-4: The corrected model

- `accumulate weights = weights.sum` — accumulated importance per
  symbol; `refWeight = 5` per in-reference.
- Asserted (T1): `accumulate_mono_append` — appending a reference
  never lowers the accumulated importance.
- Asserted (T2): `accumulate_append` — the total after appending is
  the sum of the parts (discovery order irrelevant).
- `bestScore scores = scores.foldr max 0` — the top-1 selection.
- Asserted (T3): `best_scores_dominate` — selection soundness: every
  score is dominated by the selection.
- Recorded (P4, not asserted): attainment — the maximum is attained by
  a real symbol — and rank-propagation damping — are future work.

## YP-5: Falsifiable predictions

- P1 (proved, T1): appending references is monotone — no discovery
  lowers a symbol's accumulated importance.
- P2 (proved, T2): the total after appending any partition equals the
  sum of the parts — discovery order is irrelevant.
- P3 (proved, T3): the top-1 selection dominates every score.
- P4a (proved, `best_attained`, loop 3 it. 1): the maximum is attained
  by a real symbol — selection never returns a phantom score.
- P4b (recorded with rationale, loop 3 it. 9): damping-generalized
  monotonicity is deliberately NOT attempted — see YP-7. The honest
  prediction is that it FAILS: damped rank propagation is not monotone
  in new references, by construction.

## YP-7: The damping decision, recorded (loop 3, iteration 9)

P4 bundled two obligations: attainment (discharged, it. 1) and
rank-propagation damping. Iteration 9 closes the bundle by recording the
damping decision with its rationale instead of pretending it is pending
work.

### Why damping stays out of scope

1. **Float semantics.** Damping is a multiplication by 0 < d < 1 per
   hop. The estate's ranking layer is `Nat`-scaled by policy (loop 2's
   Float-vs-Nat decision); a faithful damping model needs rationals, and
   floating ranking is how ranks silently drift between runs.
2. **Damping is non-monotone BY DESIGN.** A damped update redistributes
   rank: node j gains an out-edge to a new node k, j's out-degree grows,
   and every OLD neighbor of j receives a strictly smaller share
   (PageRank's own literature notes rank is not monotone in edges).
   Concretely with weights and split-even propagation: j references
   {a} with weight 5 — a gets 5. Add k: j references {a, k} — a gets
   2.5, k gets 2.5. Adding a reference LOWERED a's rank from 5 to 2.5.
   The estate's additive model makes `accumulate_mono_append` true by
   arithmetic; a damped model makes its analogue FALSE by arithmetic.
   Monotonicity is not an implementation gap — it is the property the
   design rejects.
3. **The consumer only needs order-stability, not propagation.** The
   repo map's contract (selection soundness + append monotonicity) is
   what the agent relies on: new discoveries never demote old symbols.
   Damping would break that contract in exchange for graph-global
   ranking the repo map does not claim.

### What a future damping model must carry

If damping is ever wanted, the YP to write first is the NON-monotonicity
theorem (a witness like the one above, machine-checked) plus an explicit
statement of which monotonicity theorems it sacrifices. Any model that
keeps `accumulate_mono_append` true is not damping — it is the additive
model with extra steps.
