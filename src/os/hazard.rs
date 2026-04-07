// ============================================================
// HAZARD RATE MODEL (ℒ₂)
// ============================================================
// Real-time hazard rate calculation
// Cancel-execution imbalance tracking
// Regime-dependent dynamics
// ============================================================

use super::*;
use std::collections::VecDeque;

/// Hazard rate configuration
#[derive(Debug, Clone)]
pub struct HazardConfig {
    pub alpha: [f64; 3],
    pub decay: f64,
    pub max_rate: f64,
    pub min_rate: f64,
    pub history_size: usize,
}

impl Default for HazardConfig {
    fn default() -> Self {
        Self {
            alpha: [0.1, 0.5, 0.2],
            decay: 0.99,
            max_rate: 1.0,
            min_rate: 0.0,
            history_size: 1000,
        }
    }
}

/// Hazard event
#[derive(Debug, Clone)]
pub struct HazardEvent {
    pub timestamp_ns: u64,
    pub hazard_rate: f64,
    pub imbalance: f64,
    pub spread: f64,
    pub in_regime: bool,
}

/// Hazard rate calculator
pub struct HazardRate {
    config: HazardConfig,
    current_rate: f64,
    history: VecDeque<HazardEvent>,
    imbalance_history: VecDeque<f64>,
    last_update_ns: u64,
}

impl HazardRate {
    /// Create new hazard rate calculator
    pub fn new(alpha: [f64; 3], decay: f64) -> Self {
        Self {
            config: HazardConfig {
                alpha,
                decay,
                ..Default::default()
            },
            current_rate: 0.0,
            history: VecDeque::with_capacity(1000),
            imbalance_history: VecDeque::with_capacity(100),
            last_update_ns: 0,
        }
    }
    
    /// Update hazard rate
    pub fn update(&mut self, spread: f64, imbalance: f64, dt: f64, in_regime: bool) -> f64 {
        if !in_regime {
            // Decay when not in regime
            self.current_rate *= self.config.decay;
            return self.current_rate;
        }
        
        // f_Δ(δ, ℐ) = α₀δ + α₁·max(0,ℐ) + α₂ℐ²
        let f_delta = self.config.alpha[0] * spread
            + self.config.alpha[1] * imbalance.max(0.0)
            + self.config.alpha[2] * imbalance.powi(2);
        
        // Update rate
        self.current_rate += f_delta * dt;
        self.current_rate = self.current_rate.clamp(self.config.min_rate, self.config.max_rate);
        
        // Record event
        self.history.push_back(HazardEvent {
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            hazard_rate: self.current_rate,
            imbalance,
            spread,
            in_regime,
        });
        
        while self.history.len() > self.config.history_size {
            self.history.pop_front();
        }
        
        self.current_rate
    }
    
    /// Update with automatic dt calculation
    pub fn tick(&mut self, spread: f64, imbalance: f64, in_regime: bool) -> f64 {
        let now = crate::utils::time::get_hardware_timestamp();
        let dt = if self.last_update_ns > 0 {
            (now - self.last_update_ns) as f64 / 1e9
        } else {
            0.001
        };
        
        self.last_update_ns = now;
        self.update(spread, imbalance, dt, in_regime)
    }
    
    /// Calculate cancel-execution imbalance ℐ_ce
    pub fn cancel_exec_imbalance(&mut self, ticks: &[Tick]) -> f64 {
        let mut cancels = 0.0;
        let mut execs = 0.0;
        
        for tick in ticks {
            match tick.tick_type() {
                TickType::Cancel => cancels += tick.volume,
                TickType::Trade => execs += tick.volume,
                _ => {}
            }
        }
        
        self.imbalance_history.push_back((cancels - execs) / (cancels + execs + 1e-8));
        while self.imbalance_history.len() > 100 {
            self.imbalance_history.pop_front();
        }
        
        self.imbalance_history.back().copied().unwrap_or(0.0)
    }
    
    /// Get current hazard rate
    pub fn current(&self) -> f64 {
        self.current_rate
    }
    
    /// Get hazard history
    pub fn history(&self) -> &VecDeque<HazardEvent> {
        &self.history
    }
    
    /// Get average hazard over window
    pub fn avg_hazard(&self, window_seconds: f64) -> f64 {
        let now = crate::utils::time::get_hardware_timestamp();
        let cutoff = now - (window_seconds * 1e9) as u64;
        
        let events: Vec<&HazardEvent> = self.history.iter()
            .filter(|e| e.timestamp_ns >= cutoff)
            .collect();
        
        if events.is_empty() {
            return self.current_rate;
        }
        
        events.iter().map(|e| e.hazard_rate).sum::<f64>() / events.len() as f64
    }
    
    /// Check if hazard is critical
    pub fn is_critical(&self, threshold: f64) -> bool {
        self.current_rate >= threshold
    }
    
    /// Reset hazard rate
    pub fn reset(&mut self) {
        self.current_rate = 0.0;
        self.history.clear();
        self.imbalance_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hazard_update() {
        let mut hazard = HazardRate::new([0.1, 0.5, 0.2], 0.99);
        
        let rate = hazard.update(0.05, 0.3, 0.001, true);
        assert!(rate > 0.0);
        assert!(rate <= 1.0);
    }
    
    #[test]
    fn test_cancel_exec_imbalance() {
        let mut hazard = HazardRate::new([0.1, 0.5, 0.2], 0.99);
        
        let ticks = vec![
            Tick::trade(100.0, 1000.0, 1000, 1, 0),
            Tick::cancel(100.0, 500.0, 1001, 1),
        ];
        
        let imbalance = hazard.cancel_exec_imbalance(&ticks);
        assert!((imbalance + 0.333).abs() < 0.001);
    }
}
