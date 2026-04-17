// ============================================================
// GRANGER CAUSALITY (𝒢_VAR(p))
// ============================================================
// Vector Autoregression based causality testing
// F-test for nested models
// Bootstrap for significance
// ============================================================

use super::*;
use ndarray::{Array1, Array2, s};
use ndarray_linalg::{Inverse, LeastSquares};
use std::f64;

/// Granger causality test results
#[derive(Debug, Clone)]
pub struct GrangerResult {
    pub f_statistic: f64,
    pub p_value: f64,
    pub is_causal: bool,
    pub optimal_lag: usize,
    pub resid_var_full: f64,
    pub resid_var_reduced: f64,
    pub dof1: usize,
    pub dof2: usize,
}

/// VAR Model for Granger causality
pub struct VARModel {
    lags: usize,
    coefficients: Array2<f64>,
    residuals: Array2<f64>,
}

impl VARModel {
    /// Fit VAR model to data
    pub fn fit(y: &[f64], x: &[f64], lags: usize) -> Result<Self, String> {
        let n = y.len().min(x.len());
        if n <= lags + 5 {
            return Err("Insufficient samples".to_string());
        }

        // Build design matrix: [y_{t-1}, y_{t-2}, ..., x_{t-1}, x_{t-2}, ...]
        let num_predictors = 2 * lags;
        let num_obs = n - lags;

        let mut design = Array2::zeros((num_obs, num_predictors + 1)); // +1 for intercept
        let mut target = Array1::zeros(num_obs);

        for t in lags..n {
            let row = t - lags;
            target[row] = y[t];

            // Add intercept
            design[[row, 0]] = 1.0;

            // Add lagged values
            for lag in 0..lags {
                design[[row, 1 + lag]] = y[t - lag - 1];
                design[[row, 1 + lags + lag]] = x[t - lag - 1];
            }
        }

        // Solve using least squares: (X'X)^{-1} X'y
        let xt = design.t();
        let xtx = xt.dot(&design);
        let xty = xt.dot(&target);

        let coefficients = match xtx.inv() {
            Ok(inv) => inv.dot(&xty),
            Err(_) => {
                // Use pseudo-inverse if singular
                let (u, s, vt) = xtx.svd(true, true).map_err(|e| format!("SVD failed: {}", e))?;
                let s_inv = Array2::from_diag(&Array1::from(
                    s.iter().map(|&si| if si > 1e-10 { 1.0 / si } else { 0.0 }).collect::<Vec<_>>()
                ));
                let pseudo_inv = vt.t().dot(&s_inv).dot(&u.t());
                pseudo_inv.dot(&xty)
            }
        };

        // Calculate residuals
        let fitted = design.dot(&coefficients);
        let residuals = &target - &fitted;

        Ok(Self {
            lags,
            coefficients,
            residuals: residuals.insert_axis(ndarray::Axis(1)),
        })
    }

    /// Get residual variance
    pub fn residual_variance(&self) -> f64 {
        let ssr = self.residuals.iter().map(|&r| r * r).sum::<f64>();
        ssr / self.residuals.nrows() as f64
    }

    /// Predict next value
    pub fn predict(&self, y_history: &[f64], x_history: &[f64]) -> f64 {
        let mut prediction = self.coefficients[[0, 0]]; // intercept

        for lag in 0..self.lags {
            if lag < y_history.len() {
                prediction += self.coefficients[[0, 1 + lag]] * y_history[y_history.len() - lag - 1];
            }
            if lag < x_history.len() {
                prediction += self.coefficients[[0, 1 + self.lags + lag]] * x_history[x_history.len() - lag - 1];
            }
        }

        prediction
    }
}

/// Granger causality tester
pub struct GrangerCausality {
    config: CausalityConfig,
    cache_enabled: bool,
}

impl GrangerCausality {
    /// Create new Granger causality tester
    pub fn new(config: CausalityConfig) -> Self {
        Self {
            config,
            cache_enabled: true,
        }
    }

    /// Test if X Granger-causes Y
    pub fn test(&mut self, y: &[f64], x: &[f64], lags: usize) -> GrangerResult {
        // Check cache
        let cache_key = CausalityCache::hash_key(y, x, lags);
        if self.cache_enabled {
            if let Some(cached) = CAUSALITY_CACHE.get(cache_key) {
                return GrangerResult {
                    f_statistic: cached.granger_score,
                    p_value: cached.granger_pvalue,
                    is_causal: cached.is_significant,
                    optimal_lag: lags,
                    resid_var_full: 0.0,
                    resid_var_reduced: 0.0,
                    dof1: lags,
                    dof2: y.len() - 2 * lags - 1,
                };
            }
        }

        // Fit full model (with X)
        let full_model = match VARModel::fit(y, x, lags) {
            Ok(m) => m,
            Err(_) => {
                return GrangerResult {
                    f_statistic: 0.0,
                    p_value: 1.0,
                    is_causal: false,
                    optimal_lag: lags,
                    resid_var_full: f64::INFINITY,
                    resid_var_reduced: f64::INFINITY,
                    dof1: lags,
                    dof2: 0,
                };
            }
        };

        // Fit reduced model (without X)
        let reduced_model = match VARModel::fit(y, &vec![0.0; y.len()], lags) {
            Ok(m) => m,
            Err(_) => {
                return GrangerResult {
                    f_statistic: 0.0,
                    p_value: 1.0,
                    is_causal: false,
                    optimal_lag: lags,
                    resid_var_full: full_model.residual_variance(),
                    resid_var_reduced: f64::INFINITY,
                    dof1: lags,
                    dof2: 0,
                };
            }
        };

        let rss_full = full_model.residual_variance() * (y.len() - lags) as f64;
        let rss_reduced = reduced_model.residual_variance() * (y.len() - lags) as f64;
        let dof1 = lags;
        let dof2 = y.len() - 2 * lags - 1;

        // F-statistic: ((RSS_reduced - RSS_full) / dof1) / (RSS_full / dof2)
        let f_stat = ((rss_reduced - rss_full) / dof1 as f64) / (rss_full / dof2 as f64);

        // Compute p-value using F-distribution
        let p_value = self.f_cdf(f_stat, dof1 as f64, dof2 as f64);

        let result = GrangerResult {
            f_statistic: f_stat,
            p_value,
            is_causal: p_value < self.config.significance_level,
            optimal_lag: lags,
            resid_var_full: full_model.residual_variance(),
            resid_var_reduced: reduced_model.residual_variance(),
            dof1,
            dof2,
        };

        // Cache result
        if self.cache_enabled {
            let cached_result = CausalityResult {
                granger_score: result.f_statistic,
                granger_pvalue: result.p_value,
                transfer_entropy: 0.0,
                te_std_error: 0.0,
                ccm_score: 0.0,
                ccm_rho: 0.0,
                spearman_rho: 0.0,
                optimal_lag: lags,
                is_significant: result.is_causal,
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                method_agreement: 0.0,
            };
            CAUSALITY_CACHE.insert(cache_key, cached_result);
        }

        result
    }

    /// Find optimal lag using AIC/BIC
    pub fn find_optimal_lag(&mut self, y: &[f64], x: &[f64], max_lag: usize) -> usize {
        let mut best_lag = 1;
        let mut best_aic = f64::INFINITY;

        for lag in 1..=max_lag.min(y.len() / 4) {
            let model = match VARModel::fit(y, x, lag) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let n = y.len() - lag;
            let k = 2 * lag + 1; // number of parameters
            let aic = n as f64 * model.residual_variance().ln() + 2.0 * k as f64;

            if aic < best_aic {
                best_aic = aic;
                best_lag = lag;
            }
        }

        best_lag
    }

    /// F-distribution CDF approximation
    fn f_cdf(&self, x: f64, df1: f64, df2: f64) -> f64 {
        // Beta function approximation for F-distribution
        if x <= 0.0 {
            return 0.0;
        }

        let f_val = x * df1 / df2;
        let p = self.beta_inc(df1 / 2.0, df2 / 2.0, f_val / (1.0 + f_val));
        1.0 - p
    }

    /// Incomplete beta function approximation
    fn beta_inc(&self, a: f64, b: f64, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        // Continued fraction approximation
        let mut bt = (a * x.ln() + b * (1.0 - x).ln() - (a + b).ln_gamma() + a.ln_gamma() + b.ln_gamma()).exp();

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

        for _ in 0..100 {
            m += 1.0;
            let alpha = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
            d = 1.0 / (1.0 + alpha * d);
            c = 1.0 + alpha / c;
            h *= d * c;
        }

        h
    }

    /// Bootstrap significance test
    pub fn bootstrap_significance(&mut self, y: &[f64], x: &[f64], lags: usize, iterations: usize) -> f64 {
        let original_f = self.test(y, x, lags).f_statistic;
        let mut greater_count = 0;

        for _ in 0..iterations {
            // Bootstrap by shuffling X
            let mut shuffled_x: Vec<f64> = x.to_vec();
            self.shuffle(&mut shuffled_x);

            let boot_f = self.test(y, &shuffled_x, lags).f_statistic;
            if boot_f >= original_f {
                greater_count += 1;
            }
        }

        greater_count as f64 / iterations as f64
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
    fn test_granger_causality() {
        // Generate causal relationship: y depends on x with lag 1
        let n = 500;
        let mut x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).sin()).collect();
        let mut y: Vec<f64> = x.iter().map(|&v| v * 0.5).collect();

        // Add some noise
        for i in 1..n {
            y[i] += 0.1 * (i as f64).sin();
        }

        let config = CausalityConfig::default();
        let mut granger = GrangerCausality::new(config);

        let result = granger.test(&y, &x, 2);

        // Should detect causality
        println!("F-stat: {}, p-value: {}", result.f_statistic, result.p_value);
    }
}
