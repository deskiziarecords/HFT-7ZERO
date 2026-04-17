// ============================================================
// MONITORING MODULE
// ============================================================
// Real-time system monitoring and metrics collection
// Latency watchdog with P99 tracking
// Detection probability tracking
// Alert system with multiple severity levels
// ============================================================

pub mod metrics;
pub mod latency_watchdog;
pub mod detection_tracker;
pub mod alerts;
pub mod health_check;
pub mod telemetry;

pub use metrics::{MetricsCollector, MetricType, MetricValue, SystemMetrics};
pub use latency_watchdog::{LatencyWatchdog, WatchdogConfig, LatencyBreach};
pub use detection_tracker::{DetectionTracker, DetectionEvent, DetectionRiskLevel};
pub use alerts::{AlertManager, Alert, AlertSeverity, AlertChannel};
pub use health_check::{HealthChecker, ComponentHealth, HealthStatus};
pub use telemetry::{TelemetryCollector, TelemetryData, SpanCollector};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use tokio::sync::mpsc;

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub metrics_interval_ms: u64,
    pub latency_threshold_ns: u64,
    pub detection_threshold: f64,
    pub alert_cooldown_ms: u64,
    pub enable_telemetry: bool,
    pub enable_profiling: bool,
    pub metrics_retention_days: u32,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_interval_ms: 100,
            latency_threshold_ns: 1_000_000, // 1ms
            detection_threshold: 0.001,       // 0.1%
            alert_cooldown_ms: 5000,          // 5 seconds
            enable_telemetry: true,
            enable_profiling: false,
            metrics_retention_days: 7,
        }
    }
}

/// Global monitoring instance
pub static MONITORING: once_cell::sync::Lazy<Arc<MonitoringSystem>> =
    once_cell::sync::Lazy::new(|| Arc::new(MonitoringSystem::new()));

/// Main monitoring system
pub struct MonitoringSystem {
    config: RwLock<MonitoringConfig>,
    metrics: Arc<MetricsCollector>,
    latency_watchdog: Arc<LatencyWatchdog>,
    detection_tracker: Arc<DetectionTracker>,
    alert_manager: Arc<AlertManager>,
    health_checker: Arc<HealthChecker>,
    telemetry: Option<Arc<TelemetryCollector>>,
    alert_tx: mpsc::UnboundedSender<Alert>,
}

impl MonitoringSystem {
    /// Create new monitoring system
    pub fn new() -> Self {
        let (alert_tx, alert_rx) = mpsc::unbounded_channel();

        let metrics = Arc::new(MetricsCollector::new());
        let latency_watchdog = Arc::new(LatencyWatchdog::new(WatchdogConfig::default()));
        let detection_tracker = Arc::new(DetectionTracker::new());
        let alert_manager = Arc::new(AlertManager::new(alert_rx));
        let health_checker = Arc::new(HealthChecker::new());

        Self {
            config: RwLock::new(MonitoringConfig::default()),
            metrics: metrics.clone(),
            latency_watchdog: latency_watchdog.clone(),
            detection_tracker: detection_tracker.clone(),
            alert_manager: alert_manager.clone(),
            health_checker: health_checker.clone(),
            telemetry: None,
            alert_tx,
        }
    }

    /// Start monitoring system
    pub async fn start(&self) {
        // Start alert manager
        self.alert_manager.start();

        // Start metrics collection loop
        let metrics = self.metrics.clone();
        let interval_ms = self.config.read().metrics_interval_ms;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                metrics.collect().await;
            }
        });

        // Start health check loop
        let health_checker = self.health_checker.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                health_checker.run_checks().await;
            }
        });

        tracing::info!("Monitoring system started");
    }

    /// Record latency metric
    pub fn record_latency(&self, operation: &str, latency_ns: u64) {
        self.metrics.record_latency(operation, latency_ns);
        self.latency_watchdog.record_latency(operation, latency_ns);

        // Check threshold
        let threshold = self.config.read().latency_threshold_ns;
        if latency_ns > threshold {
            let _ = self.alert_tx.send(Alert::latency_breach(operation, latency_ns, threshold));
        }
    }

    /// Record detection event
    pub fn record_detection(&self, event: DetectionEvent) {
        self.detection_tracker.record_event(event.clone());

        if event.risk_level >= DetectionRiskLevel::Medium {
            let _ = self.alert_tx.send(Alert::detection_risk(event));
        }
    }

    /// Record system metric
    pub fn record_metric(&self, name: &str, value: f64, metric_type: MetricType) {
        self.metrics.record_metric(name, value, metric_type);
    }

    /// Send alert
    pub fn send_alert(&self, alert: Alert) {
        let _ = self.alert_tx.send(alert);
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> SystemMetrics {
        self.metrics.snapshot()
    }

    /// Get detection statistics
    pub fn get_detection_stats(&self) -> DetectionStats {
        self.detection_tracker.get_stats()
    }

    /// Get health status
    pub fn health_status(&self) -> HealthStatus {
        self.health_checker.overall_status()
    }
}

/// Detection statistics
#[derive(Debug, Clone, Default)]
pub struct DetectionStats {
    pub total_events: u64,
    pub high_risk_events: u64,
    pub critical_risk_events: u64,
    pub current_risk_level: DetectionRiskLevel,
    pub last_event_time_ns: u64,
    pub avg_risk_score: f64,
}
