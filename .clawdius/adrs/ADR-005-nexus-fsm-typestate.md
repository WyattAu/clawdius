# ADR-005: Nexus FSM Typestate Pattern

## Status
Accepted

## Context
Clawdius enforces a 24-phase R&D lifecycle (Nexus FSM) to ensure engineering rigor:
- **Phases**: Context Discovery → Domain Analysis → ... → Knowledge Transfer
- **Quality gates**: Exit/entry conditions between phases
- **Artifact tracking**: Cryptographic hashes of deliverables
- **Audit trail**: CHANGELOG.md with all state transitions

Traditional FSM implementations have issues:
- **Runtime state checks**: Allow illegal states to exist at runtime
- **Enum-only approaches**: Can represent invalid combinations
- **Database-backed state**: Non-deterministic, slower, potential for corruption

The system requires **compile-time guarantees** that:
1. Illegal phase transitions cannot be expressed in code
2. Each phase has a unique type that cannot be constructed incorrectly
3. State consumption prevents duplicate transitions

## Decision
Implement the **Typestate Pattern** for the Nexus FSM using Rust's ownership system.

### Core Pattern
```rust
pub trait PhaseState: sealed::Sealed {
    type Next: PhaseState;
    fn phase(&self) -> Phase;
    fn validate_transition(&self, event: Event) -> Result<Self::Next, TransitionError>;
}

pub struct Fsm<S: PhaseState> {
    state: S,
    artifacts: ArtifactRegistry,
    gates: GateEvaluator,
}

impl<S: PhaseState> Fsm<S> {
    pub fn transition(self, event: Event) -> Result<Fsm<S::Next>, TransitionError> {
        let next_state = self.state.validate_transition(event)?;
        self.gates.evaluate_exit(self.state.phase())?;
        self.gates.evaluate_entry(next_state.phase())?;
        let hash = self.artifacts.compute_hash();
        self.log_transition(&self.state, &next_state, &hash)?;
        Ok(Fsm {
            state: next_state,
            artifacts: self.artifacts,
            gates: self.gates,
        })
    }
}
```

### 24 Phase Types
Each phase is a distinct type:
```rust
pub struct ContextDiscovery;
pub struct DomainAnalysis;
pub struct StakeholderMapping;
// ... 21 more phases ...
pub struct KnowledgeTransfer;
```

### Key Properties
1. **State consumption**: `transition(self)` moves ownership, preventing reuse
2. **Type-level transitions**: `S::Next` is determined at compile time
3. **Sealed trait**: External crates cannot implement `PhaseState`
4. **Gate enforcement**: Quality gates checked before transition completes

## Consequences

### Positive
- **Compile-time safety**: Invalid transitions caught at compile time, not runtime
- **Self-documenting**: Type signatures show valid state progressions
- **No runtime overhead**: Zero-cost abstraction over simple state machine
- **Exhaustive matching**: Compiler ensures all phases are handled
- **Memory efficiency**: FSM state is a single byte (Phase enum)

### Negative
- **Verbosity**: Each phase requires a separate type definition
- **Learning curve**: Developers must understand typestate pattern
- **Generic complexity**: `Fsm<S>` requires generic bounds throughout
- **Limited flexibility**: Dynamic phase selection requires additional machinery

## Alternatives Considered

### Runtime Enum Matching
```rust
pub enum Phase { ContextDiscovery, DomainAnalysis, /* ... */ }

fn transition(current: Phase, event: Event) -> Result<Phase, Error> {
    match (current, event) {
        (Phase::ContextDiscovery, Event::DiscoveryComplete) => Ok(Phase::DomainAnalysis),
        // ...
        _ => Err(Error::InvalidTransition),
    }
}
```

**Rejected**: Illegal states representable at runtime; no compile-time protection; runtime checks add overhead.

### Database-Backed State
**Rejected**: Non-deterministic behavior possible; external dependency; slower; potential for corruption.

### 12-Phase Model
**Rejected**: Insufficient granularity for quality gate enforcement; 24 phases provide better checkpoints.

### Actor-Based State Machine
**Rejected**: Additional complexity; message passing overhead; not necessary for single-threaded FSM logic.

## Related Standards
- **IEEE 1016**: State Machine Specification (Section 6.2)
- **ISO 25010**: Completeness (24 phases defined)
- **CMMI Level 3**: Process Definition with quality gates
- **EARS Syntax**: Requirements notation for state-driven behavior

## Related ADRs
- ADR-001: Rust Native Implementation (ownership enables typestate)
- ADR-002: Sentinel JIT Sandbox Architecture (quality gates)

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
