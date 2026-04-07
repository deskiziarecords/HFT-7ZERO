### Latency Benchmarks (latency_bench.rs)

    Order Book Updates: Single and batch update latencies

    Risk Gate: All 6 lambda triggers, individual and combined

    Harmonic Detector: Phase inversion detection with/without classification

    ML Inference: Feature extraction and batch inference

    Full Pipeline: End-to-end tick→signal latency

    Execution Pipeline: Gate check, stealth execution, fragmentation

    Latency Distribution: P50, P95, P99, P999 percentiles

    Component Comparison: Relative performance across modules

    Stress Tests: High-throughput concurrent processing
    
  ### Throughput Benchmarks (throughput_bench.rs)

    Packet Processing: Single and batch packet handling

    Order Book: Update throughput and query rates

    Risk Engine: Risk calculation throughput

    Signal Processing: Harmonic detection throughput

    Execution: Order execution and fragmentation rates

    Concurrent Processing: Parallel risk computation scaling

    Memory Operations: Zero-copy buffer throughput

### Risk Computation Benchmarks (risk_compute_bench.rs)

    VaR Methods: Historical, Parametric, Monte Carlo comparison

    Risk Gate: Individual lambda performance profiling

    Stress Testing: Scenario execution throughput

    PnL Calculation: Real-time profit/loss tracking

    Real-time Monitoring: Continuous risk metric updates



``` bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench --bench latency_bench
cargo bench --bench throughput_bench
cargo bench --bench risk_compute_bench

# Run with specific filter
cargo bench -- latency
cargo bench -- risk_gate

# Run with release optimizations
cargo bench --release

# Save detailed results
cargo bench -- --save-baseline baseline
cargo bench -- --baseline baseline --load-baseline new

# Generate HTML report
cargo bench -- --verbose --output-format bencher | tee results.txt

```
  Method Comparison: Accuracy vs. speed trade-offs

  Metrics Aggregation: Portfolio-level risk aggregation
