#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub latency_p50_ns: u64,
    pub latency_p99_ns: u64,
    pub throughput_ticks_sec: f64,
    pub detection_probability: f64,
    pub sharpe_ratio: f64,
    pub total_trades: u64,
    pub total_pnl: f64,
}

pub struct MetricsCollector;

impl MetricsCollector {
    pub fn new() -> Self { Self }
    pub fn record_metric(&self, _n: &str, _v: f64, _t: MetricType) {}
    pub fn record_latency(&self, _n: &str, _l: u64) {}
    pub fn record_error(&self, _e: String) {}
    pub async fn collect(&self) {}
    pub fn snapshot(&self) -> SystemMetrics { SystemMetrics::default() }
}

pub enum MetricType { Gauge, Counter, Histogram }
pub struct MetricValue;
