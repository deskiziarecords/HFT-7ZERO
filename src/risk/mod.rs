pub mod engine;
pub mod gate;
pub mod triggers;
pub mod var;
pub mod limits;
pub mod pnl;
pub mod ev_atr;
pub mod stress_test;

pub use engine::RiskEngine;
pub use gate::{RiskGate, GateStatus, GateDecision, GateContext};
pub use triggers::{RiskTriggers, TriggerType, TriggerSeverity};
pub use var::{ValueAtRisk, HistoricalVaR, ParametricVaR};
pub use pnl::{PnLCalculator, TradeRecord, Position};
pub use stress_test::{StressTester, StressScenario, ScenarioResult};
pub use limits::{PositionLimits, RiskLimits, LimitBreach};

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub var_confidence: f64,
    pub var_horizon_seconds: u64,
    pub max_correlation: f64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size: 1_000_000.0,
            max_daily_loss: 100_000.0,
            max_drawdown: 0.05,
            var_confidence: 0.99,
            var_horizon_seconds: 1,
            max_correlation: 0.7,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RiskMetrics {
    pub current_position: f64,
    pub current_pnl: f64,
    pub daily_pnl: f64,
    pub daily_loss: f64,
    pub drawdown: f64,
    pub var_95: f64,
    pub var_99: f64,
    pub expected_shortfall: f64,
    pub correlation_matrix: Vec<Vec<f64>>,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone)]
pub enum RiskEvent {
    LimitBreached(LimitBreach),
    GateTriggered(TriggerType),
    DrawdownLimitHit { drawdown: f64, limit: f64 },
    VaRExceeded { var_value: f64, actual_loss: f64 },
}
