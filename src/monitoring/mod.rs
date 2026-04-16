pub mod metrics;
pub mod interaction_logger;
pub mod alerts;
pub mod detection_tracker;
pub mod latency_watchdog;
pub mod health_check;
pub mod telemetry;

pub use metrics::{MetricsCollector, MetricType, MetricValue, SystemMetrics};
pub use interaction_logger::InteractionLogger;
pub use detection_tracker::{DetectionEvent, DetectionRiskLevel, DetectionTracker};
pub use alerts::{Alert, AlertSeverity};
pub use health_check::{HealthChecker, ComponentHealth, HealthStatus};
pub use telemetry::{TelemetryCollector, TelemetryData, SpanCollector};

use std::sync::Arc;
#[derive(Debug, Clone, Default)]
pub struct DetectionStats {
    pub total_events: u64,
    pub high_risk_events: u64,
    pub critical_risk_events: u64,
    pub current_risk_level: DetectionRiskLevel,
    pub last_event_time_ns: u64,
    pub avg_risk_score: f64,
}

pub struct MonitoringSystem {
    pub metrics: Arc<MetricsCollector>,
}

impl MonitoringSystem {
    pub fn new() -> Self {
        Self { metrics: Arc::new(MetricsCollector::new()) }
    }
}

pub static MONITORING: once_cell::sync::Lazy<Arc<MonitoringSystem>> =
    once_cell::sync::Lazy::new(|| Arc::new(MonitoringSystem::new()));
