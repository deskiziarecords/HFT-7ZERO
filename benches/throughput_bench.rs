// ============================================================
// THROUGHPUT BENCHMARK
// ============================================================
// Measures maximum processing rate
// Packets per second, ticks per second
// Orders per second, fills per second
// ============================================================

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
    Throughput, SamplingMode,
};
use hft_stealth_system::*;
use std::sync::Arc;
use std::time::Duration;

// ============================================================
// PACKET PROCESSING THROUGHPUT
// ============================================================

fn bench_packet_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_processing");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    // Generate synthetic packets
    let packet_sizes = [64, 128, 256, 512, 1024, 1518];
    
    for &size in &packet_sizes {
        let packet = vec![0u8; size];
        
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("process_packet", size), &packet, |b, p| {
            b.iter(|| {
                // Simulate packet parsing
                let timestamp = get_hardware_timestamp();
                let parsed = black_box(p.len());
                black_box((timestamp, parsed));
            })
        });
    }
    
    group.finish();
}

fn bench_batch_packet_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_packet_processing");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let batch_sizes = [64, 128, 256, 512, 1024];
    
    for &batch_size in &batch_sizes {
        let packets: Vec<Vec<u8>> = (0..batch_size)
            .map(|_| vec![0u8; 128])
            .collect();
        
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_process", batch_size), &packets, |b, pkts| {
            b.iter(|| {
                for packet in pkts {
                    black_box(packet.len());
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// ORDER BOOK THROUGHPUT
// ============================================================

fn bench_order_book_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let update_counts = [1000, 10000, 100000];
    
    for &count in &update_counts {
        let ticks = generate_test_ticks(count);
        
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("updates", count), &ticks, |b, tk| {
            let mut book = OrderBook::new(1, 0.01);
            b.iter(|| {
                for tick in tk {
                    book.update(black_box(tick));
                }
            })
        });
    }
    
    group.finish();
}

fn bench_order_book_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_queries");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    // Pre-populate order book
    let mut book = OrderBook::new(1, 0.01);
    let ticks = generate_test_ticks(1000);
    for tick in &ticks {
        book.update(tick);
    }
    
    let query_counts = [1000, 10000, 100000];
    
    for &count in &query_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("best_bid", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    black_box(book.best_bid());
                }
            })
        });
        
        group.bench_with_input(BenchmarkId::new("depth_query", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    black_box(book.top_levels(10));
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// RISK ENGINE THROUGHPUT
// ============================================================

fn bench_risk_engine_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("risk_engine_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut engine = RiskEngine::new(RiskConfig::default(), tx);
    let book = generate_test_order_book();
    let positions = dashmap::DashMap::new();
    
    let update_counts = [1000, 10000];
    
    for &count in &update_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("risk_updates", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    let _ = engine.update(&book, &positions);
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// SIGNAL PROCESSING THROUGHPUT
// ============================================================

fn bench_signal_processing_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("signal_processing_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let mut detector = HarmonicTrapDetector::new(256);
    let prices: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    let batch_sizes = [100, 500, 1000];
    
    for &batch_size in &batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::new("harmonic_detection", batch_size), &batch_size, |b, &bs| {
            b.iter(|| {
                for _ in 0..bs {
                    black_box(detector.detect_trap(&prices, &prices));
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// EXECUTION THROUGHPUT
// ============================================================

fn bench_execution_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    let order_counts = [100, 500, 1000];
    
    for &count in &order_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("execute_orders", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    let mut order_clone = order.clone();
                    black_box(executor.execute(&mut order_clone, &book));
                }
            })
        });
    }
    
    group.finish();
}

fn bench_fragmentation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("fragmentation_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    let config = FragmentConfig::default();
    let mut fragmenter = Fragmenter::new(config);
    
    let fragment_counts = [1000, 10000, 100000];
    
    for &count in &fragment_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("fragment_orders", count), &count, |b, &c| {
            b.iter(|| {
                for _ in 0..c {
                    black_box(fragmenter.fragment(0.025, 100.00));
                }
            })
        });
    }
    
    group.finish();
}

// ============================================================
// CONCURRENT THROUGHPUT
// ============================================================

fn bench_concurrent_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(15));
    
    use std::thread;
    use std::sync::Arc;
    
    let thread_counts = [1, 2, 4, 8];
    let iterations_per_thread = 10000;
    
    for &threads in &thread_counts {
        group.throughput(Throughput::Elements((threads * iterations_per_thread) as u64));
        group.bench_with_input(BenchmarkId::new("parallel_risk", threads), &threads, |b, &t| {
            b.iter(|| {
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                let engine = Arc::new(RiskEngine::new(RiskConfig::default(), tx));
                let book = Arc::new(generate_test_order_book());
                let positions = Arc::new(dashmap::DashMap::new());
                
                let mut handles = vec![];
                for _ in 0..t {
                    let engine = engine.clone();
                    let book = book.clone();
                    let positions = positions.clone();
                    
                    handles.push(thread::spawn(move || {
                        for _ in 0..iterations_per_thread {
                            let _ = engine.update(&book, &positions);
                        }
                    }));
                }
                
                for handle in handles {
                    handle.join().unwrap();
                }
                black_box(());
            })
        });
    }
    
    group.finish();
}

// ============================================================
// MEMORY THROUGHPUT
// ============================================================

fn bench_memory_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_throughput");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    
    // Zero-copy buffer throughput
    let buffer_sizes = [1024, 4096, 16384, 65536];
    
    for &size in &buffer_sizes {
        let mut buffer = ZeroCopyBuffer::new(size).unwrap();
        let data = vec![0u8; size];
        
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("zero_copy_write", size), &size, |b, &_sz| {
            b.iter(|| {
                buffer.write_direct(|slice| {
                    slice.copy_from_slice(&data);
                    data.len()
                });
                black_box(());
            })
        });
        
        group.bench_with_input(BenchmarkId::new("zero_copy_read", size), &size, |b, &_sz| {
            b.iter(|| {
                buffer.read_direct(|slice| {
                    black_box(slice.len());
                });
            })
        });
    }
    
    group.finish();
}

// ============================================================
// REGISTER BENCHMARKS
// ============================================================

criterion_group!(
    name = throughput_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(500);
    targets = 
        bench_packet_processing,
        bench_batch_packet_processing,
        bench_order_book_throughput,
        bench_order_book_queries,
        bench_risk_engine_throughput,
        bench_signal_processing_throughput,
        bench_execution_throughput,
        bench_fragmentation_throughput,
        bench_concurrent_throughput,
        bench_memory_throughput
);

criterion_main!(throughput_benches);
