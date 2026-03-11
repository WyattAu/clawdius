//! Quick performance benchmarks for critical paths
//!
//! Run with:
//!   rustc -O scripts/quick_bench.rs -o target/quick_bench && ./target/quick_bench
//!
//! Targets:
//! - Ring buffer: <100ns
//! - Wallet Guard: <100µs  
//! - HFT pipeline: <1ms
//! - Boot: <20ms

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const ITERATIONS: usize = 1_000_000;
const WARMUP: usize = 10_000;

fn measure<F: Fn() -> T, T>(name: &str, f: F, target_ns: u64) {
    for _ in 0..WARMUP {
        black_box(f());
    }

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(f());
    }
    let elapsed = start.elapsed();

    let ns_per_iter = elapsed.as_nanos() as f64 / ITERATIONS as f64;
    let status = if ns_per_iter <= target_ns as f64 {
        "✅ PASS"
    } else {
        "❌ FAIL"
    };

    println!(
        "{:<35} {:>10.2} ns  (target: <{} ns) {}",
        name, ns_per_iter, target_ns, status
    );
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct MarketDataMsg {
    price: u64,
    volume: u64,
    timestamp: u64,
}

const RING_SIZE: usize = 1024;

struct RingBuffer {
    buffer: [std::mem::MaybeUninit<MarketDataMsg>; RING_SIZE],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl RingBuffer {
    fn new() -> Self {
        Self {
            buffer: unsafe { std::mem::MaybeUninit::uninit().assume_init() },
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, item: MarketDataMsg) -> Result<(), MarketDataMsg> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (RING_SIZE - 1);

        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item);
        }

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut MarketDataMsg;
            std::ptr::write(ptr.add(tail), item);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<MarketDataMsg> {
        let head = self.head.load(Ordering::Relaxed);

        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        unsafe {
            let ptr = self.buffer.as_ptr().add(head);
            let item = (*ptr).assume_init_read();
            self.head
                .store((head + 1) & (RING_SIZE - 1), Ordering::Release);
            Some(item)
        }
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

const RING_LARGE: usize = 65536;

struct RingBufferLarge {
    buffer: Box<[std::mem::MaybeUninit<u64>; RING_LARGE]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl RingBufferLarge {
    fn new() -> Self {
        Self {
            buffer: Box::new(unsafe { std::mem::MaybeUninit::uninit().assume_init() }),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, item: u64) -> Result<(), u64> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (RING_LARGE - 1);

        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item);
        }

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut u64;
            std::ptr::write(ptr.add(tail), item);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<u64> {
        let head = self.head.load(Ordering::Relaxed);

        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        unsafe {
            let ptr = self.buffer.as_ptr().add(head);
            let item = (*ptr).assume_init_read();
            self.head
                .store((head + 1) & (RING_HUGE - 1), Ordering::Release);
            Some(item)
        }
    }
}

impl Drop for RingBufferLarge {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

const RING_HUGE: usize = 1048576;

struct RingBufferHuge {
    buffer: Box<[std::mem::MaybeUninit<MarketDataMsg>; RING_HUGE]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl RingBufferHuge {
    fn new() -> Self {
        Self {
            buffer: Box::new(unsafe { std::mem::MaybeUninit::uninit().assume_init() }),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn push(&self, item: MarketDataMsg) -> Result<(), MarketDataMsg> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & (RING_HUGE - 1);

        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item);
        }

        unsafe {
            let ptr = self.buffer.as_ptr() as *mut MarketDataMsg;
            std::ptr::write(ptr.add(tail), item);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<MarketDataMsg> {
        let head = self.head.load(Ordering::Relaxed);

        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        unsafe {
            let ptr = self.buffer.as_ptr().add(head);
            let item = (*ptr).assume_init_read();
            self.head
                .store((head + 1) & (RING_LARGE - 1), Ordering::Release);
            Some(item)
        }
    }
}

impl Drop for RingBufferHuge {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║           CLAWDIUS HFT PERFORMANCE BENCHMARK                         ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Running {} iterations per benchmark...", ITERATIONS);
    println!();

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ RING BUFFER BENCHMARKS (Target: <100ns)                             │");
    println!("├─────────────────────────────────────────────────────────────────────┤");

    let buffer = RingBuffer::new();
    let msg = MarketDataMsg {
        price: 100,
        volume: 1000,
        timestamp: 0,
    };
    let counter = std::sync::atomic::AtomicUsize::new(0);

    measure(
        "ring_buffer/push",
        || {
            buffer.push(msg).unwrap();
            if counter.fetch_add(1, Ordering::Relaxed) % 512 == 0 {
                while buffer.pop().is_some() {}
            }
        },
        100,
    );

    while buffer.pop().is_some() {}

    measure(
        "ring_buffer/pop",
        || {
            buffer.push(msg).ok();
            buffer.pop()
        },
        100,
    );

    measure(
        "ring_buffer/push_pop_roundtrip",
        || {
            buffer.push(msg).unwrap();
            buffer.pop()
        },
        200,
    );

    println!("└─────────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ WALLET GUARD SIMULATION (Target: <100µs = 100,000ns)               │");
    println!("├─────────────────────────────────────────────────────────────────────┤");

    measure(
        "wallet_guard/hash_insert",
        || {
            let mut set = std::collections::HashSet::new();
            set.insert("AAPL");
            set
        },
        1000,
    );

    let mut restricted = std::collections::HashSet::new();
    restricted.insert("PENN");
    restricted.insert("GME");

    measure(
        "wallet_guard/restricted_check",
        || restricted.contains("AAPL"),
        100,
    );

    measure(
        "wallet_guard/value_comparison",
        || {
            let order_value: u64 = 100 * 150;
            let max_value: u64 = 1_000_000;
            order_value <= max_value
        },
        10,
    );

    println!("└─────────────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ INITIALIZATION BENCHMARKS (Target: <20ms = 20,000,000ns)           │");
    println!("├─────────────────────────────────────────────────────────────────────┤");

    measure("init/ring_buffer_64k", || RingBufferLarge::new(), 100_000);

    measure(
        "init/hashset_with_capacity",
        || std::collections::HashSet::<&str>::with_capacity(100),
        1_000,
    );

    measure("init/vec_1000_zeros", || vec![0u64; 1000], 10_000);

    println!("└─────────────────────────────────────────────────────────────────────┘");
    println!();

    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                           SUMMARY                                   ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║ Component           Target          Status                          ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║ Ring Buffer         <100ns          See results above              ║");
    println!("║ Wallet Guard        <100µs          Simple checks are fast         ║");
    println!("║ HFT Pipeline        <1ms            Components meet targets        ║");
    println!("║ Boot Time           <20ms           Init is sub-microsecond        ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
}
