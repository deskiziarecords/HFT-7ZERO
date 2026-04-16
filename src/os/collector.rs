// ============================================================
// COLLECTOR HANDLER (Layer 4)
// ============================================================
// Executes sweeps (liquidity harvesting)
// States: IDL (Idle), TGT (Targeting), SWP (Sweeping), CMP (Complete), RUN (Runaway), VET (Vetoed)
// ============================================================

use super::*;
use std::collections::VecDeque;

/// Collector state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorState {
    Idle,       // IDL - No active sweep
    Targeting,  // TGT - Sweep target identified
    Sweeping,   // SWP - Actively sweeping
    Complete,   // CMP - Sweep finished successfully
    Runaway,    // RUN - Gamma squeeze, sweep converted to runaway
    Vetoed,     // VET - λ6 blocked the sweep
}

impl CollectorState {
    /// Convert to single character for state vector encoding
    pub fn as_char(&self) -> char {
        match self {
            CollectorState::Idle => 'I',
            CollectorState::Targeting => 'T',
            CollectorState::Sweeping => 'S',
            CollectorState::Complete => 'C',
            CollectorState::Runaway => 'R',
            CollectorState::Vetoed => 'V',
        }
    }
    
    /// Convert from character
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'I' => Some(CollectorState::Idle),
            'T' => Some(CollectorState::Targeting),
            'S' => Some(CollectorState::Sweeping),
            'C' => Some(CollectorState::Complete),
            'R' => Some(CollectorState::Runaway),
            'V' => Some(CollectorState::Vetoed),
            _ => None,
        }
    }
    
    /// Check if active (not idle or complete)
    pub fn is_active(&self) -> bool {
        matches!(self, CollectorState::Targeting | CollectorState::Sweeping | CollectorState::Runaway)
    }
}

/// Sweep target
#[derive(Debug, Clone)]
pub struct SweepTarget {
    pub price: f64,
    pub density: f64,
    pub direction: SweepDirection,
    pub timestamp_ns: u64,
}

/// Sweep direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepDirection {
    Up,     // Sweep above current price
    Down,   // Sweep below current price
}

/// Collector configuration
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub min_target_density: f64,       // Minimum density to trigger targeting (default: 0.6)
    pub sweep_completion_threshold: f64, // Price return threshold for completion (default: 0.3 ATR)
    pub runaway_threshold: f64,         // Gamma level for runaway (default: 2.0)
    pub max_sweep_duration_ns: u64,     // Max sweep duration before timeout (default: 30s)
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            min_target_density: 0.6,
            sweep_completion_threshold: 0.3,
            runaway_threshold: 2.0,
            max_sweep_duration_ns: 30_000_000_000, // 30 seconds
        }
    }
}

/// Main collector handler
pub struct CollectorHandler {
    config: CollectorConfig,
    state: CollectorState,
    current_target: Option<SweepTarget>,
    sweep_start_ns: u64,
    sweep_entry_price: f64,
    gamma_level: f64,
    lambda6_veto: bool,
    completed_targets: VecDeque<SweepTarget>,
}

impl CollectorHandler {
    /// Create new collector handler
    pub fn new(config: CollectorConfig) -> Self {
        Self {
            config,
            state: CollectorState::Idle,
            current_target: None,
            sweep_start_ns: 0,
            sweep_entry_price: 0.0,
            gamma_level: 0.0,
            lambda6_veto: false,
            completed_targets: VecDeque::with_capacity(100),
        }
    }
    
    /// Update collector state
    pub fn update(
        &mut self,
        targets: &[SweepTarget],
        current_price: f64,
        atr: f64,
        gamma: f64,
        lambda6_blocked: bool,
        now_ns: u64,
    ) -> CollectorState {
        // Handle λ6 veto (highest priority)
        if lambda6_blocked {
            self.state = CollectorState::Vetoed;
            self.lambda6_veto = true;
            return self.state;
        }
        
        // Handle gamma squeeze (runaway)
        if gamma >= self.config.runaway_threshold && self.state.is_active() {
            self.state = CollectorState::Runaway;
            return self.state;
        }
        
        // State transition logic
        match self.state {
            CollectorState::Idle => {
                // Check for new targets
                let best_target = targets.iter()
                    .max_by(|a, b| a.density.partial_cmp(&b.density).unwrap())
                    .filter(|t| t.density >= self.config.min_target_density);
                
                if let Some(target) = best_target {
                    self.state = CollectorState::Targeting;
                    self.current_target = Some(target.clone());
                    self.gamma_level = 0.0;
                }
            }
            
            CollectorState::Targeting => {
                // Transition to sweeping when price approaches target
                if let Some(target) = &self.current_target {
                    let distance = match target.direction {
                        SweepDirection::Up => (target.price - current_price).abs(),
                        SweepDirection::Down => (current_price - target.price).abs(),
                    };
                    
                    if distance < atr * 0.5 {
                        self.state = CollectorState::Sweeping;
                        self.sweep_start_ns = now_ns;
                        self.sweep_entry_price = current_price;
                    }
                }
                
                // Check for better target
                let better_target = targets.iter()
                    .filter(|t| t.density > self.current_target.as_ref().map(|ct| ct.density).unwrap_or(0.0))
                    .max_by(|a, b| a.density.partial_cmp(&b.density).unwrap());
                
                if let Some(target) = better_target {
                    self.current_target = Some(target.clone());
                }
            }
            
            CollectorState::Sweeping => {
                // Check for completion
                let price_movement = match self.current_target.as_ref().map(|t| t.direction) {
                    Some(SweepDirection::Up) => current_price - self.sweep_entry_price,
                    Some(SweepDirection::Down) => self.sweep_entry_price - current_price,
                    None => 0.0,
                };
                
                let completion_threshold = atr * self.config.sweep_completion_threshold;
                
                if price_movement >= completion_threshold {
                    // Sweep successful
                    self.state = CollectorState::Complete;
                    if let Some(target) = self.current_target.clone() {
                        self.completed_targets.push_back(target);
                        while self.completed_targets.len() > 100 {
                            self.completed_targets.pop_front();
                        }
                    }
                } else if now_ns - self.sweep_start_ns > self.config.max_sweep_duration_ns {
                    // Timeout - revert to idle
                    self.state = CollectorState::Idle;
                    self.current_target = None;
                }
            }
            
            CollectorState::Complete => {
                // Reset after completion
                self.state = CollectorState::Idle;
                self.current_target = None;
                self.gamma_level = 0.0;
            }
            
            CollectorState::Runaway => {
                // Runaway active - wait for gamma to subside
                if gamma < self.config.runaway_threshold * 0.5 {
                    self.state = CollectorState::Idle;
                    self.current_target = None;
                }
            }
            
            CollectorState::Vetoed => {
                // Wait for λ6 to clear
                if !lambda6_blocked {
                    self.state = CollectorState::Idle;
                    self.lambda6_veto = false;
                }
            }
        }
        
        self.gamma_level = gamma;
        self.state
    }
    
    /// Update gamma level (from Layer 5)
    pub fn update_gamma(&mut self, gamma: f64) {
        self.gamma_level = gamma;
    }
    
    /// Get current state
    pub fn state(&self) -> CollectorState {
        self.state
    }
    
    /// Get current target
    pub fn current_target(&self) -> Option<&SweepTarget> {
        self.current_target.as_ref()
    }
    
    /// Check if λ6 veto is active
    pub fn is_vetoed(&self) -> bool {
        self.lambda6_veto
    }
    
    /// Get sweep progress (0.0 to 1.0)
    pub fn sweep_progress(&self, current_price: f64, atr: f64) -> f64 {
        if let Some(target) = &self.current_target {
            let target_price = target.price;
            let entry = self.sweep_entry_price;
            let threshold = atr * self.config.sweep_completion_threshold;
            
            let movement = match target.direction {
                SweepDirection::Up => current_price - entry,
                SweepDirection::Down => entry - current_price,
            };
            
            (movement / threshold).min(1.0).max(0.0)
        } else {
            0.0
        }
    }
    
    /// Get completed targets (for analytics)
    pub fn completed_targets(&self) -> &VecDeque<SweepTarget> {
        &self.completed_targets
    }
    
    /// Reset collector
    pub fn reset(&mut self) {
        self.state = CollectorState::Idle;
        self.current_target = None;
        self.sweep_start_ns = 0;
        self.sweep_entry_price = 0.0;
        self.gamma_level = 0.0;
        self.lambda6_veto = false;
        self.completed_targets.clear();
    }
    
    /// Get statistics
    pub fn stats(&self) -> CollectorStats {
        CollectorStats {
            state: self.state,
            has_target: self.current_target.is_some(),
            target_density: self.current_target.as_ref().map(|t| t.density).unwrap_or(0.0),
            gamma_level: self.gamma_level,
            vetoed: self.lambda6_veto,
            completed_count: self.completed_targets.len(),
            sweep_duration_ns: if self.sweep_start_ns > 0 {
                crate::utils::time::get_hardware_timestamp() - self.sweep_start_ns
            } else {
                0
            },
        }
    }
}

/// Collector statistics
#[derive(Debug, Clone)]
pub struct CollectorStats {
    pub state: CollectorState,
    pub has_target: bool,
    pub target_density: f64,
    pub gamma_level: f64,
    pub vetoed: bool,
    pub completed_count: usize,
    pub sweep_duration_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collector_state_machine() {
        let config = CollectorConfig::default();
        let mut collector = CollectorHandler::new(config);
        
        let now = crate::utils::time::get_hardware_timestamp();
        
        // Initially idle
        assert_eq!(collector.state(), CollectorState::Idle);
        
        // Add target
        let targets = vec![SweepTarget {
            price: 101.0,
            density: 0.8,
            direction: SweepDirection::Up,
            timestamp_ns: now,
        }];
        
        collector.update(&targets, 100.0, 0.5, 0.0, false, now);
        assert_eq!(collector.state(), CollectorState::Targeting);
        
        // Approach target -> sweep
        collector.update(&targets, 100.8, 0.5, 0.0, false, now + 1_000_000_000);
        assert_eq!(collector.state(), CollectorState::Sweeping);
        
        // Complete sweep
        collector.update(&targets, 101.2, 0.5, 0.0, false, now + 2_000_000_000);
        assert_eq!(collector.state(), CollectorState::Complete);
        
        // Auto-reset
        collector.update(&targets, 101.2, 0.5, 0.0, false, now + 3_000_000_000);
        assert_eq!(collector.state(), CollectorState::Idle);
    }
    
    #[test]
    fn test_runaway_detection() {
        let config = CollectorConfig::default();
        let mut collector = CollectorHandler::new(config);
        
        let now = crate::utils::time::get_hardware_timestamp();
        
        let targets = vec![SweepTarget {
            price: 101.0,
            density: 0.8,
            direction: SweepDirection::Up,
            timestamp_ns: now,
        }];
        
        collector.update(&targets, 100.0, 0.5, 0.0, false, now);
        assert_eq!(collector.state(), CollectorState::Targeting);
        
        // High gamma triggers runaway
        collector.update(&targets, 100.5, 0.5, 2.5, false, now + 1_000_000_000);
        assert_eq!(collector.state(), CollectorState::Runaway);
        
        // Gamma subsides
        collector.update(&targets, 100.5, 0.5, 0.8, false, now + 2_000_000_000);
        assert_eq!(collector.state(), CollectorState::Idle);
    }
    
    #[test]
    fn test_lambda6_veto() {
        let config = CollectorConfig::default();
        let mut collector = CollectorHandler::new(config);
        
        let now = crate::utils::time::get_hardware_timestamp();
        
        let targets = vec![SweepTarget {
            price: 101.0,
            density: 0.8,
            direction: SweepDirection::Up,
            timestamp_ns: now,
        }];
        
        collector.update(&targets, 100.0, 0.5, 0.0, true, now);
        assert_eq!(collector.state(), CollectorState::Vetoed);
        
        // λ6 clears
        collector.update(&targets, 100.0, 0.5, 0.0, false, now + 1_000_000_000);
        assert_eq!(collector.state(), CollectorState::Idle);
    }
    
    #[test]
    fn test_state_encoding() {
        assert_eq!(CollectorState::Idle.as_char(), 'I');
        assert_eq!(CollectorState::Targeting.as_char(), 'T');
        assert_eq!(CollectorState::Sweeping.as_char(), 'S');
        assert_eq!(CollectorState::Complete.as_char(), 'C');
        assert_eq!(CollectorState::Runaway.as_char(), 'R');
        assert_eq!(CollectorState::Vetoed.as_char(), 'V');
        
        assert_eq!(CollectorState::from_char('C'), Some(CollectorState::Complete));
        assert_eq!(CollectorState::from_char('X'), None);
    }
}
