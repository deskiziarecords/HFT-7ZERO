// ============================================================
// SPEARMAN CORRELATION (ρ_Spearman(λ))
// ============================================================
// Rank-based correlation with latency
// Robust to outliers and non-linear relationships
// Lagged correlation analysis
// ============================================================

use super::*;

/// Spearman correlation result
#[derive(Debug, Clone)]
pub struct SpearmanResult {
    pub rho: f64,
    pub p_value: f64,
    pub is_significant: bool,
    pub optimal_lag: i32,
    pub confidence_interval: (f64, f64),
}

/// Lagged correlation result
#[derive(Debug, Clone)]
pub struct LaggedCorrelation {
    pub lags: Vec<i32>,
    pub correlations: Vec<f64>,
    pub max_correlation: f64,
    pub max_lag: i32,
    pub min_correlation: f64,
    pub min_lag: i32,
}

/// Spearman correlation calculator
pub struct SpearmanCorrelation {
    config: CausalityConfig,
    cache: HashMap<u64, SpearmanResult>,
}

impl SpearmanCorrelation {
    /// Create new Spearman calculator
    pub fn new(config: CausalityConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Calculate Spearman correlation with optimal lag
    pub fn calculate_with_lag(&mut self, x: &[f64], y: &[f64], max_lag: usize) -> SpearmanResult {
        let n = x.len().min(y.len());
        if n < self.config.min_sample_size {
            return SpearmanResult {
                rho: 0.0,
                p_value: 1.0,
                is_significant: false,
                optimal_lag: 0,
                confidence_interval: (0.0, 0.0),
            };
        }

        let lagged_corr = self.lagged_correlation(x, y, max_lag as i32);

        SpearmanResult {
            rho: lagged_corr.max_correlation,
            p_value: self.p_value(lagged_corr.max_correlation, n),
            is_significant: lagged_corr.max_correlation.abs() > 0.3,
            optimal_lag: lagged_corr.max_lag,
            confidence_interval: self.bootstrap_ci(x, y, lagged_corr.max_lag),
        }
    }

    /// Calculate Spearman correlation at specific lag
    pub fn calculate_at_lag(&self, x: &[f64], y: &[f64], lag: i32) -> f64 {
        if lag >= 0 {
            let n = x.len().min(y.len() - lag as usize);
            if n < 3 {
                return 0.0;
            }

            let x_trunc: Vec<f64> = x[..n].to_vec();
            let y_trunc: Vec<f64> = y[lag as usize..lag as usize + n].to_vec();

            self.spearman_rho(&x_trunc, &y_trunc)
        } else {
            self.calculate_at_lag(y, x, -lag)
        }
    }

    /// Compute Spearman's rank correlation coefficient
    fn spearman_rho(&self, x: &[f64], y: &[f64]) -> f64 {
        let n = x.len().min(y.len());
        if n < 2 {
            return 0.0;
        }

        // Compute ranks
        let x_ranks = self.rank(x);
        let y_ranks = self.rank(y);

        // Compute differences
        let d_squared: f64 = x_ranks.iter()
            .zip(y_ranks.iter())
            .map(|(rx, ry)| {
                let d = (rx - ry) as f64;
                d * d
            })
            .sum();

        // Spearman's rho = 1 - (6 * Σd²) / (n(n² - 1))
        1.0 - (6.0 * d_squared) / (n as f64 * ((n * n - 1) as f64))
    }

    /// Compute ranks (with tie handling)
    fn rank(&self, data: &[f64]) -> Vec<f64> {
        let n = data.len();
        let mut indexed: Vec<(usize, f64)> = data.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let mut ranks = vec![0.0; n];
        let mut i = 0;

        while i < n {
            let mut j = i;
            while j < n && (indexed[j].1 - indexed[i].1).abs() < 1e-10 {
                j += 1;
            }

            let rank = (i + j - 1) as f64 / 2.0 + 1.0;
            for k in i..j {
                ranks[indexed[k].0] = rank;
            }
            i = j;
        }

        ranks
    }

    /// Compute lagged correlation for multiple lags
    pub fn lagged_correlation(&self, x: &[f64], y: &[f64], max_lag: i32) -> LaggedCorrelation {
        let mut lags = Vec::new();
        let mut correlations = Vec::new();

        for lag in -max_lag..=max_lag {
            let corr = self.calculate_at_lag(x, y, lag);
            lags.push(lag);
            correlations.push(corr);
        }

        let max_idx = correlations.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let min_idx = correlations.iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        LaggedCorrelation {
            lags,
            correlations,
            max_correlation: correlations[max_idx],
            max_lag: lags[max_idx],
            min_correlation: correlations[min_idx],
            min_lag: lags[min_idx],
        }
    }

    /// Compute p-value for Spearman correlation
    fn p_value(&self, rho: f64, n: usize) -> f64 {
        if n < 3 {
            return 1.0;
        }

        // Student's t approximation: t = r * sqrt((n-2)/(1-r²))
        let t = rho * ((n - 2) as f64 / (1.0 - rho * rho + 1e-8)).sqrt();
        let p = 2.0 * (1.0 - self.t_cdf(t, (n - 2) as f64));
        p.min(1.0).max(0.0)
    }

    /// Student's t CDF approximation
    fn t_cdf(&self, t: f64, df: f64) -> f64 {
        let x = (t + (t * t + df).sqrt()) / (2.0 * (t * t + df).sqrt());
        self.beta_inc(df / 2.0, df / 2.0, x)
    }

    /// Bootstrap confidence interval
    fn bootstrap_ci(&self, x: &[f64], y: &[f64], lag: i32, iterations: usize) -> (f64, f64) {
        let n = x.len().min(y.len());
        let mut bootstrap_rhos = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            // Resample with replacement
            let indices: Vec<usize> = (0..n)
                .map(|_| rand::random::<usize>() % n)
                .collect();

            let x_boot: Vec<f64> = indices.iter().map(|&i| x[i]).collect();
            let y_boot: Vec<f64> = indices.iter().map(|&i| y[i]).collect();

            let rho = self.calculate_at_lag(&x_boot, &y_boot, lag);
            bootstrap_rhos.push(rho);
        }

        bootstrap_rhos.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let lower = bootstrap_rhos[iterations / 20];
        let upper = bootstrap_rhos[iterations * 19 / 20];

        (lower, upper)
    }

    fn beta_inc(&self, a: f64, b: f64, x: f64) -> f64 {
        // Simplified incomplete beta - use approximation
        if x <= 0.0 { return 0.0; }
        if x >= 1.0 { return 1.0; }

        let bt = (a * x.ln() + b * (1.0 - x).ln() - (a + b).ln_gamma() + a.ln_gamma() + b.ln_gamma()).exp();

        if x < (a + 1.0) / (a + b + 2.0) {
            bt * self.beta_frac(a, b, x) / a
        } else {
            1.0 - bt * self.beta_frac(b, a, 1.0 - x) / b
        }
    }

    fn beta_frac(&self, a: f64, b: f64, x: f64) -> f64 {
        let mut m = 0.0;
        let mut c = 1.0;
        let mut d = 1.0 / (1.0 - (a + b) * x / (a + 1.0));
        let mut h = d;

        for _ in 0..50 {
            m += 1.0;
            let alpha = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
            d = 1.0 / (1.0 + alpha * d);
            c = 1.0 + alpha / c;
            h *= d * c;
        }

        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spearman_correlation() {
        let config = CausalityConfig::default();
        let mut spearman = SpearmanCorrelation::new(config);

        let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|&v| v * 0.5 + (v * 0.1).sin()).collect();

        let result = spearman.calculate_with_lag(&x, &y, 10);
        println!("Spearman rho: {:.4}, p-value: {:.4}, lag: {}",
                 result.rho, result.p_value, result.optimal_lag);

        assert!(result.rho > 0.9);
    }
}
