import Init

-- Repo-Map Scoring Normalization Proofs for Clawdius
-- Reference implementation: crates/clawdius-core/src/graph_rag/repo_map.rs
-- Lean 4.28.0 core library only (no Mathlib).
--
-- The repo-map scoring function computes an importance score for each code
-- symbol. This file formally verifies that the scoring normalization always
-- produces values in [0, 1], regardless of which scoring components are active.
--
-- Since Float comparison is not decidable in Lean 4 (IEEE 754 semantics),
-- we prove the theorem using Nat (natural number) arithmetic scaled by 100
-- to represent the centesimal values used in the implementation.
--
-- Scoring components (scaled by 100):
--   file_usage_score: 0 or 40
--   level_score:      0 or 20
--   kind_weight:      10, 15, 20, 25, 30, or 35
--   length_score:     0 or 10
--   Normalization divisor: 110 (represents 1.1)
--   Maximum raw sum:  40 + 20 + 35 + 10 = 105
--   Max normalized:   min(105 * 100 / 110, 100) = 95 (represents 0.95)

namespace RepoMapScoring

-- File usage score is 0 or 40
def fileUsageVal (active : Bool) : Nat :=
  if active then 40 else 0

-- Level score is 0 or 20
def levelVal (isTopLevel : Bool) : Nat :=
  if isTopLevel then 20 else 0

-- Kind weight for each symbol kind (scaled by 100)
-- Defined as a simple function with explicit branches
def kindWeightVal (kind : Nat) : Nat :=
  if kind = 0 then 30
  else if kind = 1 then 25
  else if kind = 2 then 35
  else if kind = 3 then 15
  else if kind = 4 then 20
  else 10

-- Length score is 0 or 10
def lengthVal (isLong : Bool) : Nat :=
  if isLong then 10 else 0

-- Raw sum of all scoring components
def rawScore (fu lv kw ln : Nat) : Nat := fu + lv + kw + ln

-- Normalization: divides by 110 then caps at 100 (representing 1.0)
def normalize (raw : Nat) : Nat :=
  min (raw * 100 / 110) 100

-- Lemma 1: Each component is bounded above
theorem fileUsage_bounded (active : Bool) :
    fileUsageVal active ≤ 40 := by
  simp [fileUsageVal]; split <;> omega

theorem level_bounded (isTopLevel : Bool) :
    levelVal isTopLevel ≤ 20 := by
  simp [levelVal]; split <;> omega

theorem kindWeight_bounded (kind : Nat) :
    kindWeightVal kind ≤ 35 := by
  simp only [kindWeightVal]
  by_cases h : kind = 0 <;> by_cases h2 : kind = 1 <;>
  by_cases h3 : kind = 2 <;> by_cases h4 : kind = 3 <;>
  by_cases h5 : kind = 4 <;> simp_all

theorem length_bounded (isLong : Bool) :
    lengthVal isLong ≤ 10 := by
  simp [lengthVal]; split <;> omega

-- Lemma 2: Raw sum is bounded by 105
theorem raw_bounded (fu lv kw ln : Nat) :
    fu ≤ 40 → lv ≤ 20 → kw ≤ 35 → ln ≤ 10 →
    rawScore fu lv kw ln ≤ 105 := by
  simp [rawScore]; omega

-- Lemma 3: Normalization never exceeds 100 (i.e., 1.0)
theorem normalize_bounded (raw : Nat) :
    normalize raw ≤ 100 := by
  simp [normalize]; apply Nat.min_le_right

-- Lemma 4: Normalization is always non-negative
theorem normalize_nonneg (raw : Nat) :
    0 ≤ normalize raw := by
  exact Nat.zero_le (normalize raw)

-- Main Theorem: Score is always in [0, 100] (representing [0.0, 1.0])
-- For any valid combination of scoring components, the normalized
-- repo-map score lies in the unit interval.
theorem repo_map_score_bounded (fuActive lvTop kindParam lnLong : Bool) :
    let fu := fileUsageVal fuActive
    let lv := levelVal lvTop
    let kw := kindWeightVal (if kindParam then 1 else 0)
    let ln := lengthVal lnLong
    0 ≤ normalize (rawScore fu lv kw ln) ∧
    normalize (rawScore fu lv kw ln) ≤ 100 := by
  simp only
  constructor
  · exact normalize_nonneg (rawScore (fileUsageVal fuActive) (levelVal lvTop)
        (kindWeightVal (if kindParam then 1 else 0)) (lengthVal lnLong))
  · exact normalize_bounded (rawScore (fileUsageVal fuActive) (levelVal lvTop)
        (kindWeightVal (if kindParam then 1 else 0)) (lengthVal lnLong))

-- Corollary: Maximum possible normalized score is 95 (0.9545...)
-- This means the clamp to 1.0 is technically unreachable but provides
-- a safety margin against future changes to component bounds.
theorem max_normalized_score :
    normalize 105 = 95 := by
  decide

-- Corollary: All scoring components are non-negative
theorem all_components_nonneg (fuActive lvTop lnLong : Bool) (kindParam : Nat) :
    0 ≤ fileUsageVal fuActive ∧
    0 ≤ levelVal lvTop ∧
    0 ≤ kindWeightVal kindParam ∧
    0 ≤ lengthVal lnLong := by
  simp only [fileUsageVal, levelVal, lengthVal, kindWeightVal]
  by_cases h : kindParam = 0 <;> by_cases h2 : kindParam = 1 <;>
  by_cases h3 : kindParam = 2 <;> by_cases h4 : kindParam = 3 <;>
  by_cases h5 : kindParam = 4 <;>
  split <;> split <;> simp_all


-- ---------------------------------------------------------------------------
-- Graph-ranking retrieval (YP-REPO-MAP-018, loop 2 iteration 10)
-- ---------------------------------------------------------------------------

/-! The repo map retrieves symbols by accumulated reference
importance: each in-reference contributes a weight to the target
symbol, and top-k selection returns the highest accumulated scores.
The formal properties: accumulation is order-independent and monotone
under new references, and the maximum-selection dominates every score.
(Rank-propagation interactions — PageRank damping — are Float
semantics and recorded, not modeled.) -/

/-- Accumulated importance: the sum of reference weights. Order-
independent by construction (a sum). -/
def accumulate (weights : List Nat) : Nat := weights.sum

/-- Reference weight per in-reference. -/
def refWeight : Nat := 5

theorem refWeight_pos : 0 < refWeight := by decide

/-- Sum splits over append (local helper; the core name differs). -/
theorem sum_append (l1 l2 : List Nat) : (l1 ++ l2).sum = l1.sum + l2.sum := by
  induction l1 with
  | nil => simp
  | cons a t ih => simp only [List.cons_append, List.sum_cons, ih, Nat.add_assoc]

/-- T2 (append closure): the accumulated total after appending equals
the sum of the parts — discovery order is irrelevant to the total. -/
theorem accumulate_append (l1 l2 : List Nat) :
    accumulate (l1 ++ l2) = accumulate l1 + accumulate l2 := by
  unfold accumulate
  exact sum_append l1 l2

/-- T1 (append monotonicity): a new reference never lowers the
accumulated importance — the ranking is monotone under discovery. -/
theorem accumulate_mono_append (weights : List Nat) (w : Nat) :
    accumulate weights ≤ accumulate (weights ++ [w]) := by
  rw [accumulate_append]
  omega

/-- The best score in a list: the maximum (0 for the empty map). -/
def bestScore (scores : List Nat) : Nat := scores.foldr max 0

/-- T3 (SELECTION SOUNDNESS): the maximum-selection dominates every
score — top-k retrieval built on `bestScore` can never rank a lower
score above a higher one. -/
theorem best_scores_dominate (scores : List Nat) :
    ∀ s ∈ scores, s ≤ bestScore scores := by
  intro s hs
  unfold bestScore
  induction scores with
  | nil => exact absurd hs (by simp)
  | cons head tail ih =>
    cases List.mem_cons.mp hs with
    | inl h => rw [h]; exact Nat.le_max_left _ _
    | inr h =>
      have := ih h
      exact Nat.le_trans this (Nat.le_max_right _ _)

/-- P4 (ATTAINMENT): the maximum is attained by a real symbol — the
top-1 selection is not synthetic. Induction: either the tail's best is
bounded by the head (head attains), or the tail is non-empty and its
maximum is attained by the induction hypothesis. -/
theorem best_attained (scores : List Nat) (hne : scores ≠ []) :
    ∃ s, s ∈ scores ∧ s = bestScore scores := by
  induction scores with
  | nil => exact absurd rfl hne
  | cons a t ih =>
    unfold bestScore at *
    by_cases h : t.foldr max 0 ≤ a
    · refine ⟨a, List.mem_cons_self, ?_⟩
      simp only [List.foldr_cons]
      rw [Nat.max_eq_left h]
    · have ht : t ≠ [] := by
        intro hnil
        rw [hnil] at h
        simp at h
      obtain ⟨s, hs_mem, hs_eq⟩ := ih ht
      have hle : a ≤ List.foldr max 0 t := Nat.le_of_lt (by omega)
      refine ⟨s, List.mem_cons_of_mem _ hs_mem, ?_⟩
      simp only [List.foldr_cons]
      rw [hs_eq, Nat.max_comm]
      exact (Nat.max_eq_left hle).symm

end RepoMapScoring
-- ---------------------------------------------------------------------------
-- P4b: the damping non-monotonicity witness
-- (YP-REPO-MAP-018 YP-7, loop 4 it. 1)
-- ---------------------------------------------------------------------------

/-! YP-7 recorded the DECISION: the estate's ranking layer is additive,
and damping is deliberately out of scope. This section makes the reason
machine-checked. The additive model's crown property is
`accumulate_mono_append` — adding a reference never lowers any symbol's
rank. A damped model cannot carry that theorem: propagation splits a
node's weight across its out-edges, so adding an out-edge strictly
shrinks every old neighbor's share.

The witness is a two-line Nat model of the YP-7 worked example (scale 10
stands in for the multiply-by-0.5): node j references only `a` with
weight 5 — a's rank is 5. j gains a second out-edge `k` — a's share is
now 50/20 = 2. Adding a reference LOWERED the old neighbor's rank. -/

/-- Damped share of `weight` split across `outdeg` out-edges, scaled by
10 to stay in `Nat` (the honest model of multiply-by-0.5 per hop). -/
def dampedShare (weight outdeg : Nat) : Nat := weight * 10 / (10 * outdeg)

/-- THE ANTI-MONOTONICITY WITNESS: the direct analogue of
`accumulate_mono_append` is FALSE under damping. Same weight, same node
`a` — the only change is one more out-edge from its referrer, and a's
rank drops. `accumulate_mono_append` says the additive model's analogue
is always true; this says the damped model's analogue is always false
for the old neighbors. Monotonicity is not an implementation gap — it is
the property the design rejects. -/
theorem damped_add_lowers_old_neighbor :
    dampedShare 5 2 < dampedShare 5 1 ∧ dampedShare 5 1 = 5 := by
  decide

/-- Same shrink across degrees 2 → 3: the old neighbors keep losing
share with every new out-edge. -/
theorem damped_share_strictly_shrinks :
    dampedShare 5 3 < dampedShare 5 2 ∧ dampedShare 5 2 < dampedShare 5 1 := by
  decide

-- ---------------------------------------------------------------------------
-- P4b (structure level): damping fails append-monotonicity on a graph
-- (YP-REPO-MAP-018 YP-7, loop 4 it. 4)
-- ---------------------------------------------------------------------------

/-! `damped_add_lowers_old_neighbor` witnessed the single-share failure.
Iteration 4 scales the witness to the STRUCTURE level: ranks computed
from an edge list, where the perturbation is exactly the additive
model's scenario — appending one edge. The append-monotonicity theorem
(`accumulate_mono_append`) has NO analogue under damping: appending the
edge (0 → 2) lowers node 1's rank, though node 1's own references did
not change. -/

/-- Out-degree of `j`: how many references leave `j`. -/
def outdeg (refs : List (Nat × Nat)) (j : Nat) : Nat :=
  (refs.filter (fun e => e.1 == j)).length

/-- Damped rank of `target`: every in-reference from `j` contributes
`weight * scale / (10 * outdeg j)` — the weight split across the
referrer's out-edges (scale 10 keeps it in `Nat`). -/
def dampedRank (refs : List (Nat × Nat)) (scale weight target : Nat) : Nat :=
  (refs.filter (fun e => e.2 == target)).foldl
    (fun acc e => acc + weight * scale / (10 * outdeg refs e.1)) 0

/-- THE STRUCTURE-LEVEL ANTI-MONOTONICITY: appending the edge (0 → 2)
— which does not touch node 1 or any of its references — lowers node
1's rank from 5 to 2. The additive model's `accumulate_mono_append`
holds for the same append by arithmetic; the damped model's analogue is
false, and now machine-checked at the graph level. -/
theorem damped_append_lowers :
    dampedRank [(0, 1)] 10 5 1 = 5 ∧
    dampedRank [(0, 1), (0, 2)] 10 5 1 = 2 ∧
    dampedRank [(0, 1), (0, 2)] 10 5 1 < dampedRank [(0, 1)] 10 5 1 := by
  rw [show dampedRank [(0, 1)] 10 5 1 = 5 from rfl,
      show dampedRank [(0, 1), (0, 2)] 10 5 1 = 2 from rfl]
  exact ⟨rfl, rfl, by decide⟩

/-- AND Nat truncation breaks weight conservation: the two shares do
not sum back to the weight (2 + 2 ≠ 5). Exact conservation requires
rationals — so a correct damping model needs the rational/Float
machinery the estate rejected for ranking, on top of sacrificing
monotonicity. Two independent defects, one design. -/
theorem damped_truncation_breaks_conservation :
    dampedShare 5 2 + dampedShare 5 2 ≠ 5 := by decide
