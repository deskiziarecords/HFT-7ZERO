pub mod frequency;
pub mod market_os;
pub mod hazard;
pub mod liquidity_field;
pub mod gamma_control;
pub mod bankruptcy;
pub mod regime_detector;
pub mod order_flow;
pub mod adaptive_controller;

pub use frequency::{UpdateFrequency, OperatorType};
pub use market_os::{MarketOS, OSMetrics, OperatorResult};
pub use adaptive_controller::AdaptiveController;
pub use hazard::HazardRate;
pub use gamma_control::GammaController;
pub use bankruptcy::BankruptcyGate;
pub use regime_detector::{RegimeDetector, MarketRegime, RegimeSignal};
pub use order_flow::{OrderFlowAnalyzer, FlowMetrics, ImbalanceSignal};

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct OSConfig {
    pub regime_bounds: Vec<f64>,
    pub regime_threshold: f64,
    pub hazard_alpha: [f64; 3],
    pub hazard_decay: f64,
    pub macro_shock_alpha: f64,
    pub macro_event_window_ns: u64,
    pub liquidity_viscosity: f64,
    pub liquidity_diffusion: f64,
    pub field_resolution: usize,
    pub gamma_eta: f64,
    pub gamma_kappa: f64,
    pub gamma_target: f64,
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

#[derive(Debug, Clone, Default)]
pub struct OSState {
    pub regime: Vec<(f64, f64)>,
    pub hazard_rate: f64,
    pub volatility: f64,
    pub gamma: f64,
    pub theta: bool,
    pub liquidity_field: Vec<f64>,
    pub vorticity: Vec<f64>,
    pub timestamp_ns: u64,
    pub sequence: u64,
}

pub static MARKET_OS: once_cell::sync::Lazy<Arc<MarketOS>> = once_cell::sync::Lazy::new(|| {
    Arc::new(MarketOS::new(OSConfig::default()))
});
