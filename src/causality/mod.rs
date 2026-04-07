// ============================================================
// CAUSALITY INFERENCE MODULE
// ============================================================
// Multi-method causal discovery for market microstructure
// Granger causality, Transfer Entropy, Convergent Cross Mapping
// Spearman correlation with latency, Signal fusion
// ============================================================

pub mod granger;
pub mod transfer_entropy;
pub mod ccm;
pub mod spearman;
pub mod fusion;
pub mod lag_selection;
pub mod causality_network;

pub use granger::{GrangerCausality, GrangerResult, VARModel};
pub use transfer_entropy::{TransferEntropy, TEConfig, TEResult};
pub use ccm::{ConvergentCrossMapping, CCMResult, CCMConfig};
pub use spearman::{SpearmanCorrelation, SpearmanResult, LaggedCorrelation};
pub use fusion::{SignalFusion, FusionConfig, FusedSignal, AdaptiveWeight};
pub use lag_selection::{LagSelector, AIC, BIC, HQIC};
pub use causality_network::{CausalityNetwork, CausalityEdge, NetworkMetrics};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// Causality configuration
#[derive(Debug, Clone)]
pub struct CausalityConfig {
    pub max_lag: usize,
    pub significance_level: f64,
    pub bootstrap_iterations: usize,
    pub min_sample_size: usize,
    pub use_parallel: bool,
    pub cache_results: bool,
}

impl Default for CausalityConfig {
    fn default() -> Self {
        Self {
            max_lag: 10,
            significance_level: 0.05,
            bootstrap_iterations: 1000,
            min_sample_size: 100,
            use_parallel: true,
            cache_results: true,
        }
    }
}

/// Causality result from all methods
#[derive(Debug, Clone)]
pub struct CausalityResult {
    pub granger_score: f64,
    pub granger_pvalue: f64,
    pub transfer_entropy: f64,
    pub te_std_error: f64,
    pub ccm_score: f64,
    pub ccm_rho: f64,
    pub spearman_rho: f64,
    pub optimal_lag: usize,
    pub is_significant: bool,
    pub timestamp_ns: u64,
    pub method_agreement: f64,  // Agreement between methods (0-1)
}

/// Causal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalDirection {
    XtoY,    // X causes Y
    YtoX,    // Y causes X
    Bidirectional,
    NoCausality,
    Undetermined,
}

/// Global causality cache
pub static CAUSALITY_CACHE: once_cell::sync::Lazy<Arc<CausalityCache>> = 
    once_cell::sync::Lazy::new(|| Arc::new(CausalityCache::new()));

/// Causality result cache
pub struct CausalityCache {
    cache: DashMap<u64, CausalityResult>,
    max_size: usize,
}

impl CausalityCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::with_capacity(10000),
            max_size: 10000,
        }
    }
    
    pub fn get(&self, key: u64) -> Option<CausalityResult> {
        self.cache.get(&key).map(|r| r.clone())
    }
    
    pub fn insert(&self, key: u64, result: CausalityResult) {
        if self.cache.len() >= self.max_size {
            // Remove oldest entry (simplified - would need LRU)
            if let Some(entry) = self.cache.iter().next() {
                self.cache.remove(entry.key());
            }
        }
        self.cache.insert(key, result);
    }
    
    pub fn clear(&self) {
        self.cache.clear();
    }
    
    fn hash_key(x: &[f64], y: &[f64], max_lag: usize) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        x.len().hash(&mut hasher);
        y.len().hash(&mut hasher);
        max_lag.hash(&mut hasher);
        
        // First few values for uniqueness
        if x.len() > 0 { x[0].to_bits().hash(&mut hasher); }
        if y.len() > 0 { y[0].to_bits().hash(&mut hasher); }
        if x.len() > 1 { x[x.len()/2].to_bits().hash(&mut hasher); }
        
        hasher.finish()
    }
}
