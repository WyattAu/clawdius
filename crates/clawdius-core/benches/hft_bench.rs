//! HFT Performance Benchmarks
//!
//! Critical path benchmarks for HFT mode:
//! - Ring buffer: <100ns
//! - Wallet Guard: <100µs
//! - HFT Pipeline: <1ms
//! - Boot: <20ms

use clawdius_core::broker::{
    ring_buffer::RingBuffer,
    signal::{MarketData, Signal, SignalDirection, SignalEngine},
    strategy::Strategy,
    wallet_guard::{Order, OrderSide, WalletGuard},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_decimal::Decimal;
use std::time::Instant;

fn bench_ring_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer");
    group.sample_size(10_000);

    #[derive(Clone, Copy, Debug)]
    struct MarketDataMsg {
        price: u64,
        volume: u64,
        timestamp: u64,
    }

    group.bench_function("push_single", |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| black_box(buffer.push(black_box(msg))));
    });

    group.bench_function("pop_single", |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        buffer.push(msg).unwrap();
        b.iter(|| black_box(buffer.pop()));
    });

    group.bench_function("push_pop_roundtrip", |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| {
            black_box(buffer.push(black_box(msg)).unwrap());
            black_box(buffer.pop())
        });
    });

    group.bench_function("burst_1000", |b| {
        let buffer: RingBuffer<MarketDataMsg, 2048> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| {
            for i in 0..1000 {
                let _ = black_box(buffer.push(MarketDataMsg {
                    timestamp: i,
                    ..msg
                }));
            }
            let mut count = 0;
            while buffer.pop().is_some() {
                count += 1;
            }
            black_box(count)
        });
    });

    group.finish();
}

fn bench_wallet_guard(c: &mut Criterion) {
    let mut group = c.benchmark_group("wallet_guard");
    group.sample_size(10_000);

    let guard = WalletGuard::default();

    let small_order = Order {
        symbol: "AAPL".to_string(),
        quantity: Decimal::from(100),
        price: Decimal::from(150),
        side: OrderSide::Buy,
    };

    group.bench_function("validate_order", |b| {
        b.iter(|| black_box(guard.check_order(black_box(&small_order))));
    });

    group.bench_function("order_value_calc", |b| {
        b.iter(|| black_box(small_order.value()));
    });

    group.bench_function("validate_market_access", |b| {
        b.iter(|| black_box(guard.validate_market_access()));
    });

    group.bench_function("check_daily_volume", |b| {
        let volume = Decimal::from(1_000_000);
        b.iter(|| black_box(guard.check_daily_volume(black_box(volume))));
    });

    let mut guard_with_restrictions = WalletGuard::default();
    guard_with_restrictions.restrict_symbol("PENN");
    guard_with_restrictions.restrict_symbol("GME");

    group.bench_function("validate_with_restrictions", |b| {
        b.iter(|| black_box(guard_with_restrictions.check_order(&small_order)));
    });

    group.finish();
}

fn bench_hft_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("hft_pipeline");
    group.sample_size(1000);

    struct BenchStrategy;

    impl Strategy for BenchStrategy {
        fn evaluate(&self, data: &MarketData) -> Option<Signal> {
            if data.price > Decimal::from(100) {
                Some(Signal {
                    symbol: data.symbol.clone(),
                    direction: SignalDirection::Buy,
                    confidence: 0.8,
                    strategy: "BenchStrategy".to_string(),
                    timestamp: data.timestamp,
                })
            } else {
                None
            }
        }

        fn name(&self) -> &str {
            "BenchStrategy"
        }
    }

    let mut engine = SignalEngine::new();
    engine.register_strategy(Box::new(BenchStrategy));

    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        price: Decimal::from(150),
        volume: Decimal::from(1000),
        timestamp: 0,
    };

    group.bench_function("signal_generation", |b| {
        b.iter(|| black_box(engine.process(black_box(&market_data))));
    });

    group.bench_function("drain_signals", |b| {
        for _ in 0..100 {
            let _ = engine.process(&market_data);
        }
        b.iter(|| black_box(engine.drain_signals()));
    });

    let guard = WalletGuard::default();
    let order = Order {
        symbol: "AAPL".to_string(),
        quantity: Decimal::from(100),
        price: Decimal::from(150),
        side: OrderSide::Buy,
    };

    group.bench_function("full_pipeline_signal_to_risk", |b| {
        b.iter(|| {
            let signals = black_box(engine.process(&market_data));
            if !signals.is_empty() {
                let _ = black_box(guard.check_order(&order));
            }
        });
    });

    group.finish();
}

fn bench_boot_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("boot_simulation");
    group.sample_size(100);

    group.bench_function("ring_buffer_init", |b| {
        b.iter(|| {
            let buffer: RingBuffer<u64, 65536> = RingBuffer::new();
            black_box(buffer)
        });
    });

    group.bench_function("wallet_guard_init", |b| {
        b.iter(|| {
            let guard = WalletGuard::new(
                Decimal::from(1_000_000),
                Decimal::from(10_000_000),
                Decimal::from(100_000),
            );
            black_box(guard)
        });
    });

    group.bench_function("signal_engine_init", |b| {
        b.iter(|| {
            let engine = SignalEngine::new();
            black_box(engine)
        });
    });

    group.bench_function("full_hft_stack_init", |b| {
        b.iter(|| {
            let buffer: RingBuffer<u64, 65536> = RingBuffer::new();
            let guard = WalletGuard::default();
            let engine = SignalEngine::new();
            black_box((buffer, guard, engine))
        });
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1));

    #[derive(Clone, Copy)]
    struct Msg {
        data: [u8; 64],
    }

    let buffer: RingBuffer<Msg, 1_048_576> = RingBuffer::new();
    let msg = Msg { data: [0u8; 64] };

    group.bench_function("ring_buffer_msg_rate", |b| {
        let mut count = 0u64;
        b.iter(|| {
            black_box(buffer.push(msg).is_ok());
            count += 1;
            if count % 1000 == 0 {
                while buffer.pop().is_some() {}
            }
        });
    });

    group.finish();
}

#[allow(missing_docs)]
criterion_group!(
    benches,
    bench_ring_buffer,
    bench_wallet_guard,
    bench_hft_pipeline,
    bench_boot_simulation,
    bench_throughput,
);
criterion_main!(benches);
