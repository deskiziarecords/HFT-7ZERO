pub mod granger;
pub mod transfer_entropy;
pub mod ccm;
pub mod spearman;
pub mod fusion;
pub mod lag_selection;
pub mod causality_network;

pub use granger::{GrangerCausality, GrangerResult};
pub use transfer_entropy::{TransferEntropy, TEResult};
pub use ccm::{ConvergentCrossMapping, CCMResult};
pub use spearman::{SpearmanCorrelation, SpearmanResult};
pub use fusion::{SignalFusion, FusionConfig, FusedSignal};

use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct CausalityConfig {
    pub max_lag: usize,
    pub significance_level: f64,
    pub bootstrap_iterations: usize,
    pub min_sample_size: usize,
}

impl Default for CausalityConfig {
    fn default() -> Self {
        Self {
            max_lag: 10,
            significance_level: 0.05,
            bootstrap_iterations: 100,
            min_sample_size: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CausalityResult {
    pub optimal_lag: usize,
    pub phi_t: f64,
}

pub static CAUSALITY_CACHE: once_cell::sync::Lazy<DashMap<u64, CausalityResult>> =
    once_cell::sync::Lazy::new(|| DashMap::new());
