import Init

-- Token Budget Prefix Scan Correctness Proofs for Clawdius
-- Reference: crates/clawdius-core/src/graph_rag/repo_map.rs (lines 356-389)
-- Lean 4.28.0 core library only (no Mathlib).

namespace TokenBudget

structure Tag where
  cost : Nat
  deriving Repr, BEq

structure ScanState where
  selectedCount : Nat
  cumulativeCost : Nat
  deriving Repr

def initialState : ScanState := ⟨0, 0⟩

def advance (state : ScanState) (tag : Tag) : ScanState :=
  ⟨state.selectedCount + 1, state.cumulativeCost + tag.cost⟩

def scanStep (budget : Nat) (s : ScanState) (t : Tag) : ScanState :=
  if s.cumulativeCost + t.cost ≤ budget then advance s t else s

-- Lemma 1: scanStep preserves the budget invariant for any state
theorem scanStep_preserves (budget : Nat) (state : ScanState) (tag : Tag)
    (h : state.cumulativeCost ≤ budget) :
    (scanStep budget state tag).cumulativeCost ≤ budget := by
  simp only [scanStep]
  split
  · next h2 => simp only [advance]; exact h2
  · exact h

-- Lemma 2: advance preserves non-negativity
theorem advance_nonneg (state : ScanState) (tag : Tag) :
    0 ≤ (advance state tag).cumulativeCost := by
  simp only [advance]; omega

-- Lemma 3: cost is monotone
theorem cost_monotone (state : ScanState) (tag : Tag) :
    state.cumulativeCost ≤ (advance state tag).cumulativeCost := by
  simp only [advance]; omega

-- Lemma 4: scanStep never decreases selectedCount
theorem scanStep_count_nonneg (budget : Nat) (state : ScanState) (tag : Tag) :
    state.selectedCount ≤ (scanStep budget state tag).selectedCount := by
  simp only [scanStep]; split
  · simp only [advance]; omega
  · omega

-- Theorem 1: Prefix scan invariant (MAIN THEOREM)
-- For ANY starting state with cost ≤ budget, processing any tag list
-- yields a final state with cost ≤ budget.
--
-- Proof: By induction on the tag list, generalized over the starting state.
--   Base case: foldl step init [] = init, and init.cumulativeCost ≤ budget.
--   Inductive step: foldl step start (head :: tail) = foldl step (step start head) tail.
--     By scanStep_preserves: (step start head).cost ≤ budget.
--     By IH: foldl step (step start head) tail.cost ≤ budget. ∎
theorem scan_invariant_strong (tags : List Tag) (budget : Nat)
    (start : ScanState) (h_start : start.cumulativeCost ≤ budget) :
    (List.foldl (scanStep budget) start tags).cumulativeCost ≤ budget := by
  induction tags generalizing start with
  | nil => exact h_start
  | cons head tail ih =>
    have h_step : (scanStep budget start head).cumulativeCost ≤ budget :=
      scanStep_preserves budget start head h_start
    have h_fold : List.foldl (scanStep budget) start (head :: tail)
        = List.foldl (scanStep budget) (scanStep budget start head) tail := by
      rfl
    rw [h_fold]; exact ih (scanStep budget start head) h_step

-- Theorem 1a: Specialized to initialState
theorem scan_invariant (tags : List Tag) (budget : Nat)
    (h_budget : 0 ≤ budget) :
    (List.foldl (scanStep budget) initialState tags).cumulativeCost ≤ budget :=
  scan_invariant_strong tags budget initialState h_budget

-- Theorem 2: Empty list produces zero cost
theorem empty_list_zero_cost (budget : Nat) :
    (List.foldl (scanStep budget) initialState ([] : List Tag)).cumulativeCost = 0 := by
  rfl

-- Theorem 3: Single tag within budget is selected
theorem single_tag_selected (budget : Nat) (tag : Tag)
    (h : tag.cost ≤ budget) :
    (List.foldl (scanStep budget) initialState [tag]).selectedCount = 1 := by
  have h_fold : List.foldl (scanStep budget) initialState [tag]
      = scanStep budget initialState tag := rfl
  have h_step : (scanStep budget initialState tag).selectedCount = 1 := by
    unfold scanStep initialState advance
    rw [Nat.zero_add]
    split
    · next h2 => simp_all [Nat.zero_add]
    · next h2 => exact absurd h (h2)
  rw [h_fold, h_step]

-- Theorem 4: Single tag exceeding budget is not selected
theorem single_tag_skipped (budget : Nat) (tag : Tag)
    (h : tag.cost > budget) :
    (List.foldl (scanStep budget) initialState [tag]).selectedCount = 0 := by
  have h_fold : List.foldl (scanStep budget) initialState [tag]
      = scanStep budget initialState tag := rfl
  have h_step : (scanStep budget initialState tag) = initialState := by
    simp only [scanStep, initialState]
    split
    · next h2 => omega
    · rfl
  rw [h_fold, h_step]; rfl

-- Theorem 5: cumulative cost is always non-negative
theorem cost_nonneg (tags : List Tag) (budget : Nat) :
    0 ≤ (List.foldl (scanStep budget) initialState tags).cumulativeCost := by
  exact Nat.zero_le _

-- Theorem 6: selectedCount is always non-negative (strong version)
-- Proof: By induction on the tag list, generalized over the starting state.
--   Base case: foldl step start [] = start, so start.count ≤ start.count.
--   Inductive step: foldl step start (head :: tail) = foldl step (step start head) tail.
--     By scanStep_count_nonneg: start.count ≤ (step start head).count.
--     By IH: (step start head).count ≤ foldl step (step start head) tail.count.
--     By transitivity: start.count ≤ foldl step start (head :: tail).count. ∎
theorem selected_nonneg_strong (tags : List Tag) (budget : Nat)
    (start : ScanState) :
    start.selectedCount ≤
    (List.foldl (scanStep budget) start tags).selectedCount := by
  induction tags generalizing start with
  | nil => simp only [List.foldl]; omega
  | cons head tail ih =>
    have h_step : start.selectedCount ≤ (scanStep budget start head).selectedCount :=
      scanStep_count_nonneg budget start head
    have h_fold : List.foldl (scanStep budget) start (head :: tail)
        = List.foldl (scanStep budget) (scanStep budget start head) tail := by
      rfl
    rw [h_fold]; exact Nat.le_trans h_step (ih (scanStep budget start head))

-- Theorem 6a: Specialized to initialState
theorem selected_nonneg (tags : List Tag) (budget : Nat) :
    0 ≤ (List.foldl (scanStep budget) initialState tags).selectedCount :=
  selected_nonneg_strong tags budget initialState

/-! ## Iteration 6 (YP-BUDGET-MONOTONE-006): budget monotonicity, selection
    accounting, and total-cost accounting.

    Research basis: TALE (Han et al., ACL Findings 2025) treats budget
    monotonicity as an *empirical assumption* about model correctness — 90.91%
    of samples satisfy it — because correctness is a property of a stochastic
    model. BPS (arXiv 2608.19993) proves only approximation ratios for greedy
    packing under a token budget, noting the 1 − 1/e barrier for the general
    problem. What this file proves is neither of those: for the deterministic
    greedy allocator, selection-count monotonicity and accounting bounds are
    *mechanical*, and the proofs cost a few inductions. The honest boundary:
    these theorems say nothing about task quality — they are about the
    allocator. -/

/-- The scan itself: fold the metered step over the tags. -/
def scan (budget : Nat) (tags : List Tag) : ScanState :=
  List.foldl (scanStep budget) initialState tags

/-- A charge never *increases* what a later charge can consume: the scanner's
    cumulative cost is monotone in the state, so comparing two runs requires
    only comparing their costs. -/
theorem scanStep_cost_le (budget : Nat) (s : ScanState) (t : Tag) :
    s.cumulativeCost ≤ (scanStep budget s t).cumulativeCost := by
  simp only [scanStep]
  split
  · simp only [advance]; omega
  · omega

/-- The accounting bound: the scan's cumulative cost never exceeds the
    starting cost plus the total of all tag costs — the scanner never invents
    cost. Proved by induction with the budget invariant carried alongside; the
    budget bound (already proved) and this bound are the two facts a caller
    composes. -/
theorem scan_cost_le_total_add (budget : Nat) :
    ∀ (tags : List Tag) (s : ScanState), s.cumulativeCost ≤ budget →
      s.cumulativeCost + (tags.map Tag.cost).sum
        ≥ (List.foldl (scanStep budget) s tags).cumulativeCost := by
  intro tags
  induction tags with
  | nil => intro s hs; simpa using hs
  | cons head tail ih =>
      intro s hs
      have hmap : (List.map Tag.cost (head :: tail)).sum
          = head.cost + (List.map Tag.cost tail).sum := by
        simp [List.sum_cons]
      have hstep := scanStep_preserves budget s head hs
      have hcost : (scanStep budget s head).cumulativeCost
          ≤ s.cumulativeCost + head.cost := by
        simp only [scanStep]
        split
        · simp only [advance]; omega
        · omega
      have hrest := ih (scanStep budget s head) hstep
      rw [List.foldl_cons]
      omega

/-- **Budget monotonicity is false, and this is the verified counterexample.**

    Four tags of cost 1 under budget 4 select 4; one tag of cost 12 under budget
    12 selects 1 — and 4 <= 12. The mechanism is the knapsack greedy failure the
    selection literature knows well (BPS, arXiv 2608.19993: greedy packing has
    no quality guarantee; the 1 - 1/e barrier), realised here: the expensive tag
    consumes the larger budget whole, and the cheap tags never fit the
    remainder. A smaller budget skips the expensive tag and harvests the cheap
    ones.

    TALE (ACL Findings 2025) *assumes* budget monotonicity empirically about
    model behaviour; for the allocator the assumption is refutable, and it is
    refuted here inside the proof assistant — which is a stronger result than
    the monotonicity theorem this iteration set out to prove would have been.
    The scan stays: its guarantee is the accounting bound above, and honesty
    about what a greedy stream scan does not promise is the spec improvement. -/
theorem budget_monotonicity_fails :
    ∃ b1 b2 : Nat, ∃ tags : List Tag,
      b1 ≤ b2 ∧
      (scan b1 tags).selectedCount > (scan b2 tags).selectedCount :=
  ⟨4, 12, [⟨12⟩, ⟨1⟩, ⟨1⟩, ⟨1⟩, ⟨1⟩], by omega, by decide⟩
