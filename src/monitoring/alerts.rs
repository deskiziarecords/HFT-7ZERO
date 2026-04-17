// ============================================================
// ALERT MANAGER
// ============================================================
// Multi-channel alert system
// Severity-based escalation
// Cooldown and deduplication
// ============================================================

use super::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tokio::sync::mpsc;

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum AlertSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
    Emergency = 4,
}

/// Alert channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertChannel {
    Log,
    Console,
    Email,
    Slack,
    PagerDuty,
    Webhook,
}

/// Alert structure
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub channel: AlertChannel,
    pub timestamp_ns: u64,
    pub source: String,
    pub metadata: HashMap<String, String>,
    pub acknowledged: bool,
    pub resolved: bool,
}

impl Alert {
    /// Create new alert
    pub fn new(title: &str, message: &str, severity: AlertSeverity) -> Self {
        Self {
            id: crate::utils::time::get_hardware_timestamp(),
            title: title.to_string(),
            message: message.to_string(),
            severity,
            channel: AlertChannel::Log,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            source: "hft-system".to_string(),
            metadata: HashMap::new(),
            acknowledged: false,
            resolved: false,
        }
    }

    /// Create latency breach alert
    pub fn latency_breach(operation: &str, latency_ns: u64, threshold_ns: u64) -> Self {
        let severity = if latency_ns > threshold_ns * 2 {
            AlertSeverity::Critical
        } else {
            AlertSeverity::Error
        };

        Self::new(
            &format!("Latency breach: {}", operation),
            &format!("{} latency = {}ns (threshold: {}ns)", operation, latency_ns, threshold_ns),
            severity,
        )
    }

    /// Create detection risk alert
    pub fn detection_risk(event: DetectionEvent) -> Self {
        let severity = match event.risk_level {
            DetectionRiskLevel::Medium => AlertSeverity::Warning,
            DetectionRiskLevel::High => AlertSeverity::Error,
            DetectionRiskLevel::VeryHigh => AlertSeverity::Critical,
            DetectionRiskLevel::Critical => AlertSeverity::Emergency,
            _ => AlertSeverity::Info,
        };

        let mut alert = Self::new(
            &format!("Detection risk: {:?}", event.event_type),
            &format!("Risk score: {:.4}, Level: {:?}", event.risk_score, event.risk_level),
            severity,
        );
        alert.metadata.insert("risk_score".to_string(), event.risk_score.to_string());
        alert.metadata.insert("event_type".to_string(), format!("{:?}", event.event_type));
        alert
    }

    /// Create system health alert
    pub fn system_health(component: &str, status: &str) -> Self {
        Self::new(
            &format!("Health: {}", component),
            &format!("Component {} is {}", component, status),
            if status == "unhealthy" { AlertSeverity::Error } else { AlertSeverity::Info },
        )
    }
}

/// Alert manager
pub struct AlertManager {
    alerts: RwLock<VecDeque<Alert>>,
    alert_history: RwLock<VecDeque<Alert>>,
    last_alert_time: RwLock<HashMap<String, u64>>,
    config: AlertManagerConfig,
    rx: mpsc::UnboundedReceiver<Alert>,
    max_alerts: usize,
}

/// Alert manager configuration
#[derive(Debug, Clone)]
pub struct AlertManagerConfig {
    pub cooldown_ms: u64,
    pub max_alerts_per_minute: u32,
    pub enable_email: bool,
    pub enable_slack: bool,
    pub enable_pagerduty: bool,
    pub slack_webhook_url: String,
    pub pagerduty_integration_key: String,
}

impl Default for AlertManagerConfig {
    fn default() -> Self {
        Self {
            cooldown_ms: 5000,
            max_alerts_per_minute: 60,
            enable_email: false,
            enable_slack: false,
            enable_pagerduty: false,
            slack_webhook_url: String::new(),
            pagerduty_integration_key: String::new(),
        }
    }
}

impl AlertManager {
    /// Create new alert manager
    pub fn new(rx: mpsc::UnboundedReceiver<Alert>) -> Self {
        Self {
            alerts: RwLock::new(VecDeque::with_capacity(1000)),
            alert_history: RwLock::new(VecDeque::with_capacity(10000)),
            last_alert_time: RwLock::new(HashMap::new()),
            config: AlertManagerConfig::default(),
            rx,
            max_alerts: 1000,
        }
    }

    /// Start alert manager
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut rx = self.rx;

            while let Some(alert) = rx.recv().await {
                self.process_alert(alert).await;
            }
        });
    }

    /// Process alert
    async fn process_alert(&self, alert: Alert) {
        // Check cooldown
        let key = format!("{}:{}", alert.source, alert.title);
        let now = crate::utils::time::get_hardware_timestamp();

        {
            let mut last_times = self.last_alert_time.write();
            if let Some(&last) = last_times.get(&key) {
                if now - last < self.config.cooldown_ms * 1_000_000 {
                    return; // Alert in cooldown
                }
            }
            last_times.insert(key, now);
        }

        // Store alert
        {
            let mut alerts = self.alerts.write();
            alerts.push_back(alert.clone());
            while alerts.len() > self.max_alerts {
                alerts.pop_front();
            }
        }

        // Store in history
        {
            let mut history = self.alert_history.write();
            history.push_back(alert.clone());
            while history.len() > 10000 {
                history.pop_front();
            }
        }

        // Route to appropriate channels
        self.route_alert(&alert).await;

        // Log alert
        match alert.severity {
            AlertSeverity::Emergency => tracing::error!("[EMERGENCY] {}: {}", alert.title, alert.message),
            AlertSeverity::Critical => tracing::error!("[CRITICAL] {}: {}", alert.title, alert.message),
            AlertSeverity::Error => tracing::error!("[ERROR] {}: {}", alert.title, alert.message),
            AlertSeverity::Warning => tracing::warn!("[WARNING] {}: {}", alert.title, alert.message),
            AlertSeverity::Info => tracing::info!("[INFO] {}: {}", alert.title, alert.message),
        }
    }

    /// Route alert to appropriate channels
    async fn route_alert(&self, alert: &Alert) {
        // Always log
        self.send_to_log(alert);

        // Console for critical+
        if alert.severity >= AlertSeverity::Critical {
            self.send_to_console(alert);
        }

        // Email for error+
        if self.config.enable_email && alert.severity >= AlertSeverity::Error {
            self.send_to_email(alert).await;
        }

        // Slack for warning+
        if self.config.enable_slack && alert.severity >= AlertSeverity::Warning {
            self.send_to_slack(alert).await;
        }

        // PagerDuty for critical+
        if self.config.enable_pagerduty && alert.severity >= AlertSeverity::Critical {
            self.send_to_pagerduty(alert).await;
        }
    }

    /// Send to log
    fn send_to_log(&self, alert: &Alert) {
        // Already handled by tracing
    }

    /// Send to console
    fn send_to_console(&self, alert: &Alert) {
        eprintln!("\n🔴 ALERT [{}]: {}\n   {}\n",
                  format!("{:?}", alert.severity).to_uppercase(),
                  alert.title,
                  alert.message);
    }

    /// Send to email (simplified)
    async fn send_to_email(&self, alert: &Alert) {
        // In production, integrate with SMTP
        tracing::debug!("Sending email alert: {}", alert.title);
    }

    /// Send to Slack
    async fn send_to_slack(&self, alert: &Alert) {
        if self.config.slack_webhook_url.is_empty() {
            return;
        }

        let color = match alert.severity {
            AlertSeverity::Emergency => "danger",
            AlertSeverity::Critical => "danger",
            AlertSeverity::Error => "danger",
            AlertSeverity::Warning => "warning",
            AlertSeverity::Info => "good",
        };

        let payload = serde_json::json!({
            "attachments": [{
                "color": color,
                "title": format!("[{}] {}", format!("{:?}", alert.severity).to_uppercase(), alert.title),
                "text": alert.message,
                "fields": [
                    {"title": "Source", "value": alert.source, "short": true},
                    {"title": "Time", "value": alert.timestamp_ns, "short": true}
                ],
                "ts": alert.timestamp_ns / 1_000_000_000
            }]
        });

        // In production, send HTTP request
        tracing::debug!("Sending Slack alert: {}", alert.title);
    }

    /// Send to PagerDuty
    async fn send_to_pagerduty(&self, alert: &Alert) {
        if self.config.pagerduty_integration_key.is_empty() {
            return;
        }

        let severity = match alert.severity {
            AlertSeverity::Emergency => "critical",
            AlertSeverity::Critical => "critical",
            AlertSeverity::Error => "error",
            AlertSeverity::Warning => "warning",
            AlertSeverity::Info => "info",
        };

        // In production, send PagerDuty event
        tracing::debug!("Sending PagerDuty alert: {}", alert.title);
    }

    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.alerts.read()
            .iter()
            .filter(|a| !a.resolved)
            .cloned()
            .collect()
    }

    /// Get alert history
    pub fn get_history(&self, count: usize) -> Vec<Alert> {
        self.alert_history.read()
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Acknowledge alert
    pub fn acknowledge(&self, alert_id: u64) -> bool {
        let mut alerts = self.alerts.write();
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            true
        } else {
            false
        }
    }

    /// Resolve alert
    pub fn resolve(&self, alert_id: u64) -> bool {
        let mut alerts = self.alerts.write();
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
            true
        } else {
            false
        }
    }

    /// Clear resolved alerts
    pub fn clear_resolved(&self) {
        let mut alerts = self.alerts.write();
        alerts.retain(|a| !a.resolved);
    }
}

/// Alert escalation manager
pub struct EscalationManager {
    levels: Vec<EscalationLevel>,
    current_level: usize,
    last_escalation: Instant,
}

#[derive(Debug, Clone)]
pub struct EscalationLevel {
    pub severity_threshold: AlertSeverity,
    pub delay_seconds: u64,
    pub actions: Vec<EscalationAction>,
}

#[derive(Debug, Clone)]
pub enum EscalationAction {
    NotifyManager,
    NotifyTeam,
    HaltTrading,
    EmergencyShutdown,
    CallOnCall,
}

impl EscalationManager {
    pub fn new() -> Self {
        let levels = vec![
            EscalationLevel {
                severity_threshold: AlertSeverity::Error,
                delay_seconds: 60,
                actions: vec![EscalationAction::NotifyTeam],
            },
            EscalationLevel {
                severity_threshold: AlertSeverity::Critical,
                delay_seconds: 30,
                actions: vec![EscalationAction::NotifyManager, EscalationAction::HaltTrading],
            },
            EscalationLevel {
                severity_threshold: AlertSeverity::Emergency,
                delay_seconds: 10,
                actions: vec![EscalationAction::CallOnCall, EscalationAction::EmergencyShutdown],
            },
        ];

        Self {
            levels,
            current_level: 0,
            last_escalation: Instant::now(),
        }
    }

    pub fn check_escalation(&mut self, highest_severity: AlertSeverity) -> Vec<EscalationAction> {
        let mut actions = Vec::new();

        for (i, level) in self.levels.iter().enumerate() {
            if highest_severity >= level.severity_threshold && i >= self.current_level {
                if self.last_escalation.elapsed() >= Duration::from_secs(level.delay_seconds) {
                    actions.extend(level.actions.clone());
                    self.current_level = i + 1;
                    self.last_escalation = Instant::now();
                }
                break;
            }
        }

        actions
    }

    pub fn reset(&mut self) {
        self.current_level = 0;
        self.last_escalation = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_creation() {
        let alert = Alert::latency_breach("pipeline", 2_000_000, 1_000_000);
        assert_eq!(alert.severity, AlertSeverity::Critical);
        assert!(alert.title.contains("Latency breach"));
    }

    #[test]
    fn test_escalation() {
        let mut escalation = EscalationManager::new();

        let actions = escalation.check_escalation(AlertSeverity::Critical);
        assert!(!actions.is_empty());
    }
}
