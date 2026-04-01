//! WCET (Worst-Case Execution Time) Measurement Benchmarks
//!
//! Strict SLO compliance harnesses for the Clawdius HFT pipeline.
//! Per YP-HFT-BROKER-001:
//!   - Ring buffer ops:      < 100ns  per operation
//!   - Wallet guard check:   < 100us  per check
//!   - Signal-to-risk path:  < 1ms    end-to-end
//!   - Dispatch notify:      < 100us  per notification

use clawdius_core::broker::{
    signal::{MarketData, Signal, SignalDirection, SignalEngine},
    strategy::Strategy,
    wallet_guard::{Order, OrderSide, RiskDecision, RiskParams, Wallet, WalletGuard},
    RingBuffer,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_decimal::Decimal;
use std::time::Duration;

const RING_BUFFER_OP_TARGET_NS: u64 = 100;
const WALLET_GUARD_CHECK_TARGET_US: u64 = 100;
const SIGNAL_TO_RISK_TARGET_US: u64 = 1000;
const DISPATCH_TARGET_US: u64 = 100;

#[derive(Clone, Copy, Debug)]
struct MarketDataMsg {
    price: u64,
    volume: u64,
    timestamp: u64,
}

fn bench_ring_buffer_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/ring_buffer_write");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);

    // NOTE: Const generics prevent runtime capacity selection. We benchmark two
    // representative sizes. Capacity 1_048_576 is omitted — 24 MB on stack causes
    // overflow in release benchmarks. Heap allocation would change the performance
    // characteristics being measured.
    group.bench_function(BenchmarkId::new("capacity", 1024), |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| black_box(buffer.push(black_box(msg))));
    });

    group.bench_function(BenchmarkId::new("capacity", 65536), |b| {
        let buffer: RingBuffer<MarketDataMsg, 65536> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| black_box(buffer.push(black_box(msg))));
    });

    group.finish();
}

fn bench_ring_buffer_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/ring_buffer_read");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);

    group.bench_function(BenchmarkId::new("capacity", 1024), |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        buffer.push(msg).unwrap();
        b.iter(|| black_box(buffer.pop()));
    });

    group.bench_function(BenchmarkId::new("capacity", 65536), |b| {
        let buffer: RingBuffer<MarketDataMsg, 65536> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        buffer.push(msg).unwrap();
        b.iter(|| black_box(buffer.pop()));
    });

    group.finish();
}

fn bench_ring_buffer_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/ring_buffer_roundtrip");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);

    group.bench_function("push_pop", |b| {
        let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
        let msg = MarketDataMsg {
            price: 100,
            volume: 1000,
            timestamp: 0,
        };
        b.iter(|| {
            buffer.push(black_box(msg)).unwrap();
            black_box(buffer.pop())
        });
    });

    group.finish();
}

fn bench_wallet_guard_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/wallet_guard");
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);

    let guard = WalletGuard::with_defaults();
    let wallet = Wallet::new(1_000_000_000);
    let order = Order::new(1, OrderSide::Buy, 100, 150);

    group.bench_function("check_order_small", |b| {
        b.iter(|| black_box(guard.check(black_box(&wallet), black_box(&order))));
    });

    let large_order = Order::new(1, OrderSide::Buy, 100_000, 150);

    group.bench_function("check_order_large", |b| {
        b.iter(|| black_box(guard.check(black_box(&wallet), black_box(&large_order))));
    });

    let guard_custom = WalletGuard::new(RiskParams {
        pi_max: 1_000_000,
        sigma_max: 500_000,
        lambda_max: 100_000_000,
        margin_ratio: 2,
    });

    group.bench_function("check_order_with_custom_params", |b| {
        b.iter(|| black_box(guard_custom.check(black_box(&wallet), black_box(&order))));
    });

    let sell_order = Order::new(1, OrderSide::Sell, 100, 150);

    group.bench_function("check_order_sell", |b| {
        b.iter(|| black_box(guard.check(black_box(&wallet), black_box(&sell_order))));
    });

    group.finish();
}

fn bench_signal_to_risk_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/signal_to_risk_pipeline");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);
    group.throughput(Throughput::Elements(1));

    struct WcetStrategy;

    impl Strategy for WcetStrategy {
        fn evaluate(&self, data: &MarketData) -> Option<Signal> {
            if data.price > Decimal::from(100) {
                Some(Signal {
                    symbol: data.symbol.clone(),
                    direction: SignalDirection::Buy,
                    confidence: 0.8,
                    strategy: "WcetStrategy".to_string(),
                    timestamp: data.timestamp,
                })
            } else {
                None
            }
        }

        fn name(&self) -> &'static str {
            "WcetStrategy"
        }
    }

    let mut engine = SignalEngine::new();
    engine.register_strategy(Box::new(WcetStrategy));

    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        price: Decimal::from(150),
        volume: Decimal::from(1000),
        timestamp: 0,
    };

    let guard = WalletGuard::with_defaults();
    let wallet = Wallet::new(1_000_000_000);
    let order = Order::new(1, OrderSide::Buy, 100, 150);

    group.bench_function("signal_generation_only", |b| {
        b.iter(|| black_box(engine.process(black_box(&market_data))));
    });

    group.bench_function("risk_check_only", |b| {
        b.iter(|| black_box(guard.check(black_box(&wallet), black_box(&order))));
    });

    group.bench_function("full_pipeline_signal_to_risk", |b| {
        b.iter(|| {
            let signals = black_box(engine.process(&market_data));
            if !signals.is_empty() {
                let _ = black_box(guard.check(&wallet, &order));
            }
        });
    });

    let buffer: RingBuffer<MarketDataMsg, 1024> = RingBuffer::new();
    let msg = MarketDataMsg {
        price: 150,
        volume: 1000,
        timestamp: 0,
    };

    group.bench_function("full_pipeline_with_buffer", |b| {
        b.iter(|| {
            let _ = black_box(buffer.push(msg));
            let _ = black_box(buffer.pop());
            let signals = black_box(engine.process(&market_data));
            if !signals.is_empty() {
                let _ = black_box(guard.check(&wallet, &order));
            }
        });
    });

    group.finish();
}

fn bench_ring_buffer_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/ring_buffer_contention");
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1_000);

    let buffer: RingBuffer<MarketDataMsg, 65536> = RingBuffer::new();
    let msg = MarketDataMsg {
        price: 100,
        volume: 1000,
        timestamp: 0,
    };

    group.bench_function("burst_100", |b| {
        b.iter(|| {
            for i in 0..100u64 {
                let _ = black_box(buffer.push(MarketDataMsg {
                    timestamp: i,
                    ..msg
                }));
            }
            let mut count = 0u64;
            while buffer.pop().is_some() {
                count += 1;
            }
            black_box(count);
        });
    });

    group.bench_function("near_full_write", |b| {
        b.iter(|| {
            for _ in 0..65534 {
                let _ = buffer.push(msg);
            }
            let result = black_box(buffer.push(msg));
            while buffer.pop().is_some() {}
            black_box(result);
        });
    });

    group.finish();
}

fn wcet_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("wcet/summary");

    println!("\n=== WCET SLO Summary (YP-HFT-BROKER-001) ===");
    println!(
        "  Ring buffer op target:     < {:>4} ns",
        RING_BUFFER_OP_TARGET_NS
    );
    println!(
        "  Wallet guard check target: < {:>4} us",
        WALLET_GUARD_CHECK_TARGET_US
    );
    println!(
        "  Signal-to-risk target:     < {:>4} us",
        SIGNAL_TO_RISK_TARGET_US
    );
    println!(
        "  Dispatch target:           < {:>4} us",
        DISPATCH_TARGET_US
    );
    println!("=========================================\n");
    println!("Run with: cargo bench --bench wcet_bench -- --save-baseline wcet");
    println!("Compare with: cargo bench --bench wcet_bench -- --baseline wcet");

    group.bench_function("slo_targets_documented", |b| {
        b.iter(|| {
            black_box(RING_BUFFER_OP_TARGET_NS);
            black_box(WALLET_GUARD_CHECK_TARGET_US);
            black_box(SIGNAL_TO_RISK_TARGET_US);
            black_box(DISPATCH_TARGET_US);
        });
    });

    group.finish();
}

criterion_group! {
    name = wcet;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(5))
        .sample_size(1_000)
        .significance_level(0.01);
    targets =
        bench_ring_buffer_write,
        bench_ring_buffer_read,
        bench_ring_buffer_roundtrip,
        bench_ring_buffer_contention,
        bench_wallet_guard_check,
        bench_signal_to_risk_pipeline,
        wcet_summary
}

criterion_main!(wcet);
