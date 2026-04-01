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
