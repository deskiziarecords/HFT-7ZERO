// ============================================================
// INTERRUPT HANDLER (Layer 3)
// ============================================================
// Manages macro news volatility injections
// States: IDL (Idle), PND (Pending), ACT (Active), DEC (Decay), OVR (Overridden)
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Macro event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum MacroSeverity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Macro event structure
#[derive(Debug, Clone)]
pub struct MacroEvent {
    pub id: u64,
    pub source: String,
    pub severity: MacroSeverity,
    pub timestamp_ns: u64,
    pub confirmed: bool,
    pub decay_factor: f64,
}

/// Interrupt state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptState {
    Idle,       // IDL - No active news events
    Pending,    // PND - Event detected, awaiting confirmation
    Active,     // ACT - Event impacting market
    Decay,      // DEC - Impact fading
    Overridden, // OVR - Layer 6 override active
}

impl InterruptState {
    /// Convert to single character for state vector encoding
    pub fn as_char(&self) -> char {
        match self {
            InterruptState::Idle => 'I',
            InterruptState::Pending => 'P',
            InterruptState::Active => 'A',
            InterruptState::Decay => 'D',
            InterruptState::Overridden => 'O',
        }
    }
    
    /// Convert from character (for decoding)
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'I' => Some(InterruptState::Idle),
            'P' => Some(InterruptState::Pending),
            'A' => Some(InterruptState::Active),
            'D' => Some(InterruptState::Decay),
            'O' => Some(InterruptState::Overridden),
            _ => None,
        }
    }
}

/// Configuration for interrupt handler
#[derive(Debug, Clone)]
pub struct InterruptConfig {
    pub confirmation_window_ns: u64,    // Time to wait for confirmation (default: 5s)
    pub active_duration_ns: u64,        // How long event stays active (default: 60s)
    pub decay_rate: f64,                // Exponential decay factor (default: 0.95)
    pub min_decay_threshold: f64,       // When to return to Idle (default: 0.01)
    pub max_concurrent_events: usize,   // Max events to track (default: 10)
}

impl Default for InterruptConfig {
    fn default() -> Self {
        Self {
            confirmation_window_ns: 5_000_000_000,      // 5 seconds
            active_duration_ns: 60_000_000_000,         // 60 seconds
            decay_rate: 0.95,                           // 5% decay per step
            min_decay_threshold: 0.01,                  // 1% threshold
            max_concurrent_events: 10,
        }
    }
}

/// Main interrupt handler
pub struct InterruptHandler {
    config: InterruptConfig,
    state: InterruptState,
    pending_events: VecDeque<MacroEvent>,
    active_events: VecDeque<MacroEvent>,
    current_severity: MacroSeverity,
    active_until_ns: u64,
    decay_factor: f64,
    last_update_ns: u64,
    override_active: bool,
}

impl InterruptHandler {
    /// Create new interrupt handler
    pub fn new(config: InterruptConfig) -> Self {
        Self {
            config,
            state: InterruptState::Idle,
            pending_events: VecDeque::with_capacity(config.max_concurrent_events),
            active_events: VecDeque::with_capacity(config.max_concurrent_events),
            current_severity: MacroSeverity::Low,
            active_until_ns: 0,
            decay_factor: 1.0,
            last_update_ns: 0,
            override_active: false,
        }
    }
    
    /// Update interrupt state with new macro events
    pub fn update(&mut self, macro_events: &[MacroEvent], now_ns: u64) -> InterruptState {
        // Handle Layer 6 override
        if self.override_active {
            self.state = InterruptState::Overridden;
            return self.state;
        }
        
        // Clean up old events
        self.cleanup_old_events(now_ns);
        
        // Add new events to pending
        for event in macro_events {
            if !event.confirmed {
                self.pending_events.push_back(event.clone());
            } else {
                self.active_events.push_back(event.clone());
            }
        }
        
        // Limit queue sizes
        while self.pending_events.len() > self.config.max_concurrent_events {
            self.pending_events.pop_front();
        }
        while self.active_events.len() > self.config.max_concurrent_events {
            self.active_events.pop_front();
        }
        
        // State transition logic
        match self.state {
            InterruptState::Idle => {
                if !self.pending_events.is_empty() {
                    self.state = InterruptState::Pending;
                    self.update_severity();
                } else if !self.active_events.is_empty() {
                    self.state = InterruptState::Active;
                    self.active_until_ns = now_ns + self.config.active_duration_ns;
                    self.update_severity();
                }
            }
            
            InterruptState::Pending => {
                // Check if any pending events have been confirmed
                let confirmed_count = self.pending_events.iter()
                    .filter(|e| e.confirmed)
                    .count();
                
                if confirmed_count > 0 {
                    // Move confirmed events to active
                    let confirmed: Vec<MacroEvent> = self.pending_events.iter()
                        .filter(|e| e.confirmed)
                        .cloned()
                        .collect();
                    
                    for event in confirmed {
                        self.active_events.push_back(event);
                    }
                    
                    // Remove confirmed from pending
                    self.pending_events.retain(|e| !e.confirmed);
                    
                    self.state = InterruptState::Active;
                    self.active_until_ns = now_ns + self.config.active_duration_ns;
                    self.update_severity();
                } else if self.pending_events.is_empty() {
                    // No pending events left
                    self.state = InterruptState::Idle;
                }
            }
            
            InterruptState::Active => {
                if now_ns > self.active_until_ns {
                    self.state = InterruptState::Decay;
                    self.decay_factor = 1.0;
                } else {
                    // Update severity based on active events
                    self.update_severity();
                }
            }
            
            InterruptState::Decay => {
                self.decay_factor *= self.config.decay_rate;
                
                if self.decay_factor < self.config.min_decay_threshold {
                    self.state = InterruptState::Idle;
                    self.decay_factor = 1.0;
                    self.active_events.clear();
                    self.current_severity = MacroSeverity::Low;
                }
            }
            
            InterruptState::Overridden => {
                // Wait for override to clear
                if !self.override_active {
                    self.state = InterruptState::Idle;
                }
            }
        }
        
        self.last_update_ns = now_ns;
        self.state
    }
    
    /// Update current severity based on active events
    fn update_severity(&mut self) {
        let max_severity = self.active_events.iter()
            .chain(self.pending_events.iter())
            .map(|e| e.severity)
            .max()
            .unwrap_or(MacroSeverity::Low);
        
        self.current_severity = max_severity;
    }
    
    /// Remove events older than their expiry
    fn cleanup_old_events(&mut self, now_ns: u64) {
        // Clean pending events (expire after confirmation window)
        self.pending_events.retain(|e| {
            now_ns - e.timestamp_ns < self.config.confirmation_window_ns
        });
        
        // Clean active events (expire after active duration + decay)
        self.active_events.retain(|e| {
            now_ns - e.timestamp_ns < self.config.active_duration_ns * 2
        });
    }
    
    /// Get current interrupt state
    pub fn state(&self) -> InterruptState {
        self.state
    }
    
    /// Get current severity (for other layers)
    pub fn current_severity(&self) -> MacroSeverity {
        self.current_severity
    }
    
    /// Get impact multiplier (for volatility injection)
    pub fn impact_multiplier(&self) -> f64 {
        match self.state {
            InterruptState::Idle => 0.0,
            InterruptState::Pending => 0.3,
            InterruptState::Active => 1.0,
            InterruptState::Decay => self.decay_factor,
            InterruptState::Overridden => 0.0,
        }
    }
    
    /// Set override from Layer 6
    pub fn set_override(&mut self, active: bool) {
        self.override_active = active;
        if active {
            self.state = InterruptState::Overridden;
        }
    }
    
    /// Check if override is active
    pub fn is_overridden(&self) -> bool {
        self.override_active
    }
    
    /// Reset handler
    pub fn reset(&mut self) {
        self.state = InterruptState::Idle;
        self.pending_events.clear();
        self.active_events.clear();
        self.current_severity = MacroSeverity::Low;
        self.active_until_ns = 0;
        self.decay_factor = 1.0;
        self.override_active = false;
    }
    
    /// Get statistics
    pub fn stats(&self) -> InterruptStats {
        InterruptStats {
            state: self.state,
            pending_count: self.pending_events.len(),
            active_count: self.active_events.len(),
            current_severity: self.current_severity,
            impact_multiplier: self.impact_multiplier(),
            override_active: self.override_active,
        }
    }
}

/// Interrupt handler statistics
#[derive(Debug, Clone)]
pub struct InterruptStats {
    pub state: InterruptState,
    pub pending_count: usize,
    pub active_count: usize,
    pub current_severity: MacroSeverity,
    pub impact_multiplier: f64,
    pub override_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_interrupt_state_machine() {
        let config = InterruptConfig::default();
        let mut handler = InterruptHandler::new(config);
        
        let now = crate::utils::time::get_hardware_timestamp();
        
        // Initially idle
        assert_eq!(handler.state(), InterruptState::Idle);
        
        // Add pending event
        let event = MacroEvent {
            id: 1,
            source: "FOMC".to_string(),
            severity: MacroSeverity::High,
            timestamp_ns: now,
            confirmed: false,
            decay_factor: 1.0,
        };
        
        handler.update(&[event], now);
        assert_eq!(handler.state(), InterruptState::Pending);
        
        // Confirm event (simulate by adding confirmed version)
        let confirmed_event = MacroEvent {
            confirmed: true,
            ..event
        };
        
        handler.update(&[confirmed_event], now + 1_000_000_000);
        assert_eq!(handler.state(), InterruptState::Active);
        
        // Wait for decay
        let future = now + 65_000_000_000;
        handler.update(&[], future);
        assert_eq!(handler.state(), InterruptState::Decay);
        
        // Complete decay
        let far_future = now + 200_000_000_000;
        handler.update(&[], far_future);
        assert_eq!(handler.state(), InterruptState::Idle);
    }
    
    #[test]
    fn test_state_encoding() {
        assert_eq!(InterruptState::Idle.as_char(), 'I');
        assert_eq!(InterruptState::Pending.as_char(), 'P');
        assert_eq!(InterruptState::Active.as_char(), 'A');
        assert_eq!(InterruptState::Decay.as_char(), 'D');
        assert_eq!(InterruptState::Overridden.as_char(), 'O');
        
        assert_eq!(InterruptState::from_char('I'), Some(InterruptState::Idle));
        assert_eq!(InterruptState::from_char('X'), None);
    }
}
