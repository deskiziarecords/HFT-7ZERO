// ============================================================
// RISK TRIGGERS
// ============================================================
// Individual risk trigger definitions
// Severity levels and actions
// Debouncing and hysteresis
// ============================================================

use super::*;

/// Risk trigger types (6 layers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {
    Lambda1,  // Volatility regime
    Lambda2,  // Kurtosis/drift
    Lambda3,  // Harmonic trap
    Lambda4,  // Fill probability
    Lambda5,  // Potential gradient
    Lambda6,  // Candle body conflict
}

/// Trigger severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerSeverity {
    Info,       // Informational only
    Warning,    // Warning, but continue
    Critical,   // Stop trading
    Emergency,  // Emergency shutdown
}

/// Trigger action
#[derive(Debug, Clone)]
pub enum TriggerAction {
    LogOnly,
    ReducePosition(f64),  // Reduce by percentage
    ClosePosition,
    HaltTrading(u64),      // Halt for milliseconds
    EmergencyShutdown,
}

/// Trigger configuration
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    pub enabled: bool,
    pub severity: TriggerSeverity,
    pub action: TriggerAction,
    pub debounce_ms: u64,
    pub hysteresis: f64,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            severity: TriggerSeverity::Warning,
            action: TriggerAction::LogOnly,
            debounce_ms: 100,
            hysteresis: 0.1,
        }
    }
}

/// Risk triggers manager
pub struct RiskTriggers {
    triggers: std::collections::HashMap<TriggerType, TriggerConfig>,
    last_trigger_time: std::collections::HashMap<TriggerType, u64>,
    trigger_history: VecDeque<(TriggerType, u64)>,
}

impl RiskTriggers {
    /// Create new triggers manager
    pub fn new() -> Self {
        let mut triggers = std::collections::HashMap::new();
        
        // Configure each trigger with appropriate severity
        triggers.insert(TriggerType::Lambda1, TriggerConfig {
            severity: TriggerSeverity::Warning,
            action: TriggerAction::ReducePosition(0.5),
            ..Default::default()
        });
        
        triggers.insert(TriggerType::Lambda2, TriggerConfig {
            severity: TriggerSeverity::Critical,
            action: TriggerAction::HaltTrading(5000),
            ..Default::default()
        });
        
        triggers.insert(TriggerType::Lambda3, TriggerConfig {
            severity: TriggerSeverity::Emergency,
            action: TriggerAction::EmergencyShutdown,
            ..Default::default()
        });
        
        triggers.insert(TriggerType::Lambda4, TriggerConfig {
            severity: TriggerSeverity::Warning,
            action: TriggerAction::ReducePosition(0.3),
            ..Default::default()
        });
        
        triggers.insert(TriggerType::Lambda5, TriggerConfig {
            severity: TriggerSeverity::Critical,
            action: TriggerAction::ClosePosition,
            ..Default::default()
        });
        
        triggers.insert(TriggerType::Lambda6, TriggerConfig {
            severity: TriggerSeverity::Warning,
            action: TriggerAction::LogOnly,
            ..Default::default()
        });
        
        Self {
            triggers,
            last_trigger_time: std::collections::HashMap::new(),
            trigger_history: VecDeque::with_capacity(1000),
        }
    }
    
    /// Process a trigger
    pub fn process(&mut self, trigger: TriggerType, value: f64) -> Option<TriggerAction> {
        let config = self.triggers.get(&trigger)?;
        
        if !config.enabled {
            return None;
        }
        
        // Check debounce
        let now = crate::utils::time::get_hardware_timestamp();
        if let Some(&last) = self.last_trigger_time.get(&trigger) {
            if now - last < config.debounce_ms * 1_000_000 {
                return None;
            }
        }
        
        // Record trigger
        self.last_trigger_time.insert(trigger, now);
        self.trigger_history.push_back((trigger, now));
        while self.trigger_history.len() > 1000 {
            self.trigger_history.pop_front();
        }
        
        // Log trigger
        tracing::warn!(
            "Risk trigger: {:?} (severity={:?}, value={:.4})",
            trigger, config.severity, value
        );
        
        Some(config.action.clone())
    }
    
    /// Get trigger configuration
    pub fn get_config(&self, trigger: TriggerType) -> Option<&TriggerConfig> {
        self.triggers.get(&trigger)
    }
    
    /// Update trigger configuration
    pub fn update_config(&mut self, trigger: TriggerType, config: TriggerConfig) {
        self.triggers.insert(trigger, config);
    }
    
    /// Get recent trigger history
    pub fn recent_triggers(&self, duration_ms: u64) -> Vec<TriggerType> {
        let now = crate::utils::time::get_hardware_timestamp();
        let cutoff = now - duration_ms * 1_000_000;
        
        self.trigger_history
            .iter()
            .filter(|(_, ts)| *ts >= cutoff)
            .map(|(t, _)| *t)
            .collect()
    }
    
    /// Get trigger count in time window
    pub fn trigger_count(&self, trigger: TriggerType, duration_ms: u64) -> usize {
        let now = crate::utils::time::get_hardware_timestamp();
        let cutoff = now - duration_ms * 1_000_000;
        
        self.trigger_history
            .iter()
            .filter(|(t, ts)| *t == trigger && *ts >= cutoff)
            .count()
    }
}
