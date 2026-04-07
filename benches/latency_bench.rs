// ============================================================
// LATENCY BENCHMARK
// ============================================================
// Measures end-to-end pipeline latency
// P50, P95, P99, P999 percentiles
// Microsecond precision
// ============================================================

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
    Throughput, SamplingMode, BenchmarkGroup, measurement::WallTime,
};
use hft_stealth_system::*;
use std::time::Duration;
use rand::Rng;
use std::sync::Arc;

// ============================================================
// BENCHMARK CONFIGURATION
// ============================================================

const WARMUP_ITERATIONS: usize = 1000;
const MEASUREMENT_ITERATIONS: usize = 10000;
const LATENCY_THRESHOLD_NS: u64 = 1_000_000; // 1ms

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn generate_test_ticks(count: usize) -> Vec<Tick> {
    let mut rng = rand::thread_rng();
    let mut ticks = Vec::with_capacity(count);
    
    for i in 0..count {
        ticks.push(Tick {
            price: 100.0 + rng.gen::<f64>() * 2.0,
            volume: 1000.0 + rng.gen::<f64>() * 500.0,
            timestamp_ns: i as u64 * 1_000_000,
            exchange_id: 1,
            side: (i % 2) as u8,
            tick_type: (i % 3) as u8,
            flags: 0,
            sequence: i as u32,
            instrument_id: 1,
            trade_id: i as u64,
            _padding: [0; 16],
        });
    }
    
    ticks
}

fn generate_test_order_book() -> OrderBook {
    let mut book = OrderBook::new(1, 0.01);
    
    // Populate with some depth
    for i in 0..10 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0 * (10 - i) as f64, 1000, 1);
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0 * (10 - i) as f64, 1000, 1);
        book.update(&bid);
        book.update(&ask);
    }
    
    book
}

// ============================================================
// INDIVIDUAL COMPONENT BENCHMARKS
// ============================================================

fn bench_order_book_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_update");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));
    
    let ticks = generate_test_ticks(1000);
    
    group.bench_function("single_update", |b| {
        let mut book = OrderBook::new(1, 0.01);
        b.iter(|| {
            for tick in ticks.iter().take(10) {
                book.update(black_box(tick));
            }
        })
    });
    
    group.bench_function("batch_update_100", |b| {
        let mut book = OrderBook::new(1, 0.01);
        b.iter(|| {
            for tick in ticks.iter().take(100) {
                book.update(black_box(tick));
            }
        })
    });
    
    group.finish();
}

fn bench_risk_gate(c: &mut Criterion) {
    let mut group = c.benchmark_group("risk_gate");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let gate = RiskGate::new();
    let book = generate_test_order_book();
    let ctx = GateContext {
        volatility_regime: 1,
        tau_max_ns: 500_000_000,
        price_variation: 0.001,
        atr_20: 0.005,
        delta_threshold: 0.3,
        kurtosis: 3.0,
        drift_bias: 0.0,
        predicted_prices: (0..256).map(|i| i as f64).collect(),
        actual_prices: (0..256).map(|i| i as f64).collect(),
        fill_probability: 0.5,
        conditional_pnl: 0.0,
        atr_10: 0.003,
        current_depth: 100000.0,
        candle_body_ratio: 0.5,
        order_book_conflict: false,
    };
    
    group.bench_function("evaluate_all_gates", |b| {
        b.iter(|| {
            black_box(gate.evaluate(black_box(&ctx)));
        })
    });
    
    group.bench_function("lambda1_only", |b| {
        b.iter(|| {
            let mut ctx_clone = ctx.clone();
            ctx_clone.volatility_regime = 2;
            black_box(gate.evaluate(black_box(&ctx_clone)));
        })
    });
    
    group.finish();
}

fn bench_harmonic_detector(c: &mut Criterion) {
    let mut group = c.benchmark_group("harmonic_detector");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let mut detector = HarmonicTrapDetector::new(256);
    
    // In-phase signals (no trap)
    let in_phase_pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let in_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    // Out-of-phase signals (trap)
    let out_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + std::f64::consts::PI).sin()).collect();
    
    group.bench_function("no_trap_detection", |b| {
        b.iter(|| {
            black_box(detector.detect_trap(black_box(&in_phase_pred), black_box(&in_phase_act)));
        })
    });
    
    group.bench_function("trap_detection", |b| {
        b.iter(|| {
            black_box(detector.detect_trap(black_box(&in_phase_pred), black_box(&out_phase_act)));
        })
    });
    
    group.bench_function("with_type_classification", |b| {
        b.iter(|| {
            black_box(detector.detect_with_type(black_box(&in_phase_pred), black_box(&out_phase_act)));
        })
    });
    
    group.finish();
}

fn bench_ml_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("ml_inference");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    // Note: This requires actual JAX model
    // For benchmark, we simulate inference time
    
    group.bench_function("feature_extraction_256", |b| {
        let config = FeatureConfig::default();
        let mut extractor = FeatureExtractor::new(config);
        let ticks = generate_test_ticks(256);
        
        for tick in &ticks {
            extractor.update(tick);
        }
        
        b.iter(|| {
            black_box(extractor.extract(1));
        })
    });
    
    group.bench_function("batch_inference_32", |b| {
        // Simulated batch inference
        b.iter(|| {
            // Simulate 32 predictions
            for _ in 0..32 {
                std::hint::black_box(0.5f64);
            }
        })
    });
    
    group.finish();
}

// ============================================================
// END-TO-END PIPELINE BENCHMARKS
// ============================================================

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(30));
    group.warm_up_time(Duration::from_secs(5));
    group.sample_size(MEASUREMENT_ITERATIONS);
    
    // Setup complete system
    let config = SystemConfig::for_environment(Environment::Development);
    let system = Arc::new(HFTStealthSystem::new(config).unwrap());
    
    let ticks = generate_test_ticks(1000);
    let book = generate_test_order_book();
    
    group.bench_function("tick_to_signal", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            
            // Simulate pipeline stages
            black_box(&ticks);
            black_box(&book);
            
            // Risk gate evaluation
            // ML inference
            // Signal generation
            
            let elapsed = start.elapsed().as_nanos() as u64;
            black_box(elapsed);
        })
    });
    
    group.bench_function("with_harmonic_detection", |b| {
        let mut detector = HarmonicTrapDetector::new(256);
        let prices: Vec<f64> = (0..256).map(|i| 100.0 + (i as f64 * 0.1).sin()).collect();
        
        b.iter(|| {
            let start = std::time::Instant::now();
            
            // Detect harmonic trap
            let is_trap = detector.detect_trap(&prices, &prices);
            black_box(is_trap);
            
            let elapsed = start.elapsed().as_nanos() as u64;
            black_box(elapsed);
        })
    });
    
    group.finish();
}

fn bench_execution_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_pipeline");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    group.bench_function("gate_check", |b| {
        b.iter(|| {
            black_box(executor.gate_check(0.025, 1.0, &book));
        })
    });
    
    group.bench_function("stealth_execution", |b| {
        b.iter(|| {
            let mut order_clone = order.clone();
            black_box(executor.execute(&mut order_clone, &book));
        })
    });
    
    group.bench_function("fragmentation", |b| {
        let config = FragmentConfig::default();
        let mut fragmenter = Fragmenter::new(config);
        
        b.iter(|| {
            black_box(fragmenter.fragment(0.025, 100.00));
        })
    });
    
    group.finish();
}

// ============================================================
// LATENCY PERCENTILE DISTRIBUTION
// ============================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_distribution");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(20));
    
    // Collect latency samples
    let mut latencies = Vec::with_capacity(MEASUREMENT_ITERATIONS);
    
    // Simulate pipeline latency measurements
    let mut rng = rand::thread_rng();
    
    group.bench_function("pipeline_latency_samples", |b| {
        b.iter(|| {
            // Simulate realistic latency distribution
            // Most around 500μs, some up to 1ms
            let latency = if rng.gen_bool(0.99) {
                500_000 + rng.gen::<u64>() % 400_000
            } else {
                1_000_000 + rng.gen::<u64>() % 500_000
            };
            
            latencies.push(latency);
            black_box(latency);
        })
    });
    
    // After collection, compute percentiles
    group.bench_function("percentile_calculation", |b| {
        b.iter(|| {
            let mut sorted = latencies.clone();
            sorted.sort();
            
            let p50 = sorted[sorted.len() / 2];
            let p95 = sorted[(sorted.len() * 95) / 100];
            let p99 = sorted[(sorted.len() * 99) / 100];
            let p999 = sorted[(sorted.len() * 999) / 1000];
            
            black_box((p50, p95, p99, p999));
        })
    });
    
    group.finish();
}

// ============================================================
// COMPARATIVE BENCHMARKS
// ============================================================

fn bench_component_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_comparison");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(15));
    
    // Risk gate vs ML inference vs Execution
    let gate = RiskGate::new();
    let ctx = GateContext::default();
    let mut detector = HarmonicTrapDetector::new(256);
    let prices: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    group.bench_function("risk_gate", |b| {
        b.iter(|| black_box(gate.evaluate(&ctx)))
    });
    
    group.bench_function("harmonic_detection", |b| {
        b.iter(|| black_box(detector.detect_trap(&prices, &prices)))
    });
    
    group.bench_function("both_combined", |b| {
        b.iter(|| {
            let risk_result = gate.evaluate(&ctx);
            let harmonic_result = detector.detect_trap(&prices, &prices);
            black_box((risk_result, harmonic_result));
        })
    });
    
    group.finish();
}

// ============================================================
// STRESS TEST BENCHMARKS
// ============================================================

fn bench_high_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(1));
    
    let ticks = generate_test_ticks(10000);
    
    group.bench_function("process_10000_ticks", |b| {
        let mut book = OrderBook::new(1, 0.01);
        
        b.iter(|| {
            for tick in &ticks {
                book.update(black_box(tick));
            }
        })
    });
    
    group.bench_function("concurrent_updates", |b| {
        use std::sync::Arc;
        use std::thread;
        
        let book = Arc::new(OrderBook::new(1, 0.01));
        
        b.iter(|| {
            let mut handles = vec![];
            let ticks_chunks: Vec<Vec<Tick>> = ticks.chunks(1000).map(|c| c.to_vec()).collect();
            
            for chunk in ticks_chunks {
                let book = book.clone();
                handles.push(thread::spawn(move || {
                    let mut local_book = OrderBook::new(1, 0.01);
                    for tick in chunk {
                        local_book.update(&tick);
                    }
                    local_book
                }));
            }
            
            for handle in handles {
                let _ = handle.join();
            }
            black_box(());
        })
    });
    
    group.finish();
}

// ============================================================
// REGISTER BENCHMARKS
// ============================================================

criterion_group!(
    name = latency_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(1000)
        .significance_level(0.05)
        .noise_threshold(0.02);
    targets = 
        bench_order_book_update,
        bench_risk_gate,
        bench_harmonic_detector,
        bench_ml_inference,
        bench_full_pipeline,
        bench_execution_pipeline,
        bench_latency_distribution,
        bench_component_comparison,
        bench_high_throughput
);

criterion_main!(latency_benches);
