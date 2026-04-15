//! EV-ATR Confluence Model for Position Sizing
//! Q_t = f_kelly * g_vol * h_conf * C_max

pub struct EVATRParams {
    pub lambda_frac: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub atr_ref: f64,
    pub beta_vol: f64,
    pub alpha_phi: f64,
    pub phi_min: f64,
    pub frisk: f64,
    pub lmax: f64,
    pub equity: f64,
}

impl Default for EVATRParams {
    fn default() -> Self {
        Self {
            lambda_frac: 3.0,
            avg_win: 0.015,
            avg_loss: 0.005,
            atr_ref: 0.005,
            beta_vol: 0.5,
            alpha_phi: 1.5,
            phi_min: 0.60,
            frisk: 0.01,
            lmax: 50.0,
            equity: 100_000.0,
        }
    }
}

pub struct EVATRModel {
    pub params: EVATRParams,
}

impl EVATRModel {
    pub fn new(params: EVATRParams) -> Self {
        Self { params }
    }

    pub fn compute_q_t(&self, ev_t: f64, atr_t: f64, phi_t: f64) -> f64 {
        let f_kelly = if ev_t <= 0.0 {
            0.0
        } else {
            ev_t / (self.params.lambda_frac * self.params.avg_win * self.params.avg_loss)
        };

        let g_vol = if atr_t <= self.params.atr_ref {
            1.0
        } else {
            (self.params.atr_ref / atr_t).powf(self.params.beta_vol)
        };

        let h_conf = if phi_t <= self.params.phi_min {
            0.0
        } else {
            phi_t.powf(self.params.alpha_phi)
        };

        let c_max = (self.params.frisk * self.params.equity).min(self.params.lmax * self.params.equity);

        f_kelly * g_vol * h_conf * c_max
    }
}
