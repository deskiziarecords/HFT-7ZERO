use std::collections::{HashMap, VecDeque};
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub metadata: HashMap<String, String>,
}

impl Alert {
    pub fn latency_breach(_op: &str, _lat: u64, _thresh: u64) -> Self {
        Self { severity: AlertSeverity::Warning, metadata: HashMap::new() }
    }
    pub fn detection_risk(_event: super::DetectionEvent) -> Self {
        Self { severity: AlertSeverity::Error, metadata: HashMap::new() }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AlertSeverity { Info, Warning, Error, Critical, Emergency }

pub struct AlertManager {
    pub alerts: RwLock<VecDeque<Alert>>,
    pub alert_history: RwLock<VecDeque<Alert>>,
}

impl AlertManager {
    pub fn new(_rx: tokio::sync::mpsc::UnboundedReceiver<Alert>) -> Self {
        Self {
            alerts: RwLock::new(VecDeque::new()),
            alert_history: RwLock::new(VecDeque::new()),
        }
    }
    pub fn start(&self) {}
}

pub enum AlertChannel { Slack, Email, PagerDuty }
