# ADR-007: HFT Broker Zero-GC Design

## Status
Accepted

## Context
The HFT (High-Frequency Trading) Broker mode of Clawdius operates under extreme temporal constraints:
- **Signal-to-execution latency**: <1ms end-to-end
- **Risk check WCET**: <100μs worst-case execution time
- **Market data processing**: <1μs per message
- **Zero GC pauses**: No garbage collection allowed on hot path

Traditional software engineering approaches fail in this domain:
- **Heap allocation**: Non-deterministic allocation/deallocation times
- **Garbage collection**: Pause times in milliseconds unacceptable
- **Lock contention**: Mutex overhead introduces jitter
- **Cache misses**: Poor locality degrades performance

### Regulatory Requirements
- **SEC Rule 15c3-5**: Pre-trade risk controls must be enforced
- **MiFID II**: Best execution requirements with timestamping
- **Audit trails**: All risk decisions must be logged

## Decision
Implement a **Zero-GC architecture** for the HFT Broker using:

### 1. Lock-Free Ring Buffer (SPSC)
```rust
pub struct RingBuffer<T: Copy> {
    buffer: Box<[CachePadded<T>]>,
    capacity: usize,
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
}
```

Properties:
- Single-Producer Single-Consumer pattern
- Power-of-2 capacity (2^20 entries = 1M messages)
- Cache-padded counters prevent false sharing
- Acquire/Release memory ordering for correctness

### 2. Arena Allocation
- **Pre-allocated memory**: 1GB HugePage reserved at startup
- **Linear allocation**: O(1) allocation, no per-object deallocation
- **Bulk reset**: Arena cleared between trading sessions
- **No fragmentation**: Fixed-size, bump-allocator model

### 3. Wallet Guard (Hard Interlock)
```rust
impl WalletGuard {
    pub fn validate(&self, order: &Order) -> Result<(), RiskRejection> {
        self.check_position_limit(order)?;    // O(1)
        self.check_order_size(order)?;        // O(1)
        self.check_drawdown()?;               // O(1)
        self.check_margin(order)?;            // O(1)
        Ok(())
    }
}
```

Risk checks:
- **Position limit**: Max position size per symbol
- **Order size**: Max order value
- **Daily drawdown**: Max loss per session
- **Margin adequacy**: Sufficient collateral

### 4. Thread Affinity
| Thread | Core | Purpose |
|--------|------|---------|
| Market Data | 0 | Ring buffer producer |
| Strategy | 1 | Signal generation |
| Risk | 2 | Wallet Guard |
| Order | 3 | Order dispatch |

Linux kernel parameters:
```bash
isolcpus=0-3 nohz_full=0-3 rcu_nocbs=0-3
```

## Consequences

### Positive
- **Zero GC pauses**: No allocations on hot path means no collection
- **Deterministic latency**: WCET bounds provable through analysis
- **High throughput**: Ring buffer handles millions of messages/second
- **Regulatory compliance**: Hard interlock satisfies SEC 15c3-5
- **Cache efficiency**: Cache-padded structures prevent false sharing

### Negative
- **Complexity**: Careful design required to avoid allocations
- **Memory overhead**: Pre-allocated arena wastes memory if underutilized
- **Platform dependencies**: HugePages require Linux; kernel tuning needed
- **Development difficulty**: No dynamic data structures on hot path

## Alternatives Considered

### Crossbeam Queue
| Aspect | Crossbeam | Ring Buffer |
|--------|-----------|-------------|
| Pattern | MPMC | SPSC |
| Allocation | Yes | No |
| Latency | Variable | Bounded |

**Rejected**: MPMC overhead unnecessary for single-producer pattern; allocation on push/pop violates Zero-GC.

### Heap Allocation with jemalloc
| Aspect | jemalloc | Arena |
|--------|----------|-------|
| Allocation | O(log n) | O(1) |
| Fragmentation | Possible | None |
| Determinism | Low | High |

**Rejected**: malloc/free non-deterministic; fragmentation causes variable latency.

### Software Trading (Non-deterministic)
**Rejected**: Unacceptable for HFT use case; violates latency requirements.

### Garbage-Collected Language
**Rejected**: GC pauses in milliseconds violate sub-millisecond latency requirements.

## WCET Analysis

| Operation | WCET Bound | Measurement |
|-----------|------------|-------------|
| Risk check (all 4) | <10μs | <100μs budget |
| Ring buffer push | <100ns | L1 cache hit |
| Ring buffer pop | <100ns | L1 cache hit |
| Arena alloc | <50ns | Atomic increment |

Proof sketch: Each operation involves O(1) memory accesses and arithmetic. With 1000 pessimistic operations at 10ns each: 10μs << 100μs budget.

## Related Standards
- **SEC Rule 15c3-5**: Risk Management Controls for Market Access
- **MiFID II Article 25**: Best Execution Requirements
- **IEEE 1016 Section 7.4**: Timing Specification
- **ISO/IEC 25010**: Performance Efficiency

## Related ADRs
- ADR-001: Rust Native Implementation
- ADR-006: Monoio Async Runtime

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
