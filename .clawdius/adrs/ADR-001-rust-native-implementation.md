# ADR-001: Rust Native Implementation

## Status
Accepted

## Context
Clawdius requires a high-assurance AI agentic engine that can execute code, manage sandboxes, and provide deterministic behavior. The choice of implementation language fundamentally affects:

- **Memory safety**: Preventing buffer overflows, use-after-free, and data races
- **Performance**: Startup latency, memory footprint, and execution speed
- **Security**: Supply chain attacks, sandboxing capabilities, and TCB minimization
- **Concurrency**: Async runtime behavior and determinism
- **Deployment**: Binary distribution and cross-platform support

Existing AI agent tools primarily use Node.js/TypeScript (e.g., Claude Code) or Python, which introduce:
- Garbage collection pauses affecting latency-sensitive operations
- Larger runtime dependencies and attack surface
- Non-deterministic async behavior
- Difficulty achieving sub-millisecond latency requirements for HFT Broker mode

## Decision
Clawdius will be implemented in **Rust** as a native compiled binary.

### Key Architecture Decisions
1. **Single static binary distribution** (<15MB compressed) with no external runtime dependencies
2. **Zero-cost abstractions** for all core data structures
3. **Compile-time safety guarantees** via ownership system and typestate pattern
4. **Deterministic async runtime** (monoio) for latency-sensitive paths
5. **Native platform integration** via HAL (Hardware Abstraction Layer)

## Consequences

### Positive
- **Memory safety without GC**: Ownership system prevents memory bugs at compile time
- **Deterministic latency**: No garbage collection pauses; sub-millisecond operations achievable
- **Small attack surface**: Minimal runtime dependencies, static binary
- **High performance**: Native code generation with LLVM optimizations
- **Cross-platform**: Compiles to Linux, macOS, Windows with minimal platform-specific code
- **Modern tooling**: cargo, clippy, rustfmt provide consistent development experience

### Negative
- **Steeper learning curve**: Ownership, borrowing, and lifetimes require expertise
- **Longer compile times**: Rust compilation is slower than interpreted languages
- **Smaller ecosystem**: Fewer AI/ML libraries compared to Python
- **WASM compilation complexity**: Brain component requires separate WASM toolchain

## Alternatives Considered

### Node.js/TypeScript (Claude Code approach)
| Aspect | Node.js | Rust |
|--------|---------|------|
| Memory safety | GC-based | Compile-time |
| Latency | Non-deterministic GC | Deterministic |
| Binary size | Runtime required | Static binary |
| Supply chain | npm ecosystem risk | crates.io audits |
| Concurrency | Event loop | Native threads |

**Rejected**: Non-deterministic GC pauses violate HFT Broker latency requirements; larger attack surface from npm dependencies.

### Python
| Aspect | Python | Rust |
|--------|--------|------|
| Performance | Interpreted | Native |
| Deployment | Runtime + packages | Single binary |
| Type safety | Optional | Mandatory |
| Sandboxing | Limited | Native OS integration |

**Rejected**: Poor performance characteristics; deployment complexity; insufficient type safety for high-assurance system.

### Go
| Aspect | Go | Rust |
|--------|-----|------|
| GC pauses | Present | None |
| Type system | Simple | Advanced |
| Error handling | Error values | Result type |
| Zero-cost abstractions | Limited | Full |

**Rejected**: GC pauses violate HFT latency requirements; simpler type system insufficient for typestate pattern enforcement.

### C++
| Aspect | C++ | Rust |
|--------|-----|------|
| Memory safety | Manual | Compile-time |
| Build system | Complex | cargo |
| Modern features | Mixed | Consistent |
| Supply chain | Limited tooling | cargo-audit |

**Rejected**: Manual memory management too error-prone for high-assurance system; complex build tooling.

## Related Standards
- **ISO/IEC 25010**: Software Quality - Reliability, Security, Performance Efficiency
- **NIST SP 800-53**: Security and Privacy Controls (SA-12, SA-15)
- **SLSA**: Supply-chain Levels for Software Artifacts
- **OWASP ASVS**: Application Security Verification Standard

## Related ADRs
- ADR-006: Monoio Async Runtime
- ADR-002: Sentinel JIT Sandbox Architecture
- ADR-005: Nexus FSM Typestate Pattern

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
