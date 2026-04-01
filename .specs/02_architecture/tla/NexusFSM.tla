------------------------------- MODULE NexusFSM -------------------------------
EXTENDS Naturals, Sequences, TLC

CONSTANTS MaxPhase

Phase == 0..MaxPhase

PhaseName(phase) == 
    IF phase = 0 THEN "ContextDiscovery"
    ELSE IF phase = 1 THEN "DomainAnalysis"
    ELSE IF phase = 2 THEN "StakeholderMapping"
    ELSE IF phase = 3 THEN "RequirementsElicitation"
    ELSE IF phase = 4 THEN "RequirementsAnalysis"
    ELSE IF phase = 5 THEN "RequirementsValidation"
    ELSE IF phase = 6 THEN "ArchitectureDesign"
    ELSE IF phase = 7 THEN "InterfaceSpecification"
    ELSE IF phase = 8 THEN "SecurityModeling"
    ELSE IF phase = 9 THEN "TechnologySelection"
    ELSE IF phase = 10 THEN "ImplementationPlanning"
    ELSE IF phase = 11 THEN "ResourceAllocation"
    ELSE IF phase = 12 THEN "RiskAssessment"
    ELSE IF phase = 13 THEN "CoreImplementation"
    ELSE IF phase = 14 THEN "FeatureDevelopment"
    ELSE IF phase = 15 THEN "Integration"
    ELSE IF phase = 16 THEN "UnitTesting"
    ELSE IF phase = 17 THEN "IntegrationTesting"
    ELSE IF phase = 18 THEN "SystemTesting"
    ELSE IF phase = 19 THEN "SecurityAudit"
    ELSE IF phase = 20 THEN "PerformanceValidation"
    ELSE IF phase = 21 THEN "AcceptanceTesting"
    ELSE IF phase = 22 THEN "Deployment"
    ELSE IF phase = 23 THEN "KnowledgeTransfer"
    ELSE "Unknown"

VARIABLES current_phase, history, gates_passed

TypeInvariant == 
    /\ current_phase \in Phase
    /\ history \in Seq(Phase)
    /\ gates_passed \in SUBSET Phase

Init == 
    /\ current_phase = 0
    /\ history = <<0>>
    /\ gates_passed = {}

Next == 
    /\ current_phase < MaxPhase
    /\ current_phase' = current_phase + 1
    /\ history' = Append(history, current_phase + 1)
    /\ gates_passed' = gates_passed \union {current_phase}

Spec == Init /\ [][Next]_<<current_phase, history, gates_passed>>

\* P1: Deadlock Freedom - always possible to make progress unless at terminal
DeadlockFree == 
    ~(current_phase = MaxPhase) => ENABLED Next

\* P2: Liveness - terminal phase is always reachable
TerminalReachable == 
    <><>(current_phase = MaxPhase)

\* P3: No Cycles - the FSM never returns to a previous phase
NoCycles == 
    [](current_phase \notin SubSeq(history, 1, Len(history) - 1))

\* P4: Monotonic Progress - phase index strictly increases on each transition
MonotonicProgress == 
    [][current_phase' > current_phase]_current_phase

\* P5: All phases visited before terminal
AllPhasesVisited ==
    [](current_phase = MaxPhase => 
        Len(history) = MaxPhase + 1)

\* P6: History is strictly increasing
HistoryIncreasing ==
    [](\A i, j \in 1..Len(history):
        i < j => history[i] < history[j])

\* P7: No skipped phases (sequences are 1-indexed in TLA+)
NoSkippedPhases ==
    [](\A i \in 2..Len(history):
        history[i] = history[i-1] + 1)

\* P8: Gates passed is consistent with history
GatesConsistent ==
    [](\A p \in gates_passed:
        p \in SubSeq(history, 1, Len(history) - 1))

================================================================================
