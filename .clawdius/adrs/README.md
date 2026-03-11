# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for the Clawdius project. ADRs document significant architectural decisions along with their context and consequences.

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-001](ADR-001-rust-native-implementation.md) | Rust Native Implementation | Accepted | 2026-03-08 |
| [ADR-002](ADR-002-sentinel-jit-sandbox.md) | Sentinel JIT Sandbox Architecture | Accepted | 2026-03-08 |
| [ADR-003](ADR-003-wasmtime-selection.md) | WASM Runtime Selection (Wasmtime) | Accepted | 2026-03-08 |
| [ADR-004](ADR-004-graph-rag-architecture.md) | Graph-RAG Architecture | Accepted | 2026-03-08 |
| [ADR-005](ADR-005-nexus-fsm-typestate.md) | Nexus FSM Typestate Pattern | Accepted | 2026-03-08 |
| [ADR-006](ADR-006-monoio-async-runtime.md) | Monoio Async Runtime | Accepted | 2026-03-08 |
| [ADR-007](ADR-007-hft-broker-zero-gc.md) | HFT Broker Zero-GC Design | Accepted | 2026-03-08 |

## ADR Summaries

### ADR-001: Rust Native Implementation
Decision to implement Clawdius in Rust as a native compiled binary rather than Node.js/TypeScript (Claude Code approach) or Python. Key factors: memory safety without GC, deterministic latency, small attack surface, single binary deployment.

### ADR-002: Sentinel JIT Sandbox Architecture
Decision to implement a 4-tier sandboxing system (Native, Container, WASM, Hardened) with capability-based security. Enables defense-in-depth while maintaining performance for trusted code execution.

### ADR-003: WASM Runtime Selection (Wasmtime)
Decision to use Wasmtime for the Brain component's WASM sandbox. Chosen for Rust-native implementation, Bytecode Alliance governance, fuel metering, and fast startup.

### ADR-004: Graph-RAG Architecture
Decision to use SQLite + tree-sitter for structural AST indexing and LanceDB for semantic vector search. Hybrid approach enables both precise code understanding and natural language queries.

### ADR-005: Nexus FSM Typestate Pattern
Decision to use the Typestate pattern for the 24-phase R&D lifecycle FSM. Provides compile-time guarantees that illegal phase transitions are unrepresentable.

### ADR-006: Monoio Async Runtime
Decision to use monoio (thread-per-core with io_uring) instead of Tokio. Eliminates work-stealing scheduler jitter for deterministic latency in HFT Broker mode.

### ADR-007: HFT Broker Zero-GC Design
Decision to use ring buffers, arena allocation, and lock-free data structures for the HFT Broker. Enables sub-millisecond latency with provable WCET bounds.

## ADR Template

New ADRs should follow this format:

```markdown
# ADR-XXX: [Title]

## Status
[Proposed|Accepted|Rejected|Superseded|Deprecated]

## Context
[What is the issue that we're seeing that is motivating this decision?]

## Decision
[What is the change that we're proposing and/or doing?]

## Consequences
[What becomes easier or more difficult to do because of this change?]

## Alternatives Considered
[What other options were considered?]

## Related Standards
[Applicable ISO/IEEE/IEC/NIST standards]

## Related ADRs
[Links to related ADRs]

## Date
[YYYY-MM-DD]

## Author
[Agent/Person]
```

## Related Documents

- **Yellow Papers**: `.clawdius/specs/01_research/` - Theoretical foundations
- **Blue Papers**: `.clawdius/specs/02_architecture/` - Architectural specifications
- **Requirements**: `.clawdius/specs/00_requirements/requirements.md` - SRS document

## References

- [Documenting Architecture Decisions - Michael Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [Architecture Decision Records - GitHub](https://adr.github.io/)
- [ISO/IEC/IEEE 42010:2011](https://www.iso.org/standard/50508.html) - Systems and software engineering
