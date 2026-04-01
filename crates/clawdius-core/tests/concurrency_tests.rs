//! Multi-threaded stress tests for lock-free ring buffer and concurrent wallet guard.
//!
//! Run with: cargo test --features broker-mode --test concurrency_tests

use std::sync::Arc;
use std::thread;

use clawdius_core::broker::{Order, OrderSide, RingBuffer, RiskDecision, Wallet, WalletGuard};

const BUF_N: usize = 1 << 14;

fn spawn<F, T>(name: &str, f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name(name.into())
        .stack_size(8 * 1024 * 1024)
        .spawn(f)
        .expect("failed to spawn thread")
}

// ---------------------------------------------------------------------------
// Test 1: Multi-threaded ring buffer SPSC invariant
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_spsc_stress() {
    const NUM_MESSAGES: u64 = 1_000_000;
    let buffer = Arc::new(RingBuffer::<u64, BUF_N>::new());
    let buf_prod = buffer.clone();
    let buf_cons = buffer.clone();

    let producer = spawn("producer", move || {
        for i in 0..NUM_MESSAGES {
            while buf_prod.push(i).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let consumer = spawn("consumer", move || {
        let mut received = 0u64;
        let mut last_id = 0u64;
        while received < NUM_MESSAGES {
            match buf_cons.pop() {
                Some(val) => {
                    assert_eq!(val, last_id, "FIFO violation at message {}", received);
                    last_id += 1;
                    received += 1;
                },
                None => std::hint::spin_loop(),
            }
        }
        received
    });

    producer.join().expect("producer panicked");
    let count = consumer.join().expect("consumer panicked");
    assert_eq!(count, NUM_MESSAGES);
}

// ---------------------------------------------------------------------------
// Test 2: Concurrent wallet guard (no data race)
// ---------------------------------------------------------------------------

#[test]
fn wallet_guard_concurrent_checks() {
    let guard = Arc::new(WalletGuard::with_defaults());
    let wallet = Wallet::new(1_000_000_000);

    let handles: Vec<_> = (0..100)
        .map(|i| {
            let g = guard.clone();
            let w = wallet.clone();
            spawn("guard", move || {
                let order = Order::new(i as u32, OrderSide::Buy, 10, 100);
                g.check(&w, &order)
            })
        })
        .collect();

    for h in handles {
        let result = h.join().expect("thread panicked");
        assert!(matches!(result, RiskDecision::Approve));
    }
}

#[test]
fn wallet_guard_concurrent_mixed_sides() {
    let guard = Arc::new(WalletGuard::with_defaults());
    let wallet = Wallet::new(1_000_000_000);

    let handles: Vec<_> = (0..200)
        .map(|i| {
            let g = guard.clone();
            let w = wallet.clone();
            spawn("guard-mixed", move || {
                let side = if i % 2 == 0 {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };
                let order = Order::new((i % 10) as u32, side, 5 + (i % 50), 50 + (i % 100));
                g.check(&w, &order)
            })
        })
        .collect();

    for h in handles {
        let _ = h.join().expect("thread panicked");
    }
}

// ---------------------------------------------------------------------------
// Test 3: Ring buffer capacity stress
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_capacity_stress() {
    let buffer = RingBuffer::<u64, 1024>::new();
    let cap = buffer.capacity();

    for round in 0..1_000u64 {
        for i in 0..cap {
            let val = round * cap as u64 + i as u64;
            assert!(
                buffer.push(val).is_ok(),
                "round {round} push {i} should succeed"
            );
        }
        assert!(buffer.push(0).is_err(), "round {round} should be full");

        for i in 0..cap {
            let expected = round * cap as u64 + i as u64;
            assert_eq!(
                buffer.pop(),
                Some(expected),
                "round {round} pop {i} mismatch"
            );
        }
        assert!(buffer.pop().is_none(), "round {round} should be empty");
    }
}

// ---------------------------------------------------------------------------
// Test 4: Ring buffer wraparound stress
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_wraparound_stress() {
    let buffer = RingBuffer::<u64, 64>::new();
    let cap = buffer.capacity();
    let half = cap / 2;

    for round in 0..1_000u64 {
        let base = round * cap as u64 * 2;
        let cap = cap as u64;
        let half = half as u64;

        for i in 0..cap {
            assert!(buffer.push(base + i).is_ok());
        }

        for i in 0..half {
            assert_eq!(buffer.pop(), Some(base + i));
        }

        for i in 0..half {
            let val = base + cap + i;
            assert!(buffer.push(val).is_ok());
        }

        for i in 0..(cap - half) {
            assert_eq!(buffer.pop(), Some(base + half + i));
        }
        for i in 0..half {
            assert_eq!(buffer.pop(), Some(base + cap + i));
        }

        assert!(
            buffer.pop().is_none(),
            "round {round} should be empty after wraparound"
        );
    }
}
