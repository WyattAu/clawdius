# Task Dependency Graph

This document visualizes the task dependencies for the Clawdius implementation plan using topological sort.

## Overview

- **Total Tasks:** 47
- **Critical Path Length:** 16 tasks
- **Estimated Total Effort:** 228 hours

## Dependency Graph

```mermaid
graph TD
    subgraph M1["Milestone 1: Core Infrastructure"]
        HOST001[TASK-HOST-001<br/>monoio Runtime]
        HOST002[TASK-HOST-002<br/>HostKernel]
        HOST003[TASK-HOST-003<br/>HAL Linux]
        HOST004[TASK-HOST-004<br/>HAL macOS]
        HOST005[TASK-HOST-005<br/>ComponentRegistry]
        FSM001[TASK-FSM-001<br/>24-Phase Enum]
        FSM002[TASK-FSM-002<br/>Transition Engine]
        FSM003[TASK-FSM-003<br/>ArtifactRegistry]
        FSM004[TASK-FSM-004<br/>Changelog]
        
        HOST001 --> HOST002
        HOST002 --> HOST003
        HOST003 --> HOST004
        HOST002 --> HOST005
        HOST002 --> FSM001
        FSM001 --> FSM002
        FSM001 --> FSM003
        FSM003 --> FSM004
    end

    subgraph M2["Milestone 2: Security Layer"]
        SENT001[TASK-SENT-001<br/>Tier Selection]
        SENT002[TASK-SENT-002<br/>Capabilities]
        SENT003[TASK-SENT-003<br/>Settings Validation]
        SENT004[TASK-SENT-004<br/>Secret Proxy]
        SENT005[TASK-SENT-005<br/>Sandbox Spawner]
        BRAIN001[TASK-BRAIN-001<br/>wasmtime Runtime]
        BRAIN002[TASK-BRAIN-002<br/>RPC Protocol]
        BRAIN003[TASK-BRAIN-003<br/>LLM Orchestrator]
        BRAIN004[TASK-BRAIN-004<br/>SOP Engine]
        
        HOST003 --> SENT001
        SENT001 --> SENT002
        SENT001 --> SENT003
        SENT002 --> SENT004
        HOST003 --> SENT004
        SENT002 --> SENT005
        SENT003 --> SENT005
        SENT002 --> BRAIN001
        BRAIN001 --> BRAIN002
        BRAIN002 --> BRAIN003
        BRAIN002 --> BRAIN004
    end

    subgraph M3["Milestone 3: Intelligence Layer"]
        GRAPH001[TASK-GRAPH-001<br/>SQLite AST]
        GRAPH002[TASK-GRAPH-002<br/>tree-sitter]
        GRAPH003[TASK-GRAPH-003<br/>LanceDB Vectors]
        GRAPH004[TASK-GRAPH-004<br/>Hybrid Query]
        GRAPH005[TASK-GRAPH-005<br/>MCP Host]
        
        HOST002 --> GRAPH001
        GRAPH001 --> GRAPH002
        GRAPH001 --> GRAPH003
        GRAPH002 --> GRAPH004
        GRAPH003 --> GRAPH004
        GRAPH004 --> GRAPH005
    end

    subgraph M4["Milestone 4: Domain Layer"]
        BROKER001[TASK-BROKER-001<br/>Ring Buffer]
        BROKER002[TASK-BROKER-002<br/>Wallet Guard]
        BROKER003[TASK-BROKER-003<br/>Arena Allocator]
        BROKER004[TASK-BROKER-004<br/>Signal Engine]
        BROKER005[TASK-BROKER-005<br/>Notifications]
        
        HOST002 --> BROKER001
        BROKER001 --> BROKER002
        BROKER001 --> BROKER003
        BROKER002 --> BROKER004
        BROKER003 --> BROKER004
        BROKER004 --> BROKER005
    end

    subgraph M5["Milestone 5: Interface Layer"]
        TUI001[TASK-TUI-001<br/>TUI Framework]
        TUI002[TASK-TUI-002<br/>Dashboard]
        TUI003[TASK-TUI-003<br/>Chat Interface]
        TUI004[TASK-TUI-004<br/>Broker Monitor]
        INT001[TASK-INT-001<br/>CLI Dispatcher]
        INT002[TASK-INT-002<br/>Chat Workflow]
        INT003[TASK-INT-003<br/>Refactor Workflow]
        
        FSM002 --> TUI001
        GRAPH004 --> TUI001
        TUI001 --> TUI002
        TUI001 --> TUI003
        BRAIN003 --> TUI003
        TUI001 --> TUI004
        BROKER004 --> TUI004
        FSM002 --> INT001
        GRAPH004 --> INT001
        INT001 --> INT002
        BRAIN003 --> INT002
        GRAPH004 --> INT002
        INT001 --> INT003
        GRAPH004 --> INT003
    end

    subgraph M6["Milestone 6: Verification"]
        TEST001[TASK-TEST-001<br/>Unit Tests]
        TEST002[TASK-TEST-002<br/>Integration Tests]
        SEC001[TASK-SEC-001<br/>Security Audit]
        PERF001[TASK-PERF-001<br/>Performance Validation]
        DOC001[TASK-DOC-001<br/>Documentation]
        
        INT002 --> TEST001
        INT003 --> TEST001
        TEST001 --> TEST002
        TEST001 --> SEC001
        TEST001 --> PERF001
        TEST002 --> DOC001
    end

    style HOST001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style HOST002 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style FSM001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style FSM002 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style SENT001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style SENT002 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style BRAIN001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style GRAPH001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style GRAPH002 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style BROKER001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style BROKER002 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style TUI001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style INT001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style TEST001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style SEC001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
    style PERF001 fill:#ff6b6b,stroke:#c92a2a,color:#fff
```

## Critical Path

The critical path (highlighted in red) represents the longest sequence of dependent tasks:

```
TASK-HOST-001 (4h)
    └─→ TASK-HOST-002 (6h)
        └─→ TASK-FSM-001 (6h)
            └─→ TASK-FSM-002 (8h)
                └─→ TASK-SENT-001 (6h)
                    └─→ TASK-SENT-002 (8h)
                        └─→ TASK-BRAIN-001 (6h)
                            └─→ TASK-GRAPH-001 (8h)
                                └─→ TASK-GRAPH-002 (8h)
                                    └─→ TASK-BROKER-001 (8h)
                                        └─→ TASK-BROKER-002 (8h)
                                            └─→ TASK-TUI-001 (8h)
                                                └─→ TASK-INT-001 (6h)
                                                    └─→ TASK-TEST-001 (12h)
                                                        └─→ TASK-SEC-001 (8h)
                                                        └─→ TASK-PERF-001 (6h)
```

**Critical Path Total:** 128 hours

## Parallel Execution Opportunities

Tasks that can run in parallel after their dependencies are satisfied:

### After TASK-HOST-002
- TASK-HOST-003 (HAL Linux)
- TASK-HOST-005 (ComponentRegistry)
- TASK-FSM-001 (Phase Enum)
- TASK-GRAPH-001 (SQLite AST)
- TASK-BROKER-001 (Ring Buffer)

### After TASK-SENT-002
- TASK-SENT-004 (Secret Proxy)
- TASK-SENT-005 (Sandbox Spawner)
- TASK-BRAIN-001 (wasmtime Runtime)

### After TASK-GRAPH-004
- TASK-GRAPH-005 (MCP Host)
- TASK-TUI-001 (TUI Framework)
- TASK-INT-001 (CLI Dispatcher)

## Component Dependency Summary

| Component | Depends On | Provides To |
|-----------|------------|-------------|
| COMP-HOST-001 | None | FSM, SENTINEL, BRAIN, GRAPH, BROKER |
| COMP-FSM-001 | HOST | TUI, CLI |
| COMP-SENTINEL-001 | HOST | BRAIN |
| COMP-BRAIN-001 | HOST, SENTINEL | TUI, Workflows |
| COMP-GRAPH-001 | HOST | TUI, BROKER, Workflows |
| COMP-BROKER-001 | HOST, GRAPH | TUI, Notifications |
| COMP-TUI-001 | HOST, FSM, GRAPH | User Interface |

## Milestone Gates

Each milestone has entry and exit gates:

### M1: Core Infrastructure
- **Entry:** None (foundation)
- **Exit:** All HOST and FSM tasks complete, tests passing

### M2: Security Layer
- **Entry:** M1 complete
- **Exit:** Sentinel and Brain tasks complete, capability tests passing

### M3: Intelligence Layer
- **Entry:** M1 complete
- **Exit:** Graph-RAG tasks complete, indexing validated

### M4: Domain Layer
- **Entry:** M1, M3 complete
- **Exit:** Broker tasks complete, WCET verified <100µs

### M5: Interface Layer
- **Entry:** M1, M2, M3, M4 complete
- **Exit:** TUI and CLI tasks complete, E2E workflows passing

### M6: Verification
- **Entry:** All previous milestones complete
- **Exit:** 85%+ coverage, security audit passed, docs complete

## Risk Areas

High-risk dependencies that could delay the critical path:

1. **TASK-HOST-003** (HAL Linux) - Platform-specific, may require iteration
2. **TASK-SENT-005** (Sandbox Spawner) - Security-critical, needs thorough testing
3. **TASK-BRAIN-001** (wasmtime) - Complex integration, potential version conflicts
4. **TASK-GRAPH-002** (tree-sitter) - Multi-language support, grammar issues
5. **TASK-BROKER-001** (Ring Buffer) - Lock-free complexity, memory ordering
