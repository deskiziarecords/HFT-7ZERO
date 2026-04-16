//! Schur Routing Engine with Adelic Validation
use nalgebra::{DMatrix, DVector};

#[derive(Debug, Clone, Copy)]
pub struct Venue {
    pub id: u32,
    pub latency_ms: f64,
    pub fees: f64,
}

pub struct RoutingParams {
    pub slippage_gamma: Vec<f64>,
    pub slippage_delta: Vec<f64>,
    pub correlation_decay: f64,
    pub adelic_rho: f64,
    pub adelic_max_nonzero: usize,
    pub blowup_kappa: f64,
}

pub struct RoutingResult {
    pub weights: Vec<f64>,
    pub quantities: Vec<f64>,
    pub cost_estimate: f64,
    pub adelic_valid: bool,
    pub blowup_detected: bool,
}

pub struct SchurRouter {
    pub venues: Vec<Venue>,
    pub params: RoutingParams,
}

impl SchurRouter {
    pub fn new(venues: Vec<Venue>, params: RoutingParams) -> Self {
        Self { venues, params }
    }

    pub fn optimize(
        &self,
        q_total: f64,
        ofi_matrix: &DMatrix<f64>,
        prev_weights: &DVector<f64>,
    ) -> Option<RoutingResult> {
        let n = self.venues.len();
        if n == 0 {
            return None;
        }

        let mut c = DMatrix::zeros(n, n);

        for i in 0..n {
            let q_i = q_total * prev_weights[i];
            let s_i = self.params.slippage_gamma[i] * q_i.powf(self.params.slippage_delta[i]);
            c[(i, i)] = s_i;
        }

        for i in 0..n {
            for j in (i + 1)..n {
                let d_ij = (self.venues[i].latency_ms - self.venues[j].latency_ms).abs();
                let rho_ij = (-self.params.correlation_decay * d_ij).exp();
                let c_ij = rho_ij * ofi_matrix[(i, j)] * q_total;
                c[(i, j)] = c_ij;
                c[(j, i)] = c_ij;
            }
        }

        let eig = c.clone().symmetric_eigen();
        let eigenvals = eig.eigenvalues;
        let eigenvecs = eig.eigenvectors;

        let mut adelic_valid = true;
        let mut nonzero_count = 0;
        for &val in eigenvals.iter() {
            if val.abs() > self.params.adelic_rho {
                adelic_valid = false;
                break;
            }
            if val.abs() > 1e-6 {
                nonzero_count += 1;
            }
        }
        if nonzero_count > self.params.adelic_max_nonzero {
            adelic_valid = false;
        }

        let atr_proxy = self.venues.iter().map(|v| v.fees).sum::<f64>() / n as f64;
        let blowup_detected = eigenvals.max() > atr_proxy * self.params.blowup_kappa;

        let mut k_star = 0;
        let mut min_val = f64::MAX;
        for i in 0..n {
            if eigenvals[i] < min_val {
                min_val = eigenvals[i];
                k_star = i;
            }
        }

        let v_opt = eigenvecs.column(k_star);
        let mut weights: Vec<f64> = v_opt.iter().map(|&x| x.abs()).collect();
        let w_sum: f64 = weights.iter().sum::<f64>();
        weights.iter_mut().for_each(|w| *w /= w_sum + 1e-8);

        let quantities: Vec<f64> = weights.iter().map(|&w| w * q_total).collect();
        let w_vec = DVector::from_vec(weights.clone());
        let cost_estimate = (w_vec.transpose() * (&c * &w_vec))[(0, 0)];

        Some(RoutingResult {
            weights,
            quantities,
            cost_estimate,
            adelic_valid,
            blowup_detected,
        })
    }
}
