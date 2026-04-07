// ============================================================
// MANDRA GATE (ΔE ≥ 2)
// ============================================================
// Energy-based regime change detection
// ΔE = |E(t) - E(t-1)| where E(t) = -Σ p_i log p_i
// Triggers when energy change exceeds threshold
// ============================================================

use super::*;
use std::collections::VecDeque;

/// Mandra gate state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    Idle,           // Normal operation
    Monitoring,     // Monitoring for transition
    Triggered,      // Energy threshold exceeded
    Recovering,     // Post-trigger recovery
    Locked,         // Manual reset required
}

/// Energy threshold configuration
#[derive(Debug, Clone)]
pub struct EnergyThreshold {
    pub min_delta: f64,        // Minimum ΔE to trigger (default 2.0)
    pub max_delta: f64,        // Maximum ΔE (clamp)
    pub hysteresis: f64,       // Hysteresis to prevent chattering
    pub cooldown_ms: u64,      // Cooldown after trigger
}

impl Default for EnergyThreshold {
    fn default() -> Self {
        Self {
            min_delta: 2.0,
            max_delta: 10.0,
            hysteresis: 0.5,
            cooldown_ms: 1000,
        }
    }
}

/// Mandra gate configuration
#[derive(Debug, Clone)]
pub struct MandraConfig {
    pub threshold: EnergyThreshold,
    pub window_size: usize,
    pub use_entropy: bool,
    pub use_energy: bool,
    pub use_spectral: bool,
}

impl Default for MandraConfig {
    fn default() -> Self {
        Self {
            threshold: EnergyThreshold::default(),
            window_size: 100,
            use_entropy: true,
            use_energy: true,
            use_spectral: false,
        }
    }
}

/// Mandra gate for regime change detection
pub struct MandraGate {
    config: MandraConfig,
    state: GateState,
    energy_history: VecDeque<f64>,
    entropy_history: VecDeque<f64>,
    last_trigger_time: u64,
    trigger_count: u32,
    cooldown_until: u64,
}

impl MandraGate {
    /// Create new Mandra gate
    pub fn new(config: MandraConfig) -> Self {
        Self {
            config,
            state: GateState::Idle,
            energy_history: VecDeque::with_capacity(config.window_size),
            entropy_history: VecDeque::with_capacity(config.window_size),
            last_trigger_time: 0,
            trigger_count: 0,
            cooldown_until: 0,
        }
    }
    
    /// Update gate with new signal distribution
    /// E(t) = -Σ p_i log p_i (Shannon entropy)
    pub fn update(&mut self, distribution: &[f64]) -> GateState {
        let now = crate::utils::time::get_hardware_timestamp();
        
        // Check cooldown
        if now < self.cooldown_until {
            self.state = GateState::Recovering;
            return self.state;
        }
        
        // Compute energy (entropy) of distribution
        let energy = self.compute_energy(distribution);
        let entropy = self.compute_entropy(distribution);
        
        // Store history
        self.energy_history.push_back(energy);
        self.entropy_history.push_back(entropy);
        
        while self.energy_history.len() > self.config.window_size {
            self.energy_history.pop_front();
            self.entropy_history.pop_front();
        }
        
        // Compute delta energy ΔE
        let delta_energy = if self.energy_history.len() >= 2 {
            (energy - self.energy_history[self.energy_history.len() - 2]).abs()
        } else {
            0.0
        };
        
        let delta_entropy = if self.entropy_history.len() >= 2 {
            (entropy - self.entropy_history[self.entropy_history.len() - 2]).abs()
        } else {
            0.0
        };
        
        // Combine metrics
        let delta = if self.config.use_entropy && self.config.use_energy {
            (delta_energy + delta_entropy) / 2.0
        } else if self.config.use_entropy {
            delta_entropy
        } else {
            delta_energy
        };
        
        // Check trigger condition: ΔE ≥ threshold
        let should_trigger = delta >= self.config.threshold.min_delta;
        
        // Apply hysteresis
        let was_triggered = self.state == GateState::Triggered;
        let final_trigger = if was_triggered {
            // Need to drop below threshold - hysteresis to prevent chattering
            delta > self.config.threshold.min_delta - self.config.threshold.hysteresis
        } else {
            should_trigger
        };
        
        // Update state
        self.state = if final_trigger {
            self.trigger_count += 1;
            self.last_trigger_time = now;
            self.cooldown_until = now + self.config.threshold.cooldown_ms * 1_000_000;
            GateState::Triggered
        } else if self.state == GateState::Triggered {
            GateState::Recovering
        } else {
            GateState::Idle
        };
        
        self.state
    }
    
    /// Compute energy from distribution: E = Σ p_i² (energy, not entropy)
    fn compute_energy(&self, distribution: &[f64]) -> f64 {
        distribution.iter().map(|&p| p * p).sum::<f64>()
    }
    
    /// Compute Shannon entropy: H = -Σ p_i log p_i
    fn compute_entropy(&self, distribution: &[f64]) -> f64 {
        let mut entropy = 0.0;
        for &p in distribution {
            if p > 0.0 {
                entropy -= p * p.ln();
            }
        }
        entropy
    }
    
    /// Update with spectral features (alternative input)
    pub fn update_spectral(&mut self, features: &SpectralFeatures) -> GateState {
        // Use spectral features to compute energy change
        let spectral_energy = features.spectral_centroid * features.dominant_magnitude;
        
        self.energy_history.push_back(spectral_energy);
        while self.energy_history.len() > self.config.window_size {
            self.energy_history.pop_front();
        }
        
        let delta = if self.energy_history.len() >= 2 {
            (spectral_energy - self.energy_history[self.energy_history.len() - 2]).abs()
        } else {
            0.0
        };
        
        if delta >= self.config.threshold.min_delta {
            self.state = GateState::Triggered;
            self.trigger_count += 1;
            self.last_trigger_time = crate::utils::time::get_hardware_timestamp();
        } else {
            self.state = GateState::Idle;
        }
        
        self.state
    }
    
    /// Update with real-time price stream
    pub fn update_price_stream(&mut self, prices: &[f64]) -> GateState {
        if prices.len() < 10 {
            return self.state;
        }
        
        // Compute return distribution
        let returns: Vec<f64> = prices.windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        
        // Create histogram (20 bins)
        let mut hist = vec![0.0; 20];
        let min_ret = returns.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_ret = returns.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_ret - min_ret;
        
        if range > 1e-8 {
            for &ret in &returns {
                let bin = ((ret - min_ret) / range * 19.0) as usize;
                let idx = bin.min(19);
                hist[idx] += 1.0;
            }
            
            let total = returns.len() as f64;
            for h in hist.iter_mut() {
                *h /= total;
            }
        }
        
        self.update(&hist)
    }
    
    /// Check if gate is triggered
    pub fn is_triggered(&self) -> bool {
        self.state == GateState::Triggered
    }
    
    /// Get current gate state
    pub fn state(&self) -> GateState {
        self.state
    }
    
    /// Get trigger statistics
    pub fn stats(&self) -> MandraStats {
        MandraStats {
            trigger_count: self.trigger_count,
            last_trigger_time_ns: self.last_trigger_time,
            current_state: self.state,
            current_energy: self.energy_history.back().copied().unwrap_or(0.0),
            current_entropy: self.entropy_history.back().copied().unwrap_or(0.0),
            window_size: self.energy_history.len(),
        }
    }
    
    /// Reset gate
    pub fn reset(&mut self) {
        self.state = GateState::Idle;
        self.energy_history.clear();
        self.entropy_history.clear();
        self.cooldown_until = 0;
    }
    
    /// Force trigger
    pub fn force_trigger(&mut self) {
        self.state = GateState::Triggered;
        self.last_trigger_time = crate::utils::time::get_hardware_timestamp();
        self.trigger_count += 1;
    }
}

/// Mandra gate statistics
#[derive(Debug, Clone)]
pub struct MandraStats {
    pub trigger_count: u32,
    pub last_trigger_time_ns: u64,
    pub current_state: GateState,
    pub current_energy: f64,
    pub current_entropy: f64,
    pub window_size: usize,
}

/// Energy-based market regime detector
pub struct EnergyRegimeDetector {
    mandra_gate: MandraGate,
    regime_history: VecDeque<GateState>,
    transition_threshold: usize,
}

impl EnergyRegimeDetector {
    pub fn new(config: MandraConfig, transition_threshold: usize) -> Self {
        Self {
            mandra_gate: MandraGate::new(config),
            regime_history: VecDeque::with_capacity(100),
            transition_threshold,
        }
    }
    
    pub fn update(&mut self, distribution: &[f64]) -> bool {
        let state = self.mandra_gate.update(distribution);
        self.regime_history.push_back(state);
        
        while self.regime_history.len() > 100 {
            self.regime_history.pop_front();
        }
        
        // Detect regime transition if multiple triggers in window
        let recent_triggers: usize = self.regime_history.iter()
            .filter(|&&s| s == GateState::Triggered)
            .count();
        
        recent_triggers >= self.transition_threshold
    }
    
    pub fn mandra_gate(&self) -> &MandraGate {
        &self.mandra_gate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mandra_gate() {
        let config = MandraConfig::default();
        let mut gate = MandraGate::new(config);
        
        // Stable distribution (low entropy)
        let stable = vec![0.8, 0.1, 0.05, 0.03, 0.02];
        let state1 = gate.update(&stable);
        assert_eq!(state1, GateState::Idle);
        
        // Drastic change (high entropy)
        let changed = vec![0.2, 0.2, 0.2, 0.2, 0.2];
        let state2 = gate.update(&changed);
        
        // Should trigger after change
        assert!(state2 == GateState::Triggered || gate.is_triggered());
        
        let stats = gate.stats();
        println!("Mandra gate stats: {:?}", stats);
    }
    
    #[test]
    fn test_price_stream_detection() {
        let config = MandraConfig::default();
        let mut gate = MandraGate::new(config);
        
        // Stable prices
        let stable_prices: Vec<f64> = (0..100).map(|_| 100.0).collect();
        let state1 = gate.update_price_stream(&stable_prices);
        
        // Sudden price jump
        let mut volatile_prices: Vec<f64> = (0..50).map(|_| 100.0).collect();
        volatile_prices.extend((0..50).map(|i| 100.0 + i as f64 * 0.1));
        
        let state2 = gate.update_price_stream(&volatile_prices);
        
        println!("Stable state: {:?}, Volatile state: {:?}", state1, state2);
    }
}
