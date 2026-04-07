// ============================================================
// TRANSFER ENTROPY (𝒯_ent^(6-bin))
// ============================================================
// Information-theoretic causality measure
// 6-bin discretization for market data
// Bias-corrected estimation
// ============================================================

use super::*;
use std::collections::HashMap;

/// Transfer entropy configuration
#[derive(Debug, Clone)]
pub struct TEConfig {
    pub n_bins: usize,
    pub max_lag: usize,
    pub bias_correction: bool,
    pub shuffles: usize,
}

impl Default for TEConfig {
    fn default() -> Self {
        Self {
            n_bins: 6,
            max_lag: 5,
            bias_correction: true,
            shuffles: 100,
        }
    }
}

/// Transfer entropy result
#[derive(Debug, Clone)]
pub struct TEResult {
    pub te_value: f64,
    pub te_normalized: f64,
    pub z_score: f64,
    pub p_value: f64,
    pub is_significant: bool,
    pub optimal_lag: usize,
    pub effective_bins: usize,
}

/// Transfer entropy calculator
pub struct TransferEntropy {
    config: TEConfig,
    bins: Vec<f64>,
    histories: HashMap<u64, TEResult>,
}

impl TransferEntropy {
    /// Create new transfer entropy calculator
    pub fn new(config: TEConfig) -> Self {
        Self {
            config,
            bins: Vec::new(),
            histories: HashMap::new(),
        }
    }
    
    /// Calculate transfer entropy from X to Y
    /// TE_{X→Y} = H(Y_t | Y_{t-1}) - H(Y_t | Y_{t-1}, X_{t-τ})
    pub fn calculate(&mut self, y: &[f64], x: &[f64], lag: usize) -> TEResult {
        let n = y.len().min(x.len());
        if n < self.config.n_bins * 10 {
            return TEResult {
                te_value: 0.0,
                te_normalized: 0.0,
                z_score: 0.0,
                p_value: 1.0,
                is_significant: false,
                optimal_lag: lag,
                effective_bins: 0,
            };
        }
        
        // Discretize data into 6 bins
        let y_discrete = self.discretize(y, self.config.n_bins);
        let x_discrete = self.discretize(x, self.config.n_bins);
        
        // Build joint probability distributions
        let mut p_y_given_y_prev = HashMap::new();
        let mut p_y_given_y_prev_x = HashMap::new();
        
        for t in lag + 1..n {
            let y_t = y_discrete[t];
            let y_prev = y_discrete[t - 1];
            let x_prev = if t >= lag { x_discrete[t - lag] } else { 0 };
            
            // P(Y_t | Y_{t-1})
            let key = (y_t, y_prev);
            *p_y_given_y_prev.entry(key).or_insert(0) += 1;
            
            // P(Y_t | Y_{t-1}, X_{t-τ})
            let key2 = (y_t, y_prev, x_prev);
            *p_y_given_y_prev_x.entry(key2).or_insert(0) += 1;
        }
        
        // Convert to probabilities
        let total = (n - lag - 1) as f64;
        let mut p_y = HashMap::new();
        let mut p_y_prev = HashMap::new();
        let mut p_x = HashMap::new();
        
        for ((y_t, y_prev), count) in &p_y_given_y_prev {
            *p_y.entry(y_t).or_insert(0.0) += *count as f64;
            *p_y_prev.entry(y_prev).or_insert(0.0) += *count as f64;
        }
        
        for ((_, _, x_prev), count) in &p_y_given_y_prev_x {
            *p_x.entry(x_prev).or_insert(0.0) += *count as f64;
        }
        
        // Calculate entropy H(Y_t | Y_{t-1})
        let h_y_given_y_prev = self.conditional_entropy(&p_y_given_y_prev, &p_y_prev, total);
        
        // Calculate entropy H(Y_t | Y_{t-1}, X_{t-τ})
        let h_y_given_y_prev_x = self.conditional_entropy_3d(&p_y_given_y_prev_x, &p_y_prev, &p_x, total);
        
        // Transfer entropy = difference
        let mut te = h_y_given_y_prev - h_y_given_y_prev_x;
        
        // Bias correction using shuffling
        let (bias, std_err) = if self.config.bias_correction {
            self.estimate_bias(y, x, lag)
        } else {
            (0.0, 0.01)
        };
        
        te = (te - bias).max(0.0);
        
        // Normalized transfer entropy [0, 1]
        let te_normalized = 1.0 - (-te / (h_y_given_y_prev + 1e-8)).exp();
        
        // Z-score and p-value
        let z_score = te / (std_err + 1e-8);
        let p_value = 2.0 * (1.0 - self.normal_cdf(z_score.abs()));
        
        TEResult {
            te_value: te,
            te_normalized,
            z_score,
            p_value,
            is_significant: p_value < 0.05,
            optimal_lag: lag,
            effective_bins: self.config.n_bins,
        }
    }
    
    /// Discretize continuous data into n bins using equal frequency
    fn discretize(&self, data: &[f64], n_bins: usize) -> Vec<usize> {
        if data.is_empty() {
            return vec![];
        }
        
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let mut edges = Vec::with_capacity(n_bins + 1);
        for i in 0..=n_bins {
            let idx = (i * (sorted.len() - 1) / n_bins).min(sorted.len() - 1);
            edges.push(sorted[idx]);
        }
        
        data.iter()
            .map(|&v| {
                for (i, &edge) in edges.iter().enumerate().skip(1) {
                    if v <= edge {
                        return i - 1;
                    }
                }
                n_bins - 1
            })
            .collect()
    }
    
    /// Conditional entropy H(Y|Z)
    fn conditional_entropy(&self, joint: &HashMap<(usize, usize), i32>, marginal_z: &HashMap<usize, f64>, total: f64) -> f64 {
        let mut entropy = 0.0;
        
        for ((y, z), &count) in joint {
            let p_yz = count as f64 / total;
            let p_z = marginal_z.get(z).copied().unwrap_or(1e-8);
            let p_y_given_z = p_yz / p_z;
            
            entropy += -p_yz * p_y_given_z.ln();
        }
        
        entropy
    }
    
    /// Conditional entropy H(Y|Z1, Z2)
    fn conditional_entropy_3d(&self, joint: &HashMap<(usize, usize, usize), i32>, 
                               marginal_z1: &HashMap<usize, f64>,
                               marginal_z2: &HashMap<usize, f64>,
                               total: f64) -> f64 {
        let mut entropy = 0.0;
        
        for ((y, z1, z2), &count) in joint {
            let p_yz1z2 = count as f64 / total;
            let p_z1 = marginal_z1.get(z1).copied().unwrap_or(1e-8);
            let p_z2 = marginal_z2.get(z2).copied().unwrap_or(1e-8);
            let p_joint = p_z1 * p_z2;
            let p_y_given = p_yz1z2 / (p_joint + 1e-8);
            
            entropy += -p_yz1z2 * p_y_given.ln();
        }
        
        entropy
    }
    
    /// Estimate bias using shuffling
    fn estimate_bias(&self, y: &[f64], x: &[f64], lag: usize) -> (f64, f64) {
        let mut te_shuffled = Vec::with_capacity(self.config.shuffles);
        
        for _ in 0..self.config.shuffles {
            // Shuffle X while preserving Y
            let mut shuffled_x: Vec<f64> = x.to_vec();
            self.shuffle(&mut shuffled_x);
            
            let result = self.calculate(y, &shuffled_x, lag);
            te_shuffled.push(result.te_value);
        }
        
        let bias = te_shuffled.iter().sum::<f64>() / te_shuffled.len() as f64;
        let variance = te_shuffled.iter()
            .map(|&te| (te - bias).powi(2))
            .sum::<f64>() / (te_shuffled.len() - 1) as f64;
        let std_err = variance.sqrt();
        
        (bias, std_err)
    }
    
    /// Find optimal lag
    pub fn find_optimal_lag(&mut self, y: &[f64], x: &[f64], max_lag: usize) -> usize {
        let mut best_lag = 1;
        let mut best_te = -f64::INFINITY;
        
        for lag in 1..=max_lag.min(y.len() / 10) {
            let result = self.calculate(y, x, lag);
            if result.te_value > best_te {
                best_te = result.te_value;
                best_lag = lag;
            }
        }
        
        best_lag
    }
    
    fn normal_cdf(&self, x: f64) -> f64 {
        // Approximation of standard normal CDF
        let t = 1.0 / (1.0 + 0.2316419 * x.abs());
        let d = 0.3989423 * (-x * x / 2.0).exp();
        let p = d * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
        
        if x > 0.0 {
            1.0 - p
        } else {
            p
        }
    }
    
    fn shuffle(&self, data: &mut [f64]) {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        data.shuffle(&mut rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transfer_entropy() {
        let config = TEConfig::default();
        let mut te = TransferEntropy::new(config);
        
        // Generate data with causal relationship
        let n = 1000;
        let mut x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).sin()).collect();
        let mut y: Vec<f64> = x.iter().map(|&v| v * 0.8).collect();
        
        for i in 1..n {
            y[i] += 0.1 * (i as f64).sin();
        }
        
        let result = te.calculate(&y, &x, 1);
        println!("TE: {:.6}, normalized: {:.6}, p-value: {:.6}", 
                 result.te_value, result.te_normalized, result.p_value);
    }
}
