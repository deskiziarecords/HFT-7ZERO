// ============================================================
// GAMMA CONTROL SYSTEM (ℒ₅)
// ============================================================
// Position gamma management
// Dynamic hedging signal generation
// Strike-relative scaling
// Feedback control loop
// ============================================================

use super::*;
use crate::market::OrderBook;
use std::collections::VecDeque;

/// Gamma configuration
#[derive(Debug, Clone)]
pub struct GammaConfig {
    pub eta: f64,           // Learning rate
    pub kappa: f64,         // Strike scaling factor
    pub target_gamma: f64,  // Target gamma
    pub gamma_max: f64,     // Maximum gamma
    pub gamma_min: f64,     // Minimum gamma
    pub feedback_window: usize,
}

impl Default for GammaConfig {
    fn default() -> Self {
        Self {
            eta: 0.1,
            kappa: 0.5,
            target_gamma: 0.0,
            gamma_max: 10.0,
            gamma_min: -10.0,
            feedback_window: 100,
        }
    }
}

/// Hedge signal
#[derive(Debug, Clone)]
pub struct HedgeSignal {
    pub delta_hedge: f64,      // Delta hedge amount
    pub gamma_hedge: f64,      // Gamma hedge amount
    pub vega_hedge: f64,       // Vega hedge amount
    pub theta_hedge: f64,      // Theta hedge amount
    pub confidence: f64,
    pub timestamp_ns: u64,
}

/// Gamma controller
pub struct GammaController {
    config: GammaConfig,
    current_gamma: f64,
    gamma_history: VecDeque<f64>,
    feedback_history: VecDeque<f64>,
    last_hedge: Option<HedgeSignal>,
    rng: fastrand::Rng,
}

impl GammaController {
    /// Create new gamma controller
    pub fn new(eta: f64, kappa: f64, target_gamma: f64) -> Self {
        Self {
            config: GammaConfig {
                eta,
                kappa,
                target_gamma,
                ..Default::default()
            },
            current_gamma: 0.0,
            gamma_history: VecDeque::with_capacity(1000),
            feedback_history: VecDeque::with_capacity(1000),
            last_hedge: None,
            rng: fastrand::Rng::new(),
        }
    }

    /// Update gamma with feedback
    /// Γ_{t+1} = 𝒩(Γ_t, κ_strike, Φ_fb)
    pub fn update(&mut self, current_gamma: f64, feedback: f64) -> f64 {
        // Store history
        self.gamma_history.push_back(current_gamma);
        self.feedback_history.push_back(feedback);

        while self.gamma_history.len() > 1000 {
            self.gamma_history.pop_front();
        }
        while self.feedback_history.len() > 1000 {
            self.feedback_history.pop_front();
        }

        // 𝒩(Γ_t, κ_strike, Φ_fb) = Γ_t - η·sign(Γ_t)·Φ_fb·κ
        let sign = if current_gamma > 0.0 { 1.0 } else if current_gamma < 0.0 { -1.0 } else { 0.0 };
        let adjustment = self.config.eta * sign * feedback * self.config.kappa;

        let new_gamma = current_gamma - adjustment;
        let new_gamma = new_gamma.clamp(self.config.gamma_min, self.config.gamma_max);

        self.current_gamma = new_gamma;
        new_gamma
    }

    /// Calculate feedback Φ_fb from market
    pub fn calculate_feedback(&self, book: &OrderBook) -> f64 {
        // Φ_fb = f(Δprice, skew, vol_of_vol)
        let price_change = self.estimate_price_change(book);
        let skew = self.estimate_skew(book);
        let vol_of_vol = self.estimate_vol_of_vol(book);

        // Feedback function
        0.4 * price_change + 0.3 * skew + 0.3 * vol_of_vol
    }

    /// Generate hedge signal
    pub fn generate_hedge(&mut self, book: &OrderBook, gamma: f64) -> HedgeSignal {
        let delta = self.calculate_delta(book, gamma);
        let gamma_hedge = self.calculate_gamma_hedge(gamma);
        let vega = self.calculate_vega(book);
        let theta = self.calculate_theta(book);

        let confidence = self.calculate_confidence();

        let signal = HedgeSignal {
            delta_hedge: delta,
            gamma_hedge,
            vega_hedge: vega,
            theta_hedge: theta,
            confidence,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
        };

        self.last_hedge = Some(signal.clone());
        signal
    }

    /// Calculate delta hedge amount
    fn calculate_delta(&self, book: &OrderBook, gamma: f64) -> f64 {
        // Δ = Γ · (S - K) / σ√τ
        let spot = book.mid_price();
        let strike = spot * 0.99; // Simplified
        let sigma = self.estimate_volatility(book);
        let tau = 1.0 / 365.0; // 1 day

        gamma * (spot - strike) / (sigma * tau.sqrt() + 1e-8)
    }

    /// Calculate gamma hedge amount
    fn calculate_gamma_hedge(&self, gamma: f64) -> f64 {
        // Γ_hedge = -Γ / (1 + κ·|Γ|)
        -gamma / (1.0 + self.config.kappa * gamma.abs())
    }

    /// Calculate vega hedge
    fn calculate_vega(&self, book: &OrderBook) -> f64 {
        // ∂P/∂σ
        let atm_vol = self.estimate_volatility(book);
        let vega = book.mid_price() * atm_vol * 0.01;
        vega.clamp(-1000.0, 1000.0)
    }

    /// Calculate theta hedge
    fn calculate_theta(&self, book: &OrderBook) -> f64 {
        // ∂P/∂t - time decay
        -0.01 * book.mid_price() * self.estimate_volatility(book)
    }

    /// Estimate price change
    fn estimate_price_change(&self, book: &OrderBook) -> f64 {
        let mid = book.mid_price();
        let prev_mid = self.gamma_history.back().map(|&g| g).unwrap_or(mid);
        (mid - prev_mid) / (prev_mid + 1e-8)
    }

    /// Estimate volatility skew
    fn estimate_skew(&self, book: &OrderBook) -> f64 {
        // Put-call skew approximation
        let bid_vol = self.estimate_volatility_from_depth(&book.bids);
        let ask_vol = self.estimate_volatility_from_depth(&book.asks);
        (ask_vol - bid_vol) / (bid_vol + ask_vol + 1e-8)
    }

    /// Estimate volatility of volatility
    fn estimate_vol_of_vol(&self, book: &OrderBook) -> f64 {
        if self.gamma_history.len() < 20 {
            return 0.0;
        }

        let vols: Vec<f64> = self.gamma_history.iter()
            .rev()
            .take(20)
            .map(|&g| g.abs())
            .collect();

        let mean = vols.iter().sum::<f64>() / vols.len() as f64;
        let variance = vols.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / vols.len() as f64;
        variance.sqrt()
    }

    /// Estimate volatility from order book
    fn estimate_volatility(&self, book: &OrderBook) -> f64 {
        // Based on spread and depth
        let spread_vol = book.spread() / book.mid_price();
        let depth_vol = 1.0 / (book.total_bid_volume() + book.total_ask_volume() + 1e-8);
        (spread_vol + depth_vol).min(0.5)
    }

    /// Estimate volatility from depth levels
    fn estimate_volatility_from_depth(&self, levels: &[crate::market::order_book::OrderBookLevel]) -> f64 {
        if levels.is_empty() {
            return 0.1;
        }
        let total_vol: f64 = levels.iter().map(|l| l.volume).sum();
        (1.0 / (total_vol + 1e-8)).min(0.5)
    }

    /// Calculate hedge confidence
    fn calculate_confidence(&self) -> f64 {
        if self.feedback_history.len() < 10 {
            return 0.5;
        }

        // Based on feedback consistency
        let recent: Vec<&f64> = self.feedback_history.iter().rev().take(10).collect();
        let mean = recent.iter().copied().sum::<f64>() / recent.len() as f64;
        let variance = recent.iter().map(|&&v| (v - mean).powi(2)).sum::<f64>() / recent.len() as f64;

        1.0 / (1.0 + variance * 10.0)
    }

    /// Get current gamma
    pub fn current(&self) -> f64 {
        self.current_gamma
    }

    /// Get gamma history
    pub fn history(&self) -> &VecDeque<f64> {
        &self.gamma_history
    }

    /// Get last hedge signal
    pub fn last_hedge(&self) -> Option<&HedgeSignal> {
        self.last_hedge.as_ref()
    }

    /// Reset controller
    pub fn reset(&mut self) {
        self.current_gamma = 0.0;
        self.gamma_history.clear();
        self.feedback_history.clear();
        self.last_hedge = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamma_update() {
        let mut controller = GammaController::new(0.1, 0.5, 0.0);

        let new_gamma = controller.update(1.0, 0.1);
        assert!(new_gamma < 1.0);
    }
}
