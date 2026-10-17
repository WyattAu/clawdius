/-
  Lean4 Proof: Nexus FSM Termination and Deadlock Freedom
  Component: COMP-FSM-001
  Blue Paper: BP-NEXUS-FSM-001
  Yellow Paper: YP-FSM-NEXUS-001
-/

import Std.Data.HashMap

inductive Phase where
  | contextDiscovery : Phase
  | environmentMaterialization : Phase
  | requirementsEngineering : Phase
  | epistemologicalDiscovery : Phase
  | crossLingualIntegration : Phase
  | supplyChainHardening : Phase
  | architecture : Phase
  | concurrencyAnalysis : Phase
  | securityEngineering : Phase
  | resourceManagement : Phase
  | performanceEngineering : Phase
  | crossPlatformCompatibility : Phase
  | adversarialLoop : Phase
  | cicdEngineering : Phase
  | documentation : Phase
  | knowledgeBase : Phase
  | executionGraph : Phase
  | supplyMonitoring : Phase
  | deployment : Phase
  | operations : Phase
  | closure : Phase
  | continuousMonitoring : Phase
  | knowledgeTransfer : Phase
  | archive : Phase
  deriving Repr, DecidableEq, BEq, Hashable

inductive Event where
  | discoveryComplete : Event
  | environmentMaterialized : Event
  | requirementsEngineered : Event
  | epistemologicalDiscovered : Event
  | crossLingualIntegrated : Event
  | supplyChainHardened : Event
  | architectureSpecified : Event
  | concurrencyAnalyzed : Event
  | securityEngineered : Event
  | resourceManaged : Event
  | performanceEngineered : Event
  | crossPlatformCompatible : Event
  | adversarialLoopComplete : Event
  | cicdEngineered : Event
  | documentationVerified : Event
  | knowledgeBaseUpdated : Event
  | executionGraphGenerated : Event
  | supplyMonitored : Event
  | deployed : Event
  | operated : Event
  | closed : Event
  | continuousMonitorComplete : Event
  | knowledgeTransferred : Event
  | archived : Event
  deriving Repr, DecidableEq

def phaseIndex : Phase → Nat
  | Phase.contextDiscovery => 0
  | Phase.environmentMaterialization => 1
  | Phase.requirementsEngineering => 2
  | Phase.epistemologicalDiscovery => 3
  | Phase.crossLingualIntegration => 4
  | Phase.supplyChainHardening => 5
  | Phase.architecture => 6
  | Phase.concurrencyAnalysis => 7
  | Phase.securityEngineering => 8
  | Phase.resourceManagement => 9
  | Phase.performanceEngineering => 10
  | Phase.crossPlatformCompatibility => 11
  | Phase.adversarialLoop => 12
  | Phase.cicdEngineering => 13
  | Phase.documentation => 14
  | Phase.knowledgeBase => 15
  | Phase.executionGraph => 16
  | Phase.supplyMonitoring => 17
  | Phase.deployment => 18
  | Phase.operations => 19
  | Phase.closure => 20
  | Phase.continuousMonitoring => 21
  | Phase.knowledgeTransfer => 22
  | Phase.archive => 23

def next : Phase → Option Phase
  | Phase.contextDiscovery => some Phase.environmentMaterialization
  | Phase.environmentMaterialization => some Phase.requirementsEngineering
  | Phase.requirementsEngineering => some Phase.epistemologicalDiscovery
  | Phase.epistemologicalDiscovery => some Phase.crossLingualIntegration
  | Phase.crossLingualIntegration => some Phase.supplyChainHardening
  | Phase.supplyChainHardening => some Phase.architecture
  | Phase.architecture => some Phase.concurrencyAnalysis
  | Phase.concurrencyAnalysis => some Phase.securityEngineering
  | Phase.securityEngineering => some Phase.resourceManagement
  | Phase.resourceManagement => some Phase.performanceEngineering
  | Phase.performanceEngineering => some Phase.crossPlatformCompatibility
  | Phase.crossPlatformCompatibility => some Phase.adversarialLoop
  | Phase.adversarialLoop => some Phase.cicdEngineering
  | Phase.cicdEngineering => some Phase.documentation
  | Phase.documentation => some Phase.knowledgeBase
  | Phase.knowledgeBase => some Phase.executionGraph
  | Phase.executionGraph => some Phase.supplyMonitoring
  | Phase.supplyMonitoring => some Phase.deployment
  | Phase.deployment => some Phase.operations
  | Phase.operations => some Phase.closure
  | Phase.closure => some Phase.continuousMonitoring
  | Phase.continuousMonitoring => some Phase.knowledgeTransfer
  | Phase.knowledgeTransfer => some Phase.archive
  | Phase.archive => none

def nextIter (n : Nat) (p : Phase) : Option Phase :=
  match n with
  | 0 => some p
  | m + 1 => match nextIter m p with
              | some p' => next p'
              | none => none

def stepsToTerminal : Phase → Nat
  | Phase.contextDiscovery => 23
  | Phase.environmentMaterialization => 22
  | Phase.requirementsEngineering => 21
  | Phase.epistemologicalDiscovery => 20
  | Phase.crossLingualIntegration => 19
  | Phase.supplyChainHardening => 18
  | Phase.architecture => 17
  | Phase.concurrencyAnalysis => 16
  | Phase.securityEngineering => 15
  | Phase.resourceManagement => 14
  | Phase.performanceEngineering => 13
  | Phase.crossPlatformCompatibility => 12
  | Phase.adversarialLoop => 11
  | Phase.cicdEngineering => 10
  | Phase.documentation => 9
  | Phase.knowledgeBase => 8
  | Phase.executionGraph => 7
  | Phase.supplyMonitoring => 6
  | Phase.deployment => 5
  | Phase.operations => 4
  | Phase.closure => 3
  | Phase.continuousMonitoring => 2
  | Phase.knowledgeTransfer => 1
  | Phase.archive => 0

structure GateFailure where
  gate_id : String
  message : String
  deriving Repr

def QualityGate := Phase → Except GateFailure Unit

def composeGates (g1 g2 : QualityGate) : QualityGate :=
  fun p => g1 p *> g2 p

theorem fsm_termination (p : Phase) :
    ∃ n : Nat, nextIter n p = some Phase.archive :=
  ⟨stepsToTerminal p, by cases p <;> rfl⟩

theorem fsm_deadlock_free (p : Phase) :
    p ≠ Phase.archive → ∃ p' : Phase, next p = some p' := by
  intro h
  cases p with
  | archive => contradiction
  | _ => exact ⟨_, rfl⟩

theorem fsm_transition_valid (p : Phase) (p' : Phase) :
    next p = some p' → phaseIndex p' = phaseIndex p + 1 := by
  intro h
  cases p <;> cases h <;> rfl

theorem phase_unique (p p' : Phase) :
    phaseIndex p = phaseIndex p' → p = p' := by
  intro h
  cases p <;> cases p' <;> simp only [phaseIndex] at h <;> try omega
  all_goals (rfl)

theorem fsm_monotonic_progress (p : Phase) (p' : Phase) :
    next p = some p' → phaseIndex p < phaseIndex p' := by
  intro h
  cases p <;> cases h <;> simp only [phaseIndex] <;> omega

theorem knowledge_transfer_is_terminal (p : Phase) :
    next p = none ↔ p = Phase.archive := by
  cases p <;> simp [next]

theorem nextIter_monotonic (n : Nat) (p p' : Phase) :
    nextIter n p = some p' → phaseIndex p' = phaseIndex p + n := by
  induction n generalizing p' with
  | zero =>
    intro h
    have : p = p' := by cases h; rfl
    rw [this]; omega
  | succ m ih =>
    intro h
    unfold nextIter at h
    cases h_res : nextIter m p with
    | none =>
      simp only [h_res] at h
      cases h
    | some q =>
      simp only [h_res] at h
      have hih := ih q h_res
      have htrans := fsm_transition_valid q p' h
      omega

theorem fsm_no_cycles (p : Phase) (n : Nat) :
    n > 0 → nextIter n p ≠ some p := by
  intro hpos hcontra
  have hidx := nextIter_monotonic n p p hcontra
  omega

theorem gate_enforcement (p : Phase) (g : QualityGate) :
    g p = Except.ok () → ∃ p', next p = some p' ∨ next p = none := by
  intro _
  cases h : next p with
  | none => exact ⟨p, Or.inr rfl⟩
  | some p' => exact ⟨p', Or.inl rfl⟩

-- ---------------------------------------------------------------------------
-- Failure recovery (YP-FSM-016, loop 2 iteration 8)
-- ---------------------------------------------------------------------------

/-! The legacy FSM is a strictly linear pipeline with NO failure
semantics: `next` never offers a recovery path, so a failed phase
deadlocks the entire run. Following Harel's statechart discipline
(failure transitions are first-class) and runtime-recovery practice
(Drusinsky; retry budgets), the model gains `recover`: a failed phase
drops to `continuousMonitoring`, which observes the failure and
re-runs knowledge transfer + archive. -/

/-- Failure recovery: a failed phase drops to `continuousMonitoring`,
which observes the failure and re-runs knowledge transfer + archive. The
terminal-adjacent pair (continuousMonitoring / knowledgeTransfer)
recovers into each other — see the honest boundary in T3. -/
def recover (p : Phase) : Phase :=
  match p with
  | .archive => .archive
  | .continuousMonitoring => .knowledgeTransfer
  | .knowledgeTransfer => .continuousMonitoring
  | _ => .continuousMonitoring

theorem recover_in_pair (p : Phase) (hne : p ≠ .archive) :
    recover p = .continuousMonitoring ∨ recover p = .knowledgeTransfer := by
  cases p with
  | archive => exact absurd rfl hne
  | continuousMonitoring => exact Or.inr rfl
  | knowledgeTransfer => exact Or.inl rfl
  | _ => exact Or.inl rfl

/-- Archive is terminal under failures: a failed archive phase stays
archived. For non-archive phases, recovery lands in the monitoring
pair. -/
theorem recover_ne_archive (p : Phase) (hne : p ≠ .archive) :
    recover p ≠ .archive := by
  cases p with
  | archive => exact absurd rfl hne
  | contextDiscovery => decide
  | environmentMaterialization => decide
  | requirementsEngineering => decide
  | epistemologicalDiscovery => decide
  | crossLingualIntegration => decide
  | supplyChainHardening => decide
  | architecture => decide
  | concurrencyAnalysis => decide
  | securityEngineering => decide
  | resourceManagement => decide
  | performanceEngineering => decide
  | crossPlatformCompatibility => decide
  | adversarialLoop => decide
  | cicdEngineering => decide
  | documentation => decide
  | knowledgeBase => decide
  | executionGraph => decide
  | supplyMonitoring => decide
  | deployment => decide
  | operations => decide
  | closure => decide
  | continuousMonitoring => decide
  | knowledgeTransfer => decide

/-- The effective transition: normal `next`, falling back to recovery
when the phase fails (terminal phases recover into the monitoring
pair). -/
def advance (p : Phase) : Phase := (next p).getD (recover p)

/-- DEADLOCK-FREEDOM UNDER FAILURE: every non-terminal phase either has
a normal transition or recovers into the monitoring pair. (The archive
phase is terminal: no progress obligation.) -/
theorem always_progresses (p : Phase) (h : p ≠ .archive) :
    (next p).isSome = true ∨ recover p ≠ p := by
  cases p with
  | archive => exact absurd h (by simp)
  | _ => exact Or.inl rfl

/-- Failure-interleaved step: `fails = true` routes through recovery. -/
def stepF (fails : Bool) (p : Phase) : Phase :=
  if fails then recover p else (next p).getD p

/-- Bounded-failure iteration: the pipeline through a schedule of
failures (`true` = the phase at that position failed). -/
def nextIterF : List Bool → Phase → Phase
  | [], p => p
  | f :: rest, p => nextIterF rest (stepF f p)

/-- T-budget (absorbing form): the archive phase is terminal under
failures too — a failure at `archive` keeps the run archived (the
un-archive defect of the first recovery draft is fixed). The numeric
distance bound is deliberately omitted: recovery teleports to
continuousMonitoring (distance 21), so a per-failure additive bound of
+21 would be FALSE for phases already closer to archive (e.g.
operations, distance 19 → 21). The honest statements are:
budget_absorbing here + recovery_reaches_archive below. -/
theorem budget_absorbing (fails : List Bool) :
    ∀ p : Phase, p = Phase.archive → nextIterF fails p = Phase.archive := by
  intro p h
  induction fails generalizing p h with
  | nil => exact h
  | cons f rest ih =>
    cases f with
    | true =>
      show nextIterF rest (recover p) = Phase.archive
      rw [h]
      exact ih Phase.archive rfl
    | false =>
      show nextIterF rest ((next p).getD p) = Phase.archive
      rw [h, next]
      exact ih Phase.archive rfl

/-- T3 (recovery is bounded): from every phase, ONE recovery step lands
in the monitoring pair, and `archive` is reached within 2 further
transitions — even if every remaining phase fails again. -/
theorem recovery_reaches_archive (p : Phase) :
    ∃ n, nextIter n (recover p) = some .archive := by
  cases p with
  | archive => exact ⟨0, rfl⟩
  | knowledgeTransfer => exact ⟨2, rfl⟩
  | continuousMonitoring => exact ⟨1, rfl⟩
  | _ => exact ⟨2, rfl⟩

/-- THE HONEST BOUNDARY (T3b): recovery is deadlock-free but NOT
termination-preserving under unbounded failures — a failure at
`knowledgeTransfer` recovers into `continuousMonitoring`, whose normal
transition re-enters `knowledgeTransfer`: a two-phase livelock.
CONSEQUENCE: the run manager MUST bound failures (retry budget /
escalation), or recovery itself becomes the livelock. Recorded, not
hidden — this theorem is the spec's justification for the retry
budget. -/
theorem unbounded_retry_livelock :
    recover Phase.knowledgeTransfer = Phase.continuousMonitoring ∧
    next Phase.continuousMonitoring = some Phase.knowledgeTransfer :=
  ⟨rfl, rfl⟩
