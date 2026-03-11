# ADR-003: WASM Runtime Selection (Wasmtime)

## Status
Accepted

## Context
The Brain component of Clawdius executes LLM reasoning logic and must be isolated from the Host Kernel to prevent "Brain-Leaking" attacks. WebAssembly (WASM) provides the isolation boundary, but runtime selection affects:

- **Security**: Sandbox escape prevention, side-channel resistance
- **Performance**: Startup latency, execution throughput, memory overhead
- **Integration**: Host function bindings, memory management, async support
- **Stability**: API stability, long-term maintenance, community support
- **Rust ecosystem**: Native Rust implementation, cargo integration

### Requirements
1. **Startup latency**: <10ms for Brain initialization
2. **Memory limit**: 4GB maximum WASM linear memory
3. **Fuel metering**: Prevent infinite loops in LLM-generated code
4. **Host functions**: Controlled access to filesystem, network, LLM APIs
5. **Versioned RPC**: Stable Brain-Host communication protocol

## Decision
Select **Wasmtime** as the WASM runtime for the Brain component.

### Configuration
```rust
pub fn create_wasm_runtime() -> Result<wasmtime::Engine, Error> {
    let mut config = Config::new();
    config.wasm_multi_memory(true);
    config.wasm_memory64(false);
    config.max_wasm_stack(1024 * 1024);
    config.consume_fuel(true);
    wasmtime::Engine::new(&config)
}
```

### Resource Limits
| Resource | Limit | Notes |
|----------|-------|-------|
| Memory | 4GB | WASM 32-bit max |
| Stack | 1MB | Configurable |
| Fuel | 1B units | Prevents infinite loops |
| Timeout | 30s | Per invoke operation |

## Consequences

### Positive
- **Rust-native**: Built in Rust, excellent cargo integration, no FFI overhead
- **Bytecode Alliance**: Backed by Mozilla, Fastly, Intel; long-term maintenance assured
- **WASI support**: WebAssembly System Interface for controlled host access
- **Fuel metering**: Built-in deterministic execution limits
- **Fast startup**: ~10ms module instantiation, suitable for interactive use
- **Memory safety**: Rust implementation provides defense-in-depth

### Negative
- **WASM limitations**: No threads, no direct I/O, restricted memory model
- **Debugging complexity**: WASM stack traces less informative than native
- **Compilation overhead**: First module load requires compilation (mitigated by caching)
- **No SIMD on all platforms**: Vector instructions not universally available

## Alternatives Considered

### Wasmer
| Aspect | Wasmer | Wasmtime |
|--------|--------|----------|
| Language | Rust | Rust |
| Backends | LLVM, Cranelift, Singlepass | Cranelift |
| Startup | Similar | Similar |
| Governance | Commercial (Wasmer Inc.) | Non-profit (Bytecode Alliance) |
| WASI | Yes | Yes |
| Fuel | Yes | Yes |

**Rejected**: Commercial governance creates long-term risk; slightly larger binary footprint; Cranelift-only in Wasmtime is sufficient for our needs.

### wasm3
| Aspect | wasm3 | Wasmtime |
|--------|-------|----------|
| Approach | Interpreter | JIT |
| Performance | ~10x slower | Native-like |
| Size | ~100KB | ~2MB |
| Features | Limited | Full |

**Rejected**: Interpreter performance insufficient for LLM reasoning workloads; limited WASI and host function support.

### V8 (via wasm3-rs or similar)
| Aspect | V8 | Wasmtime |
|--------|-----|----------|
| Integration | C++ FFI | Native Rust |
| Size | Large | Moderate |
| Features | Full JS + WASM | WASM only |
| Complexity | High | Low |

**Rejected**: JavaScript engine unnecessary; FFI overhead; large attack surface.

### Native Process Isolation
| Aspect | Native Process | WASM |
|--------|----------------|------|
| Isolation | OS-level | VM-level |
| Startup | ~100ms | ~10ms |
| Memory | Shared OS memory | Sandboxed |
| Control | Limited | Fine-grained |

**Rejected**: Higher startup latency; less control over resource limits; no fuel metering.

## Related Standards
- **WebAssembly Core Specification 2.0**: Runtime compliance
- **WASI Preview 1**: System interface standard
- **IEEE 1016**: Interface specification (Section 7.3)
- **OWASP ASVS V5.3**: Sandbox isolation requirements

## Related ADRs
- ADR-002: Sentinel JIT Sandbox Architecture
- ADR-001: Rust Native Implementation

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
