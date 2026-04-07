// ============================================================
// METRICS COLLECTOR
// ============================================================
// Real-time metrics collection and aggregation
// Histograms, counters, gauges
// Prometheus-compatible export
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicF64, Ordering};
use std::time::{Duration, Instant};

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,    // Ever-increasing counter
    Gauge,      // Point-in-time value
    Histogram,  // Distribution of values
    Summary,    // Quantile summary
}

/// Metric value with timestamp
#[derive(Debug, Clone)]
pub struct MetricValue {
    pub name: String,
    pub value: f64,
    pub metric_type: MetricType,
    pub timestamp_ns: u64,
    pub labels: Vec<(String, String)>,
}

/// Histogram bucket
#[derive(Debug, Clone)]
pub struct Histogram {
    pub bounds: Vec<f64>,
    pub counts: Vec<AtomicU64>,
    pub sum: AtomicF64,
    pub count: AtomicU64,
}

impl Histogram {
    pub fn new(bounds: Vec<f64>) -> Self {
        let counts = (0..=bounds.len()).map(|_| AtomicU64::new(0)).collect();
        Self {
            bounds,
            counts,
            sum: AtomicF64::new(0.0),
            count: AtomicU64::new(0),
        }
    }
    
    pub fn observe(&self, value: f64) {
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        
        for (i, &bound) in self.bounds.iter().enumerate() {
            if value <= bound {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        // Last bucket for values exceeding all bounds
        self.counts[self.bounds.len()].fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn percentile(&self, p: f64) -> f64 {
        let total = self.count.load(Ordering::Relaxed) as f64;
        if total == 0.0 {
            return 0.0;
        }
        
        let target = total * p;
        let mut cumulative = 0.0;
        
        for (i, &bound) in self.bounds.iter().enumerate() {
            cumulative += self.counts[i].load(Ordering::Relaxed) as f64;
            if cumulative >= target {
                return bound;
            }
        }
        
        self.bounds.last().copied().unwrap_or(0.0)
    }
}

/// System metrics snapshot
#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    // Latency metrics
    pub latency_p50_ns: u64,
    pub latency_p95_ns: u64,
    pub latency_p99_ns: u64,
    pub latency_p999_ns: u64,
    pub max_latency_ns: u64,
    
    // Throughput metrics
    pub ticks_per_second: f64,
    pub orders_per_second: f64,
    pub fills_per_second: f64,
    
    // System metrics
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub goroutines: u64,
    
    // Trading metrics
    pub total_pnl: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub total_trades: u64,
    
    // Detection metrics
    pub detection_probability: f64,
    pub stealth_score: f64,
    
    // Timestamp
    pub timestamp_ns: u64,
}

/// Metrics collector
pub struct MetricsCollector {
    // Latency histograms
    pipeline_latency: Histogram,
    inference_latency: Histogram,
    execution_latency: Histogram,
    
    // Counters
    ticks_processed: AtomicU64,
    orders_submitted: AtomicU64,
    orders_filled: AtomicU64,
    errors: AtomicU64,
    
    // Gauges
    current_position: AtomicF64,
    current_pnl: AtomicF64,
    detection_risk: AtomicF64,
    
    // Rate tracking
    tick_rate: AtomicF64,
    order_rate: AtomicF64,
    
    // History
    history: VecDeque<SystemMetrics>,
    max_history: usize,
    
    // Last collection time
    last_collection: Instant,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        let latency_bounds = vec![
            100.0, 250.0, 500.0, 750.0, 1000.0, 2500.0, 5000.0, 10000.0, 25000.0, 50000.0, 100000.0
        ];
        
        Self {
            pipeline_latency: Histogram::new(latency_bounds.clone()),
            inference_latency: Histogram::new(latency_bounds.clone()),
            execution_latency: Histogram::new(latency_bounds),
            ticks_processed: AtomicU64::new(0),
            orders_submitted: AtomicU64::new(0),
            orders_filled: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            current_position: AtomicF64::new(0.0),
            current_pnl: AtomicF64::new(0.0),
            detection_risk: AtomicF64::new(0.0),
            tick_rate: AtomicF64::new(0.0),
            order_rate: AtomicF64::new(0.0),
            history: VecDeque::with_capacity(1000),
            max_history: 1000,
            last_collection: Instant::now(),
        }
    }
    
    /// Record latency for an operation
    pub fn record_latency(&self, operation: &str, latency_ns: u64) {
        match operation {
            "pipeline" => self.pipeline_latency.observe(latency_ns as f64),
            "inference" => self.inference_latency.observe(latency_ns as f64),
            "execution" => self.execution_latency.observe(latency_ns as f64),
            _ => {}
        }
    }
    
    /// Record metric value
    pub fn record_metric(&self, name: &str, value: f64, metric_type: MetricType) {
        match metric_type {
            MetricType::Counter => {
                match name {
                    "ticks" => self.ticks_processed.fetch_add(value as u64, Ordering::Relaxed),
                    "orders" => self.orders_submitted.fetch_add(value as u64, Ordering::Relaxed),
                    "fills" => self.orders_filled.fetch_add(value as u64, Ordering::Relaxed),
                    "errors" => self.errors.fetch_add(value as u64, Ordering::Relaxed),
                    _ => {}
                };
            }
            MetricType::Gauge => {
                match name {
                    "position" => self.current_position.store(value, Ordering::Relaxed),
                    "pnl" => self.current_pnl.store(value, Ordering::Relaxed),
                    "detection_risk" => self.detection_risk.store(value, Ordering::Relaxed),
                    _ => {}
                };
            }
            _ => {}
        }
    }
    
    /// Increment counter
    pub fn increment_counter(&self, name: &str) {
        self.record_metric(name, 1.0, MetricType::Counter);
    }
    
    /// Update gauge
    pub fn update_gauge(&self, name: &str, value: f64) {
        self.record_metric(name, value, MetricType::Gauge);
    }
    
    /// Collect all metrics
    pub async fn collect(&self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_collection);
        
        // Update rates
        let ticks = self.ticks_processed.load(Ordering::Relaxed);
        let orders = self.orders_submitted.load(Ordering::Relaxed);
        
        // Store collection time
        self.last_collection = now;
        
        // Update rate metrics (exponential moving average)
        let tick_rate = ticks as f64 / elapsed.as_secs_f64();
        let order_rate = orders as f64 / elapsed.as_secs_f64();
        
        self.tick_rate.store(tick_rate, Ordering::Relaxed);
        self.order_rate.store(order_rate, Ordering::Relaxed);
    }
    
    /// Get current metrics snapshot
    pub fn snapshot(&self) -> SystemMetrics {
        SystemMetrics {
            latency_p50_ns: self.pipeline_latency.percentile(0.50) as u64,
            latency_p95_ns: self.pipeline_latency.percentile(0.95) as u64,
            latency_p99_ns: self.pipeline_latency.percentile(0.99) as u64,
            latency_p999_ns: self.pipeline_latency.percentile(0.999) as u64,
            max_latency_ns: self.pipeline_latency.percentile(1.0) as u64,
            
            ticks_per_second: self.tick_rate.load(Ordering::Relaxed),
            orders_per_second: self.order_rate.load(Ordering::Relaxed),
            fills_per_second: 0.0, // Would need separate tracking
            
            cpu_usage_percent: self.get_cpu_usage(),
            memory_usage_mb: self.get_memory_usage(),
            goroutines: self.get_goroutine_count(),
            
            total_pnl: self.current_pnl.load(Ordering::Relaxed),
            sharpe_ratio: self.calculate_sharpe(),
            win_rate: self.calculate_win_rate(),
            total_trades: self.orders_filled.load(Ordering::Relaxed),
            
            detection_probability: self.detection_risk.load(Ordering::Relaxed),
            stealth_score: 1.0 - self.detection_risk.load(Ordering::Relaxed),
            
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
        }
    }
    
    /// Get CPU usage percentage
    fn get_cpu_usage(&self) -> f64 {
        // Platform-specific CPU measurement
        // Simplified - in production use sysinfo crate
        0.0
    }
    
    /// Get memory usage in MB
    fn get_memory_usage(&self) -> f64 {
        // Platform-specific memory measurement
        0.0
    }
    
    /// Get goroutine/thread count
    fn get_goroutine_count(&self) -> u64 {
        std::thread::active_count() as u64
    }
    
    /// Calculate Sharpe ratio
    fn calculate_sharpe(&self) -> f64 {
        // Would need PnL history
        0.0
    }
    
    /// Calculate win rate
    fn calculate_win_rate(&self) -> f64 {
        // Would need trade history
        0.0
    }
    
    /// Export to Prometheus format
    pub fn export_prometheus(&self) -> String {
        let metrics = self.snapshot();
        
        format!(
            r#"# HELP hft_latency_p99_ns P99 latency in nanoseconds
# TYPE hft_latency_p99_ns gauge
hft_latency_p99_ns {}

# HELP hft_ticks_per_second Ticks processed per second
# TYPE hft_ticks_per_second gauge
hft_ticks_per_second {}

# HELP hft_orders_per_second Orders submitted per second
# TYPE hft_orders_per_second gauge
hft_orders_per_second {}

# HELP hft_detection_probability Current detection risk
# TYPE hft_detection_probability gauge
hft_detection_probability {}

# HELP hft_total_pnl Total profit/loss
# TYPE hft_total_pnl gauge
hft_total_pnl {}

# HELP hft_sharpe_ratio Sharpe ratio
# TYPE hft_sharpe_ratio gauge
hft_sharpe_ratio {}
"#,
            metrics.latency_p99_ns,
            metrics.ticks_per_second,
            metrics.orders_per_second,
            metrics.detection_probability,
            metrics.total_pnl,
            metrics.sharpe_ratio,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_histogram() {
        let hist = Histogram::new(vec![100.0, 200.0, 500.0, 1000.0]);
        
        hist.observe(50.0);
        hist.observe(150.0);
        hist.observe(300.0);
        hist.observe(800.0);
        hist.observe(2000.0);
        
        assert_eq!(hist.percentile(0.5), 500.0);
        assert_eq!(hist.percentile(0.9), 1000.0);
    }
    
    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        
        collector.record_latency("pipeline", 500_000);
        collector.record_latency("pipeline", 1_200_000);
        collector.record_latency("inference", 300_000);
        
        collector.increment_counter("ticks");
        collector.increment_counter("orders");
        
        let metrics = collector.snapshot();
        println!("Metrics: {:?}", metrics);
    }
}
