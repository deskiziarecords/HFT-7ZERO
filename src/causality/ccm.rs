// ============================================================
// CONVERGENT CROSS MAPPING (𝒞_CCM)
// ============================================================
// Nonlinear causality detection
// State space reconstruction using Takens' theorem
// Convergence testing with increasing library size
// ============================================================

use super::*;

/// CCM configuration
#[derive(Debug, Clone)]
pub struct CCMConfig {
    pub embedding_dim: usize,
    pub tau: usize,
    pub num_neighbors: usize,
    pub library_sizes: Vec<usize>,
    pub convergence_threshold: f64,
}

impl Default for CCMConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 3,
            tau: 1,
            num_neighbors: 5,
            library_sizes: vec![50, 100, 200, 300, 400, 500],
            convergence_threshold: 0.1,
        }
    }
}

/// CCM result
#[derive(Debug, Clone)]
pub struct CCMResult {
    pub rho: f64,                    // Cross-map skill
    pub convergence_slope: f64,      // Slope of convergence
    pub is_causal: bool,
    pub library_sizes: Vec<usize>,
    pub rho_values: Vec<f64>,
    pub embedding_dim: usize,
    pub tau: usize,
}

/// Convergent cross mapping
pub struct ConvergentCrossMapping {
    config: CCMConfig,
    cache: HashMap<u64, CCMResult>,
}

impl ConvergentCrossMapping {
    /// Create new CCM calculator
    pub fn new(config: CCMConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }
    
    /// Test causality from X to Y using CCM
    pub fn test(&mut self, y: &[f64], x: &[f64]) -> CCMResult {
        let n = y.len().min(x.len());
        let min_len = self.config.library_sizes.last().copied().unwrap_or(n / 2);
        
        if n < min_len + self.config.embedding_dim * self.config.tau {
            return CCMResult {
                rho: 0.0,
                convergence_slope: 0.0,
                is_causal: false,
                library_sizes: self.config.library_sizes.clone(),
                rho_values: vec![0.0; self.config.library_sizes.len()],
                embedding_dim: self.config.embedding_dim,
                tau: self.config.tau,
            };
        }
        
        // Build shadow manifolds for Y and X
        let y_manifold = self.build_shadow_manifold(y);
        let x_manifold = self.build_shadow_manifold(x);
        
        let mut rho_values = Vec::with_capacity(self.config.library_sizes.len());
        
        // Compute cross-map skill for increasing library sizes
        for &lib_size in &self.config.library_sizes {
            if lib_size > n {
                break;
            }
            let rho = self.cross_map_skill(&y_manifold, &x_manifold, lib_size);
            rho_values.push(rho);
        }
        
        // Check convergence: as library size increases, rho should increase
        let convergence_slope = self.compute_convergence_slope(&rho_values);
        let is_causal = convergence_slope > self.config.convergence_threshold && 
                        rho_values.last().copied().unwrap_or(0.0) > 0.5;
        
        CCMResult {
            rho: rho_values.last().copied().unwrap_or(0.0),
            convergence_slope,
            is_causal,
            library_sizes: self.config.library_sizes.clone(),
            rho_values,
            embedding_dim: self.config.embedding_dim,
            tau: self.config.tau,
        }
    }
    
    /// Build shadow manifold using Takens' embedding theorem
    fn build_shadow_manifold(&self, time_series: &[f64]) -> Vec<Vec<f64>> {
        let n = time_series.len();
        let e = self.config.embedding_dim;
        let tau = self.config.tau;
        let max_idx = n - (e - 1) * tau;
        
        let mut manifold = Vec::with_capacity(max_idx);
        
        for i in 0..max_idx {
            let mut point = Vec::with_capacity(e);
            for j in 0..e {
                point.push(time_series[i + j * tau]);
            }
            manifold.push(point);
        }
        
        manifold
    }
    
    /// Cross-map skill: predict Y from X manifold
    fn cross_map_skill(&self, y_manifold: &[Vec<f64>], x_manifold: &[Vec<f64>], library_size: usize) -> f64 {
        let n = y_manifold.len().min(x_manifold.len()).min(library_size);
        if n < self.config.num_neighbors + 1 {
            return 0.0;
        }
        
        let mut predictions = Vec::with_capacity(n);
        let mut actuals = Vec::with_capacity(n);
        
        for i in 0..n {
            let target = &x_manifold[i];
            
            // Find nearest neighbors in X manifold
            let mut neighbors: Vec<(usize, f64)> = (0..n)
                .filter(|&j| j != i)
                .map(|j|
