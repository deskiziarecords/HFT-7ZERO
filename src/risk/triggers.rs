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
            action: TriggerAction::HaltTrading
