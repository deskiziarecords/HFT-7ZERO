// ============================================================
// MARKET OPERATING SYSTEM MODULE
// ============================================================
// Core market microstructure operators (L1-L6)
// Hazard rate dynamics
// Navier-Stokes liquidity field
// Gamma control and hedging
// Bankruptcy/circuit breaker system
// ============================================================

pub mod market_os;
pub mod hazard;
pub mod liquidity_field;
pub mod gamma_control;
pub mod bankruptcy;
pub mod regime_detector;
pub mod order_flow;

pub use market_os::{MarketOS, OSConfig, OSState, OperatorResult};
pub use hazard::{HazardRate, HazardConfig, HazardEvent};
pub use liquidity_field::{NavierStokesLiquidity, LiquidityField, FieldParams, Vorticity};
pub use gamma_control::{GammaController, GammaConfig, HedgeSignal};
pub use bankruptcy::{BankruptcyGate, CircuitBreaker, BreakerStatus, RecoveryPlan};
pub use regime_detector::{RegimeDetector, MarketRegime, RegimeSignal};
pub use order_flow::{OrderFlowAnalyzer, FlowMetrics, ImbalanceSignal};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// Market OS configuration
#[derive(Debug, Clone)]
pub struct OSConfig {
    // L1: Regime bounds
    pub regime_bounds: Vec<f64>,  // ℬ₂₀, ℬ₄₀, ℬ₆₀ bounds
    pub regime_threshold: f64,

    // L2: Hazard parameters
    pub hazard_alpha: [f64; 3],
    pub hazard_decay: f64,

    // L3: Macro injection
    pub macro_shock_alpha: f64,
    pub macro_event_window_ns: u64,

    // L4: Liquidity field
    pub liquidity_viscosity: f64,
    pub liquidity_diffusion: f64,
    pub field_resolution: usize,

    // L5: Gamma control
    pub gamma_eta: f64,
    pub gamma_kappa: f64,
    pub gamma_target: f64,

    // L6: Bankruptcy
    pub max_drawdown: f64,
    pub max_loss: f64,
    pub auto_recovery: bool,
}

impl Default for OSConfig {
    fn default() -> Self {
        Self {
            regime_bounds: vec![20.0, 40.0, 60.0],
            regime_threshold: 0.5,
            hazard_alpha: [0.1, 0.5, 0.2],
            hazard_decay: 0.99,
            macro_shock_alpha: 0.2,
            macro_event_window_ns: 1_000_000_000,
            liquidity_viscosity: 0.01,
            liquidity_diffusion: 0.001,
            field_resolution: 256,
            gamma_eta: 0.1,
            gamma_kappa: 0.5,
            gamma_target: 0.0,
            max_drawdown: 0.05,
            max_loss: 100_000.0,
            auto_recovery: true,
        }
    }
}

/// Market OS state
#[derive(Debug, Clone)]
pub struct OSState {
    pub regime: Vec<(f64, f64)>,      // ℛ_t
    pub hazard_rate: f64,              // h_t
    pub volatility: f64,               // σ_t
    pub gamma: f64,                    // Γ_t
    pub theta: bool,                   // θ_t (bankruptcy flag)
    pub liquidity_field: Vec<f64>,     // u(x,t)
    pub vorticity: Vec<f64>,           // ω
    pub timestamp_ns: u64,
    pub sequence: u64,
}

impl Default for OSState {
    fn default() -> Self {
        Self {
            regime: Vec::new(),
            hazard_rate: 0.0,
            volatility: 0.01,
            gamma: 0.0,
            theta: false,
            liquidity_field: Vec::new(),
            vorticity: Vec::new(),
            timestamp_ns: 0,
            sequence: 0,
        }
    }
}

/// Operator result with metrics
#[derive(Debug, Clone)]
pub struct OperatorResult {
    pub success: bool,
    pub state_change: bool,
    pub latency_ns: u64,
    pub value: f64,
    pub message: String,
}

/// Global Market OS instance
pub static MARKET_OS: once_cell::sync::Lazy<Arc<MarketOS>> = once_cell::sync::Lazy::new(|| {
    Arc::new(MarketOS::new(OSConfig::default()))
});
