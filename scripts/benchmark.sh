#!/bin/bash
# ============================================================
# BENCHMARK SCRIPT
# ============================================================
# Runs performance benchmarks with various configurations
# Generates comparison reports
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="${PROJECT_ROOT}/benches"
RESULTS_DIR="${PROJECT_ROOT}/benchmark_results"
DATE=$(date +%Y%m%d_%H%M%S)

# ============================================================
# CONFIGURATION
# ============================================================

THREAD_COUNTS=(1 2 4 8)
BATCH_SIZES=(1 8 16 32 64)
LATENCY_THRESHOLDS=(500 1000 2000)  # microseconds

# ============================================================
# FUNCTIONS
# ============================================================

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

setup() {
    mkdir -p "$RESULTS_DIR"
    
    # Build benchmarks
    log "Building benchmarks..."
    cargo bench --no-run --features benchmarking
    
    # Check if criterion is installed
    if ! command -v critcmp >/dev/null 2>&1; then
        log "Installing critcmp for benchmark comparison..."
        cargo install critcmp
    fi
}

run_latency_bench() {
    log "Running latency benchmarks..."
    
    cargo bench --bench latency_bench -- --verbose \
        --save-baseline "latency_${DATE}" \
        --output-format bencher \
        2>&1 | tee "${RESULTS_DIR}/latency_${DATE}.txt"
}

run_throughput_bench() {
    log "Running throughput benchmarks..."
    
    for threads in "${THREAD_COUNTS[@]}"; do
        log "  Threads: $threads"
        export HFT_THREAD_POOL_SIZE=$threads
        
        cargo bench --bench throughput_bench -- --verbose \
            --save-baseline "throughput_${threads}t_${DATE}" \
            2>&1 | tee "${RESULTS_DIR}/throughput_${threads}t_${DATE}.txt"
    done
}

run_risk_bench() {
    log "Running risk computation benchmarks..."
    
    cargo bench --bench risk_compute_bench -- --verbose \
        --save-baseline "risk_${DATE}" \
        2>&1 | tee "${RESULTS_DIR}/risk_${DATE}.txt"
}

run_scalability_test() {
    log "Running scalability tests..."
    
    local results_file="${RESULTS_DIR}/scalability_${DATE}.csv"
    echo "threads,latency_p99_ns,throughput_ticks_sec" > "$results_file"
    
    for threads in "${THREAD_COUNTS[@]}"; do
        log "  Testing with $threads threads..."
        
        # Run with specific thread count
        export HFT_THREAD_POOL_SIZE=$threads
        
        # Extract results from benchmark output
        cargo bench --bench throughput_bench -- --quiet 2>/dev/null | \
            grep -E "throughput|latency" | \
            tail -2 >> "$results_file" || true
    done
    
    log "Scalability results saved to $results_file"
}

run_stress_test() {
    log "Running stress tests..."
    
    local duration=60  # seconds
    local results_file="${RESULTS_DIR}/stress_${DATE}.csv"
    
    echo "timestamp,cpu_percent,memory_mb,latency_p99_ns,detection_risk" > "$results_file"
    
    # Start system in background
    "${SCRIPT_DIR}/run_prod.sh" start
    
    # Monitor for duration
    for i in $(seq 1 $duration); do
        timestamp=$(date +%s)
        
        # Get metrics from monitoring endpoint
        if command -v curl >/dev/null 2>&1; then
            metrics=$(curl -s "http://localhost:9090/metrics" 2>/dev/null || echo "")
            
            cpu=$(echo "$metrics" | grep "hft_cpu_percent" | awk '{print $2}' || echo "0")
            memory=$(echo "$metrics" | grep "hft_memory_mb" | awk '{print $2}' || echo "0")
            latency=$(echo "$metrics" | grep "hft_latency_p99_ns" | awk '{print $2}' || echo "0")
            detection=$(echo "$metrics" | grep "hft_detection_probability" | awk '{print $2}' || echo "0")
            
            echo "$timestamp,$cpu,$memory,$latency,$detection" >> "$results_file"
        fi
        
        sleep 1
    done
    
    # Stop system
    "${SCRIPT_DIR}/run_prod.sh" stop
    
    log "Stress test results saved to $results_file"
}

generate_report() {
    log "Generating benchmark report..."
    
    local report_file="${RESULTS_DIR}/report_${DATE}.md"
    
    cat > "$report_file" << EOF
# HFT Stealth System Benchmark Report
## Generated: $(date)

### System Information
- CPU: $(lscpu | grep "Model name" | cut -d: -f2 | xargs)
- Cores: $(nproc)
- Memory: $(free -h | grep Mem | awk '{print $2}')
- OS: $(uname -a)

### Latency Results
\`\`\`
$(cat "${RESULTS_DIR}/latency_${DATE}.txt" 2>/dev/null | grep -E "P50|P95|P99|P999" || echo "No data")
\`\`\`

### Throughput Results
\`\`\`
$(cat "${RESULTS_DIR}/throughput_*_${DATE}.txt" 2>/dev/null | grep -E "throughput|elements/s" || echo "No data")
\`\`\`

### Risk Computation Results
\`\`\`
$(cat "${RESULTS_DIR}/risk_${DATE}.txt" 2>/dev/null | grep -E "historical|parametric|monte" || echo "No data")
\`\`\`

### Scalability Analysis
\`\`\`
$(cat "${RESULTS_DIR}/scalability_${DATE}.csv" 2>/dev/null || echo "No data")
\`\`\`

### Conclusions
- P99 Latency: $(grep "P99" "${RESULTS_DIR}/latency_${DATE}.txt" 2>/dev/null | head -1 || echo "N/A")
- Max Throughput: $(grep -o "[0-9.]* elements/s" "${RESULTS_DIR}/throughput_*_${DATE}.txt" 2>/dev/null | head -1 || echo "N/A")
- Detection Probability: $(grep "detection" "${RESULTS_DIR}/stress_${DATE}.csv" 2>/dev/null | tail -1 | cut -d, -f5 || echo "N/A")

EOF
    
    log "Report generated: $report_file"
    
    # Display summary
    echo ""
    echo "========== BENCHMARK SUMMARY =========="
    grep -E "P99|Throughput|Detection" "$report_file" || true
    echo "========================================"
}

compare_baselines() {
    if command -v critcmp >/dev/null 2>&1; then
        log "Comparing with previous baselines..."
        critcmp
    fi
}

# ============================================================
# MAIN
# ============================================================

case "${1:-}" in
    all)
        setup
        run_latency_bench
        run_throughput_bench
        run_risk_bench
        run_scalability_test
        run_stress_test
        generate_report
        compare_baselines
        ;;
    latency)
        setup
        run_latency_bench
        ;;
    throughput)
        setup
        run_throughput_bench
        ;;
    risk)
        setup
        run_risk_bench
        ;;
    scalability)
        setup
        run_scalability_test
        ;;
    stress)
        setup
        run_stress_test
        ;;
    report)
        generate_report
        ;;
    compare)
        compare_baselines
        ;;
    *)
        echo "Usage: $0 {all|latency|throughput|risk|scalability|stress|report|compare}"
        echo ""
        echo "Commands:"
        echo "  all         - Run all benchmarks"
        echo "  latency     - Run latency benchmarks"
        echo "  throughput  - Run throughput benchmarks"
        echo "  risk        - Run risk computation benchmarks"
        echo "  scalability - Run scalability tests"
        echo "  stress      - Run stress tests"
        echo "  report      - Generate report from existing results"
        echo "  compare     - Compare with previous baselines"
        exit 1
        ;;
esac
