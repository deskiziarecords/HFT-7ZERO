// ============================================================
// RISK MANAGEMENT MODULE
// ============================================================
// Multi-layer risk gate system
// Real-time VaR calculations
// Position limits and stress testing
// Automatic circuit breakers
// ============================================================

pub mod engine;
pub mod gate;
pub mod triggers;
pub mod var;
pub mod stress_test;
pub mod limits;
pub mod pnl;
pub mod ev_atr;

pub use engine::RiskEngine;
pub use gate::{RiskGate, GateStatus, GateDecision};
pub use triggers::{RiskTriggers, TriggerType, TriggerSeverity};
pub use var::{ValueAtRisk, HistoricalVaR, ParametricVaR};
pub use stress_test::{StressTester, StressScenario, ScenarioResult};
pub use limits::{PositionLimits, RiskLimits, LimitBreach};
pub use pnl::{PnLCalculator, TradeRecord, Position};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// Risk configuration
#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub var_confidence: f64,
    pub var_horizon_seconds: u64,
    pub max_correlation: f64,
    pub stress_test_enabled: bool,
    pub auto_liquidation: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size: 1_000_000.0,
            max_daily_loss: 100_000.0,
            max_drawdown: 0.05, // 5%
            var_confidence: 0.99,
            var_horizon_seconds: 1,
            max_correlation: 0.7,
            stress_test_enabled: true,
            auto_liquidation: true,
        }
    }
}

/// Risk metrics snapshot
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

/// Risk event
#[derive(Debug, Clone)]
pub enum RiskEvent {
    LimitBreached(LimitBreach),
    GateTriggered(TriggerType),
    VaRExceeded { var_value: f64, actual_loss: f64 },
    DrawdownLimitHit { drawdown: f64, limit: f64 },
    AutoLiquidation { position: f64, loss: f64 },
}

/// Global risk state
pub struct RiskState {
    pub config: RiskConfig,
    pub metrics: Arc<RwLock<RiskMetrics>>,
    pub positions: DashMap<u32, Position>,
    pub trades: DashMap<u64, TradeRecord>,
    pub risk_gate: Arc<RiskGate>,
    pub event_sender: tokio::sync::mpsc::UnboundedSender<RiskEvent>,
}

impl RiskState {
    pub fn new(config: RiskConfig, event_sender: tokio::sync::mpsc::UnboundedSender<RiskEvent>) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(RiskMetrics::default())),
            positions: DashMap::new(),
            trades: DashMap::new(),
            risk_gate: Arc::new(RiskGate::new()),
            event_sender,
        }
    }
}
