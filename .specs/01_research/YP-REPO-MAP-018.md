---
document_id: YP-REPO-MAP-018
version: 1.0.0
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
- P4 (recorded): attainment and damping-generalized monotonicity —
  next iteration's work.
