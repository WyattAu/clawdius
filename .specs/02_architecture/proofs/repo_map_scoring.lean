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

end RepoMapScoring
