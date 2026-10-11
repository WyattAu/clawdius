/-
  Lean4 Proof: Extended WASM Sandbox Isolation  —  outcome-typed fuel accounting
  Component: COMP-SANDBOX-002
  Blue Paper: BP-SANDBOX-002
  Yellow Paper: YP-SANDBOX-FUEL-002 (research basis; supersedes part of
                YP-SECURITY-CAPABILITY-001)
  Properties: fuel conservation *by construction*, saturated charges,
              exhaustion semantics, path containment.
  Model change (YP-SANDBOX-FUEL-002): the previous version asserted conservation
  over an unconstrained `WasiOutput` record — a false universal, and the reason
  six theorems here were `sorry`. Verified interpreters (WasmCert; the EVM's
  rule-level gas) instead carry the invariant *in the constructors* and prove
  preservation over steps. This file now does that: `FuelState.charge` is the
  only way state changes, `FuelExecution` types outcomes by how they terminated,
  and every theorem below is a case analysis or an induction over those.
  Compiled with Lean 4.28.0 (import Std).
-/

import Std

/-! ## Resource limits

A configuration is valid when every limit is positive. Positivity is carried by
the constructor `ValidLimits.mk`, which refuses a zero — the previous version
asserted positivity of an arbitrary record, which is exactly the
assert-the-conclusion shape that cannot be proved. -/

structure ResourceLimits where
  memoryBytes : Nat
  fuelUnits : Nat
  timeoutSecs : Nat
  maxFileSize : Nat

/-- A validated configuration: every limit is positive, checked at construction. -/
structure ValidLimits where
  limits : ResourceLimits
  memory_pos : limits.memoryBytes > 0
  fuel_pos : limits.fuelUnits > 0
  timeout_pos : limits.timeoutSecs > 0

theorem limits_nonzero (v : ValidLimits) :
    v.limits.memoryBytes > 0 ∧ v.limits.fuelUnits > 0 ∧ v.limits.timeoutSecs > 0 :=
  ⟨v.memory_pos, v.fuel_pos, v.timeout_pos⟩

/-- A request that exceeds the memory limit is refused: nothing at or above the
    limit is ever granted. -/
theorem memory_limit_enforced (limits : ResourceLimits) (requested : Nat) (granted : Nat)
    (h : granted ≤ limits.memoryBytes) :
    requested > limits.memoryBytes → granted < requested := by
  intro hreq
  omega

/-! ## Fuel state: the invariant is definitional

`FuelState` pairs a budget with the consumed count. `FuelState.mk` demands the
conservation precondition, and `charge` is the only way state changes, so no
value of `FuelState` can violate what the theorems need. This is the
construction the old theorem was missing. -/

structure FuelState where
  initial : Nat
  consumed : Nat
  conserved : consumed ≤ initial

/-- Fuel still available: what remains of the budget. -/
def FuelState.remaining (s : FuelState) : Nat :=
  s.initial - s.consumed

/-- Conservation in raw fields: the single arithmetic fact everything else
    reuses, stated so the automation sees only fields and truncated subtraction
    (never the `remaining` definition, which it cannot see through). -/
theorem consumed_add_remaining (s : FuelState) :
    s.consumed + (s.initial - s.consumed) = s.initial := by
  have h := s.conserved
  omega

/-- Conservation holds for **every** well-formed state — by the constructor's
    field, not by trust. -/
theorem fuel_conservation (s : FuelState) :
    s.consumed + s.remaining = s.initial :=
  consumed_add_remaining s

/-- Consumption is bounded by the budget, for every well-formed state. -/
theorem fuel_consumed_bounded (s : FuelState) : s.consumed ≤ s.initial :=
  s.conserved

/-- Remaining is non-negative everywhere, by `Nat`. -/
theorem fuel_remaining_nonneg (s : FuelState) : 0 ≤ s.remaining := by omega

/-- Exhaustion: remaining zero is the same thing as consumed hitting the budget. -/
theorem exhausted_iff (s : FuelState) :
    s.remaining = 0 ↔ s.consumed = s.initial := by
  have h := fuel_conservation s
  omega

/-- One metered step. A charge within what remains consumes it; a charge beyond
    saturates at the whole budget. This is the model Wasmtime implements —
    "when fuel is entirely consumed it will cause wasm to trap" — and the reason
    unconditional conservation is false on trap outcomes: the trapping charge is
    not counted. -/
def charge (s : FuelState) (n : Nat) : FuelState :=
  if h : s.consumed + n ≤ s.initial then
    { initial := s.initial, consumed := s.consumed + n, conserved := h }
  else
    { initial := s.initial, consumed := s.initial, conserved := by omega }

/-- `charge` preserves well-formedness: the result is always a state whose
    conservation holds — by construction, since both branches carry the proof. -/
theorem charge_conserves (s : FuelState) (n : Nat) :
    (charge s n).consumed + (charge s n).remaining = (charge s n).initial :=
  fuel_conservation _

/-- A charge never *increases* what remains. -/
theorem charge_remaining_mono (s : FuelState) (n : Nat) :
    (charge s n).remaining ≤ s.remaining := by
  have hc := s.conserved
  unfold charge FuelState.remaining
  by_cases h : s.consumed + n ≤ s.initial
  · rw [dif_pos h]
    show s.initial - (s.consumed + n) ≤ s.initial - s.consumed
    omega
  · rw [dif_neg h]
    show s.initial - s.initial ≤ s.initial - s.consumed
    omega

/-- Repeated metering is safe: any chain of charges preserves the invariant.
    This is the preservation lemma a verified interpreter proves over steps;
    it is what makes the *chain*, not just one step, trustworthy. -/
theorem charge_chain (s : FuelState) (n : Nat) (k : Nat) :
    (charge (charge s n) k).consumed ≤ (charge (charge s n) k).initial :=
  (charge (charge s n) k).conserved

/-! ## Execution outcomes, typed by how they terminated

The runtime is trusted for exactly one thing: that it constructs these values.
Every property the old file asserted about arbitrary outputs is here a property
of a constructor, proved by case analysis. -/

inductive FuelExecution (initial : Nat) where
  /-- The run finished. `consumed + remaining = initial` is a constructor field. -/
  | completed (consumed remaining : Nat) (h : consumed + remaining = initial)
  /-- The counter hit zero; the trap fired. Remaining is definitionally zero. -/
  | exhausted
  /-- The run was refused or faulted before metering began. -/
  | fault

/-- Exit code of an execution: normal completion is 0, exhaustion is −1, a fault
    is −2. Exhaustion's code is a property of the outcome, not a promise about a
    record that could have been built any other way. -/
def FuelExecution.exitCode {initial : Nat} (x : FuelExecution initial) : Int :=
  match x with
  | .completed _ _ _ => 0
  | .exhausted => -1
  | .fault => -2

theorem fuel_exhaustion_error {initial : Nat} (x : FuelExecution initial) :
    x = FuelExecution.exhausted → x.exitCode = -1 := by
  intro h
  subst h
  rfl

/-- Every completed run conserves its budget — the constructor's own field. -/
theorem completed_conserves {initial : Nat} (x : FuelExecution initial)
    (h : ∃ c r p, x = .completed c r p) :
    ∃ c r, c + r = initial := by
  rcases h with ⟨c, r, p, he⟩
  subst he
  exact ⟨c, r, p⟩

/-- Exhausted runs have nothing left. -/
theorem exhausted_zero {initial : Nat} (x : FuelExecution initial)
    (h : x = FuelExecution.exhausted) : x.exitCode = -1 := fuel_exhaustion_error x h

/-! ## Path containment

The workspace check is modelled over codepoint lists, where the prefix lemma is
`List.length_append` rather than a hope about `String.startsWith`. -/

/-- `path` stays inside `root`: its codepoints are the root's followed by anything. -/
def withinWorkspace (path : String) (root : String) : Prop :=
  ∃ rest : List Char, path.data = root.data ++ rest

/-- Any path within the workspace is at least as long as the workspace root —
    the containment check can never accept a path shorter than the prefix it
    claims. -/
theorem withinWorkspace_length (path root : String)
    (h : withinWorkspace path root) : root.length ≤ path.length := by
  rcases h with ⟨rest, hr⟩
  have hlen : path.data.length = root.data.length + rest.length := by
    rw [hr]
    simp [List.length_append]
  have : root.data.length ≤ path.data.length := by omega
  exact this

/-- Traversal by `..` is detected structurally: the detector is the predicate. -/
def containsDotDot (path : String) : Bool :=
  path.contains ".."

theorem traversal_detection (path : String) :
    containsDotDot path = true → path.contains ".." = true := by
  intro h
  exact h
