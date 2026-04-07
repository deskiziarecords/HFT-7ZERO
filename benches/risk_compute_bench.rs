// ============================================================
// RISK COMPUTATION BENCHMARK
// ============================================================
// Measures risk calculation performance
// VaR computation, stress testing
// Real-time risk metric updates
// ============================================================

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
    Throughput, SamplingMode,
};
use hft_stealth_system::*;
use rand::Rng;
use std::time::Duration;

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn generate_price_series(n: usize, volatility: f64) -> Vec<f64> {
    let mut rng = rand::thread_rng();
    let mut prices = Vec::with_capacity(n);
    let mut price = 100.0;
    
    for _ in 0..n {
        price *= 1.0 + rng.gen::<f64>() * volatility - volatility / 2.0;
        prices.push(price);
    }
    
    prices
}

fn generate_positions(n: usize) -> dashmap::DashMap<u32, Position> {
    let positions = dashmap::DashMap::new();
    let mut rng = rand::thread_rng();
    
    for i in 0..n {
        positions.insert(i as u32, Position {
            instrument_id: i as u32,
            quantity: rng.gen::<f64>() * 100.0,
            avg_price: 100.0 + rng.gen::<f64>() * 10.0,
            current_pnl: rng.gen::<f64>() * 1000.0,
        });
    }
    
    positions
}

// ============================================================
// VAR COMPUTATION BENCHMARKS
// ============================================================

fn bench_historical_var(c: &mut Criterion) {
    let mut group = c.benchmark_group("historical_var");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let window_sizes = [252, 500, 1000, 5000];
    
    for &window in &window_sizes {
        let returns: Vec<f64> = (0..window)
            .map(|_| rand::thread_rng().gen::<f64>() * 0.02 - 0.01)
            .collect();
        
        let mut var = HistoricalVaR::new(0.99, 1);
        var.update(&returns);
        
        let positions = generate_positions(10);
        let book = generate_test_order_book();
        
        group.throughput(Throughput::Elements(window as u64));
        group.bench_with_input(BenchmarkId::new("compute_var", window), &window, |b, _| {
            b.iter(|| {
                black_box(var.calculate(&positions, &book, 0.99));
            })
        });
        
        group.bench_with_input(BenchmarkId::new("expected_shortfall", window), &window, |b, _| {
            b.iter(|| {
                black_box(var.expected_shortfall(&positions, &book, 0.99));
            })
        });
    }
    
    group.finish();
}

fn bench_parametric_var(c: &mut Criterion) {
    let mut group = c.benchmark_group("parametric_var");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let window_sizes = [252, 500, 1000];
    
    for &window in &window_sizes {
        let returns: Vec<f64> = (0..window)
            .map(|_| rand::thread_rng().gen::<f64>() * 0.02 - 0.01)
            .collect();
        
        let mut var = ParametricVaR::new(0.99, 1);
        var.update(&returns);
        
        let positions = generate_positions(10);
        let book = generate_test_order_book();
        
        group.bench_with_input(BenchmarkId::new("compute_var", window), &window, |b, _| {
            b.iter(|| {
                black_box(var.calculate(&positions, &book, 0.99));
            })
        });
    }
    
    group.finish();
}

fn bench_monte_carlo_var(c: &mut Criterion) {
    let mut group = c.benchmark_group("monte_carlo_var");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(15));
    
    let simulation_counts = [1000, 10000, 100000];
    
    for &sims in &simulation_counts {
        let var = MonteCarloVaR::new(0.99, 1, sims);
        let positions = generate_positions(10);
        let book = generate_test_order_book();
        
        group.throughput(Throughput::Elements(sims as u64));
        group.bench_with_input(BenchmarkId::new("compute_var", sims), &sims, |b, _| {
            b.iter(|| {
                black_box(var.calculate(&positions, &book, 0.99));
            })
        });
    }
    
    group.finish();
}

// ============================================================
// RISK GATE BENCHMARKS
// ============================================================

fn bench_risk_gate_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("risk_gate_computation");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let gate = RiskGate::new();
    let ctx = GateContext::default();
    
    // Test each lambda individually
    let lambda_tests = [
        ("lambda1", {
            let mut ctx = ctx.clone();
            ctx.volatility_regime = 2;
            ctx
        }),
        ("lambda2", {
            let mut ctx = ctx.clone();
            ctx.kurtosis = 1.05;
            ctx.drift_bias = 0.1;
            ctx
        }),
        ("lambda3", {
            let mut ctx = ctx.clone();
            ctx.predicted_prices = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
            ctx.actual_prices = (0..256).map(|i| (i as f64 * 0.1 + std::f64::consts::PI).sin()).collect();
            ctx
        }),
        ("lambda4", {
            let mut ctx = ctx.clone();
            ctx.fill_probability = 0.8;
            ctx.conditional_pnl = -0.01;
            ctx
        }),
        ("lambda5", {
            let mut ctx = ctx.clone();
            ctx.current_depth = 1000.0;
            ctx
        }),
        ("lambda6", {
            let mut ctx = ctx.clone();
            ctx.candle_body_ratio = 0.8;
            ctx.order_book_conflict = true;
            ctx
        }),
    ];
    
    for (name, test_ctx) in lambda_tests {
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(gate.evaluate(black_box(&test_ctx)));
            })
        });
    }
    
    group.bench_function("all_lambdas", |b| {
        b.iter(|| {
            black_box(gate.evaluate(black_box(&ctx)));
        })
    });
    
    group.finish();
}

// ============================================================
// STRESS TEST BENCHMARKS
// ============================================================

fn bench_stress_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_testing");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let mut stress_tester = StressTester::new();
    let positions = generate_positions(10);
    let book = generate_test_order_book();
    
    let scenario_counts = [1, 5, 10, 20];
    
    for &count in &scenario_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("run_scenarios", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    black_box(stress_tester.run_all_scenarios(&book, &positions));
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// POSITION & PNL BENCHMARKS
// ============================================================

fn bench_pnl_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pnl_calculation");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let mut pnl_calc = PnLCalculator::new();
    let positions = generate_positions(10);
    let book = generate_test_order_book();
    
    let trade_counts = [100, 1000, 10000];
    
    for &count in &trade_counts {
        // Generate trades
        let trades: Vec<TradeRecord> = (0..count)
            .map(|i| TradeRecord {
                trade_id: i as u64,
                instrument_id: (i % 10) as u32,
                quantity: rand::thread_rng().gen::<f64>() * 10.0,
                price: 100.0 + rand::thread_rng().gen::<f64>() * 5.0,
                timestamp_ns: i as u64 * 1_000_000,
                side: (i % 2) as u8,
            })
            .collect();
        
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("calculate_pnl", count), &count, |b, _| {
            b.iter(|| {
                for trade in &trades {
                    pnl_calc.record_trade(trade.clone());
                }
                black_box(pnl_calc.calculate_total_pnl(&positions, &book));
            })
        });
    }
    
    group.finish();
}

// ============================================================
// REAL-TIME RISK MONITORING
// ============================================================

fn bench_real_time_risk_monitoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_time_risk");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut engine = RiskEngine::new(RiskConfig::default(), tx);
    let book = generate_test_order_book();
    let positions = generate_positions(10);
    
    let update_counts = [100, 500, 1000];
    
    for &count in &update_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("risk_updates", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    let _ = engine.update(&book, &positions);
                    black_box(engine.can_trade(1.0));
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// COMPARATIVE RISK METHOD BENCHMARKS
// ============================================================

fn bench_risk_method_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("risk_method_comparison");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(15));
    
    let returns: Vec<f64> = (0..1000)
        .map(|_| rand::thread_rng().gen::<f64>() * 0.02 - 0.01)
        .collect();
    
    let mut historical_var = HistoricalVaR::new(0.99, 1);
    let mut parametric_var = ParametricVaR::new(0.99, 1);
    let monte_carlo_var = MonteCarloVaR::new(0.99, 1, 10000);
    
    historical_var.update(&returns);
    parametric_var.update(&returns);
    
    let positions = generate_positions(10);
    let book = generate_test_order_book();
    
    group.bench_function("historical_var", |b| {
        b.iter(|| {
            black_box(historical_var.calculate(&positions, &book, 0.99));
        })
    });
    
    group.bench_function("parametric_var", |b| {
        b.iter(|| {
            black_box(parametric_var.calculate(&positions, &book, 0.99));
        })
    });
    
    group.bench_function("monte_carlo_var", |b| {
        b.iter(|| {
            black_box(monte_carlo_var.calculate(&positions, &book, 0.99));
        })
    });
    
    group.finish();
}

// ============================================================
// RISK METRICS AGGREGATION
// ============================================================

fn bench_risk_metrics_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("risk_aggregation");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let num_instruments = [10, 50, 100];
    
    for &n in &num_instruments {
        let positions = generate_positions(n);
        let mut risk_metrics = RiskMetrics::default();
        
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("aggregate_metrics", n), &n, |b, &_| {
            b.iter(|| {
                let total_position: f64 = positions.iter().map(|p| p.quantity).sum();
                let total_pnl: f64 = positions.iter().map(|p| p.current_pnl).sum();
                
                risk_metrics.current_position = total_position;
                risk_metrics.current_pnl = total_pnl;
                
                black_box(&risk_metrics);
            })
        });
    }
    
    group.finish();
}

// ============================================================
// REGISTER BENCHMARKS
// ============================================================

criterion_group!(
    name = risk_compute_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(500);
    targets = 
        bench_historical_var,
        bench_parametric_var,
        bench_monte_carlo_var,
        bench_risk_gate_computation,
        bench_stress_testing,
        bench_pnl_calculation,
        bench_real_time_risk_monitoring,
        bench_risk_method_comparison,
        bench_risk_metrics_aggregation
);

criterion_main!(risk_compute_benches);
