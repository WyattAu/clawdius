# ADR-006: Monoio Async Runtime

## Status
Accepted

## Context
Clawdius requires an asynchronous runtime for:
- **I/O operations**: File watching, network requests, process spawning
- **Concurrency**: Multiple agent tasks executing in parallel
- **Responsiveness**: Non-blocking TUI updates at 60 FPS
- **Deterministic latency**: HFT Broker mode requires bounded execution times

The choice of async runtime affects:
- **Latency variance**: Work-stealing vs. thread-per-core schedulers
- **Memory overhead**: Per-task allocation patterns
- **Integration**: Ecosystem compatibility, driver support
- **Debugging**: Stack traces, profiling, tracing

### Requirements
1. **Bounded latency**: <1ms variance for critical paths
2. **Low memory**: <30MB idle footprint (REQ-6.3)
3. **Fast startup**: <20ms to interactive state (REQ-6.2)
4. **Linux optimization**: Primary deployment target

## Decision
Select **monoio** as the async runtime for Clawdius.

### Rationale
monoio implements a **thread-per-core** model with io_uring:
- No work-stealing: Tasks stay on assigned thread, eliminating scheduler jitter
- io_uring: Zero-copy I/O on Linux for maximum throughput
- Native Rust: Built for Tokio ecosystem compatibility

### Configuration
```rust
pub fn create_runtime() -> Result<monoio::Runtime, Error> {
    monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .with_entries(256)
        .build()
}
```

### Thread Strategy
| Thread Type | Count | Purpose |
|-------------|-------|---------|
| Main (monoio) | 1 per core | I/O, async tasks |
| Blocking pool | Configurable | CPU-bound work |
| Market data (Broker) | Pinned core 0 | Ring buffer producer |
| Strategy (Broker) | Pinned core 1 | Signal generation |

## Consequences

### Positive
- **Deterministic latency**: No work-stealing means no scheduler-induced jitter
- **High performance**: io_uring provides best-in-class Linux I/O
- **Low overhead**: Minimal per-task bookkeeping
- **Tokio compatibility**: Many Tokio-compatible crates work with monoio
- **Small footprint**: Efficient memory usage for idle state

### Negative
- **Linux-focused**: io_uring only available on Linux; fallback needed for macOS/Windows
- **Smaller ecosystem**: Fewer monoio-specific drivers than Tokio
- **Learning curve**: Different mental model from work-stealing runtimes
- **Thread scaling**: Thread-per-core less efficient for I/O-bound workloads on many-core systems

## Alternatives Considered

### Tokio
| Aspect | Tokio | monoio |
|--------|-------|--------|
| Scheduler | Work-stealing | Thread-per-core |
| I/O | epoll/kqueue | io_uring (Linux) |
| Latency variance | Higher | Lower |
| Ecosystem | Largest | Growing |
| Platforms | All | Linux-optimized |

**Rejected**: Work-stealing causes non-deterministic latency spikes; violates HFT Broker sub-millisecond requirements.

### async-std
| Aspect | async-std | monoio |
|--------|-----------|--------|
| API | std-like | Tokio-like |
| Scheduler | Work-stealing | Thread-per-core |
| Performance | Good | Better (io_uring) |

**Rejected**: Work-stealing model; no io_uring support; smaller ecosystem than Tokio.

### smol
| Aspect | smol | monoio |
|--------|------|--------|
| Size | Very small | Small |
| Approach | Blocking executor | io_uring |
| Features | Minimal | Moderate |

**Rejected**: Insufficient features for production system; no io_uring.

### actix-rt (Tokio-based)
**Rejected**: Built on Tokio; inherits work-stealing latency issues.

## Platform Fallback

For non-Linux platforms:
- **macOS**: Use monoio with polling driver (kqueue-based)
- **Windows**: Use monoio with polling driver (IOCP-based)

Note: Full performance benefits only realized on Linux with io_uring.

## Related Standards
- **ISO/IEC 25010**: Performance Efficiency - Time Behavior
- **REQ-6.2**: Boot latency <20ms
- **REQ-6.3**: Idle memory <30MB

## Related ADRs
- ADR-001: Rust Native Implementation
- ADR-007: HFT Broker Zero-GC Design (latency requirements)

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
