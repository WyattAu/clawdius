# TLA+ Model Checking - Nexus FSM

## Prerequisites
- Java 11+ (for TLC)
- TLA+ Toolbox or tla2tools CLI

## Running TLC
```bash
java -cp tla2tools.jar tlc2.TLC -config NexusFSM.cfg NexusFSM.tla
```

## Expected Results
All 8 properties should PASS:
- DeadlockFree: No deadlock (always can advance unless terminal)
- TerminalReachable: Terminal phase always reachable (liveness)
- NoCycles: No phase revisited
- MonotonicProgress: Phase index strictly increases
- AllPhasesVisited: All 24 phases visited before terminal
- HistoryIncreasing: History sequence is strictly increasing
- NoSkippedPhases: No phase is skipped
- GatesConsistent: Quality gates match completed phases
