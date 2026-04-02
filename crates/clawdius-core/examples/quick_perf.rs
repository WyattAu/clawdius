//! Fast micro-benchmarks using actual library types.
//! Run: cargo run --release --package clawdius-core --example quick_perf

use std::hint::black_box;
use std::time::Instant;

use clawdius_core::broker::ring_buffer::RingBuffer;
use clawdius_core::broker::wallet_guard::{Order, OrderSide, Wallet, WalletGuard};

const ITERS: u64 = 1_000_000;

fn bench_ring_buffer_push() {
    let buf: RingBuffer<u64, 1024> = RingBuffer::new();
    let start = Instant::now();
    for i in 0..ITERS {
        while buf.push(i).is_err() {
            let _ = buf.pop();
        }
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as u64 / ITERS;
    let pass = ns_per < 100;
    println!(
        "ring_buffer/push              {:>8} ns  (target: <100 ns) {}",
        ns_per,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
}

fn bench_ring_buffer_pop() {
    let buf: RingBuffer<u64, 1024> = RingBuffer::new();
    for i in 0..1024u64 {
        let _ = buf.push(i);
    }
    let start = Instant::now();
    for _ in 0..ITERS {
        if let Some(v) = buf.pop() {
            let _ = buf.push(v);
        }
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as u64 / ITERS;
    let pass = ns_per < 100;
    println!(
        "ring_buffer/pop               {:>8} ns  (target: <100 ns) {}",
        ns_per,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
}

fn bench_ring_buffer_roundtrip() {
    let buf: RingBuffer<u64, 1024> = RingBuffer::new();
    let start = Instant::now();
    for i in 0..ITERS {
        let _ = buf.push(i);
        black_box(buf.pop());
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as u64 / ITERS;
    let pass = ns_per < 200;
    println!(
        "ring_buffer/roundtrip         {:>8} ns  (target: <200 ns) {}",
        ns_per,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
}

fn bench_wallet_guard_check() {
    let guard = WalletGuard::with_defaults();
    let mut wallet = Wallet::new(1_000_000_000);
    wallet.update_position(1, 1000);
    let order = Order::new(1, OrderSide::Buy, 100, 100_000);
    let start = Instant::now();
    for _ in 0..ITERS {
        black_box(guard.check(&wallet, &order));
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as u64 / ITERS;
    let pass = ns_per < 100_000;
    println!(
        "wallet_guard/check            {:>8} ns  (target: <100µs)  {}",
        ns_per,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
}

fn bench_wallet_guard_reject() {
    let guard = WalletGuard::with_defaults();
    let wallet = Wallet::new(0); // Zero cash
    let order = Order::new(1, OrderSide::Buy, 100, 100_000);
    let start = Instant::now();
    for _ in 0..ITERS {
        black_box(guard.check(&wallet, &order));
    }
    let elapsed = start.elapsed();
    let ns_per = elapsed.as_nanos() as u64 / ITERS;
    let pass = ns_per < 100_000;
    println!(
        "wallet_guard/reject           {:>8} ns  (target: <100µs)  {}",
        ns_per,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
}

fn bench_init() {
    let start = Instant::now();
    for _ in 0..10_000 {
        black_box(RingBuffer::<u64, 4096>::new());
    }
    let ns = start.elapsed().as_nanos() as u64 / 10_000;
    println!("init/ring_buffer_4096        {:>8} ns", ns);

    let start = Instant::now();
    for _ in 0..10_000 {
        black_box(WalletGuard::with_defaults());
    }
    let ns = start.elapsed().as_nanos() as u64 / 10_000;
    println!("init/wallet_guard            {:>8} ns", ns);
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  CLAWDIUS HFT PERFORMANCE (actual library types, release)   ║");
    println!(
        "║  {} iterations per benchmark                       ║",
        ITERS
    );
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ RING BUFFER (Lock-free SPSC, cache-padded atomics)         │");
    println!("├─────────────────────────────────────────────────────────────┤");
    bench_init();
    bench_ring_buffer_push();
    bench_ring_buffer_pop();
    bench_ring_buffer_roundtrip();
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ WALLET GUARD (SEC 15c3-5 risk checks)                      │");
    println!("├─────────────────────────────────────────────────────────────┤");
    bench_wallet_guard_check();
    bench_wallet_guard_reject();
    println!("└─────────────────────────────────────────────────────────────┘");
}
