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
                .map(|j| {
                    let dist = self.euclidean_distance(target, &x_manifold[j]);
                    (j, dist)
                })
                .collect();

            neighbors.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // Weighted prediction using nearest neighbors
            let total_weight: f64 = neighbors.iter()
                .take(self.config.num_neighbors)
                .map(|&(_, d)| 1.0 / (d + 1e-8))
                .sum();

            let prediction: f64 = neighbors.iter()
                .take(self.config.num_neighbors)
                .map(|&(j, d)| y_manifold[j][0] / (d + 1e-8))
                .sum::<f64>() / total_weight;

            predictions.push(prediction);
            actuals.push(y_manifold[i][0]);
        }

        // Compute Pearson correlation between predictions and actuals
        self.pearson_correlation(&predictions, &actuals)
    }

    /// Euclidean distance between two vectors
    fn euclidean_distance(&self, a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b.iter())
            .map(|(ai, bi)| (ai - bi).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Pearson correlation coefficient
    fn pearson_correlation(&self, x: &[f64], y: &[f64]) -> f64 {
        let n = x.len().min(y.len());
        if n < 2 {
            return 0.0;
        }

        let mean_x = x.iter().sum::<f64>() / n as f64;
        let mean_y = y.iter().sum::<f64>() / n as f64;

        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for i in 0..n {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }

        cov / (var_x.sqrt() * var_y.sqrt() + 1e-8)
    }

    /// Compute convergence slope using linear regression on log scale
    fn compute_convergence_slope(&self, rho_values: &[f64]) -> f64 {
        if rho_values.len() < 2 {
            return 0.0;
        }

        let x: Vec<f64> = (0..rho_values.len()).map(|i| (i + 1) as f64).collect();
        let y: Vec<f64> = rho_values.iter().copied().collect();

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
        let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);

        slope.max(0.0)
    }

    /// Test bidirectional causality
    pub fn test_bidirectional(&mut self, y: &[f64], x: &[f64]) -> (bool, bool) {
        let x_to_y = self.test(y, x);
        let y_to_x = self.test(x, y);

        (x_to_y.is_causal, y_to_x.is_causal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccm() {
        let config = CCMConfig::default();
        let mut ccm = ConvergentCrossMapping::new(config);

        // Generate data from coupled logistic maps
        let n = 500;
        let mut x = vec![0.5; n];
        let mut y = vec![0.3; n];

        for i in 1..n {
            x[i] = 3.8 * x[i-1] * (1.0 - x[i-1]);
            y[i] = 3.8 * y[i-1] * (1.0 - y[i-1]) + 0.2 * x[i-1];
        }

        let result = ccm.test(&y, &x);
        println!("CCM rho: {:.4}, slope: {:.4}, causal: {}",
                 result.rho, result.convergence_slope, result.is_causal);
    }
}
