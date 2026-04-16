// ============================================================
// PRIVILEGED HANDLER (Layer 6)
// ============================================================
// Central Bank overrides and emergency circuit breakers
// States: NOR (Normal), WAT (Watching), ACT (Active), LCK (Locked), REC (Recovery)
// ============================================================

use super::*;
use std::time::{Duration, Instant};

/// Privileged state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegedState {
    Normal,    // NOR - Normal operation
    Watching,  // WAT - Pre-override monitoring
    Active,    // ACT - Override active
    Locked,    // LCK - Permanently locked (manual reset)
    Recovery,  // REC - Recovery in progress
}

impl PrivilegedState {
    /// Convert to single character for state vector encoding
    pub fn as_char(&self) -> char {
        match self {
            PrivilegedState::Normal => 'N',
            PrivilegedState::Watching => 'W',
            PrivilegedState::Active => 'A',
            PrivilegedState::Locked => 'L',
            PrivilegedState::Recovery => 'R',
        }
    }
    
    /// Convert from character
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'N' => Some(PrivilegedState::Normal),
            'W' => Some(PrivilegedState::Watching),
            'A' => Some(PrivilegedState::Active),
            'L' => Some(PrivilegedState::Locked),
            'R' => Some(PrivilegedState::Recovery),
            _ => None,
        }
    }
    
    /// Check if trading is allowed
    pub fn can_trade(&self) -> bool {
        matches!(self, PrivilegedState::Normal | PrivilegedState::Watching)
    }
    
    /// Check if override is active
    pub fn is_overridden(&self) -> bool {
        matches!(self, PrivilegedState::Active | PrivilegedState::Locked)
    }
}

/// Override source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideSource {
    CentralBank,    // Central bank intervention
    CircuitBreaker, // Automatic circuit breaker
    Manual,         // Manual operator override
    RiskGate,       // Risk gate triggered
    Bankruptcy,     // Bankruptcy gate triggered
}

/// Override configuration
#[derive(Debug, Clone)]
pub struct PrivilegedConfig {
    pub watching_duration_ns: u64,      // How long to watch before active (default: 10s)
    pub recovery_duration_ns: u64,      // Recovery period after override (default: 60s)
    pub auto_reset: bool,               // Auto-reset from Locked (default: false)
    pub max_consecutive_overrides: u32, // Max overrides before permanent lock (default: 3)
}

impl Default for PrivilegedConfig {
    fn default() -> Self {
        Self {
            watching_duration_ns: 10_000_000_000,   // 10 seconds
            recovery_duration_ns: 60_000_000_000,   // 60 seconds
            auto_reset: false,
            max_consecutive_overrides: 3,
        }
    }
}

/// Main privileged handler
pub struct PrivilegedHandler {
    config: PrivilegedConfig,
    state: PrivilegedState,
    current_override: Option<OverrideSource>,
    override_start_ns: u64,
    override_count: u32,
    recovery_until_ns: u64,
    watching_until_ns: u64,
}

impl PrivilegedHandler {
    /// Create new privileged handler
    pub fn new(config: PrivilegedConfig) -> Self {
        Self {
            config,
            state: PrivilegedState::Normal,
            current_override: None,
            override_start_ns: 0,
            override_count: 0,
            recovery_until_ns: 0,
            watching_until_ns: 0,
        }
    }
    
    /// Update privileged state
    pub fn update(&mut self, trigger: Option<OverrideSource>, now_ns: u64) -> PrivilegedState {
        match self.state {
            PrivilegedState::Normal => {
                if let Some(source) = trigger {
                    // Enter watching state before activating
                    self.state = PrivilegedState::Watching;
                    self.watching_until_ns = now_ns + self.config.watching_duration_ns;
                    self.current_override = Some(source);
                }
            }
            
            PrivilegedState::Watching => {
                if now_ns >= self.watching_until_ns {
                    // Watching period complete - activate override
                    self.state = PrivilegedState::Active;
                    self.override_start_ns = now_ns;
                    self.override_count += 1;
                    
                    tracing::warn!(
                        "🔴 LAYER 6 OVERRIDE ACTIVE: {:?} (count: {})",
                        self.current_override, self.override_count
                    );
                } else if trigger.is_none() {
                    // Trigger cleared during watching - cancel
                    self.state = PrivilegedState::Normal;
                    self.current_override = None;
                }
            }
            
            PrivilegedState::Active => {
                if trigger.is_none() {
                    // Trigger cleared - begin recovery
                    self.state = PrivilegedState::Recovery;
                    self.recovery_until_ns = now_ns + self.config.recovery_duration_ns;
                    
                    tracing::info!("🔵 LAYER 6 RECOVERY STARTED");
                }
            }
            
            PrivilegedState::Recovery => {
                if now_ns >= self.recovery_until_ns {
                    // Recovery complete
                    self.state = PrivilegedState::Normal;
                    self.current_override = None;
                    
                    tracing::info!("✅ LAYER 6 RECOVERY COMPLETE");
                }
                
                // Check if trigger re-asserted during recovery
                if trigger.is_some() {
                    self.state = PrivilegedState::Active;
                    self.override_start_ns = now_ns;
                }
            }
            
            PrivilegedState::Locked => {
                // Locked - only manual reset works
                // State persists across updates
            }
        }
        
        // Check for lock condition (too many overrides)
        if self.override_count >= self.config.max_consecutive_overrides && !self.config.auto_reset {
            self.state = PrivilegedState::Locked;
            tracing::error!("🔒 LAYER 6 PERMANENTLY LOCKED ({} overrides)", self.override_count);
        }
        
        self.state
    }
    
    /// Manual reset (only works from Locked state)
    pub fn manual_reset(&mut self) -> bool {
        if self.state == PrivilegedState::Locked {
            self.state = PrivilegedState::Normal;
            self.override_count = 0;
            self.current_override = None;
            tracing::info!("🔓 LAYER 6 MANUAL RESET");
            true
        } else {
            false
        }
    }
    
    /// Force override (emergency use)
    pub fn force_override(&mut self, source: OverrideSource) {
        self.state = PrivilegedState::Active;
        self.current_override = Some(source);
        self.override_start_ns = crate::utils::time::get_hardware_timestamp();
        self.override_count += 1;
        
        tracing::error!("🚨 LAYER 6 FORCE OVERRIDE: {:?}", source);
    }
    
    /// Get current state
    pub fn state(&self) -> PrivilegedState {
        self.state
    }
    
    /// Get current override source
    pub fn current_override(&self) -> Option<OverrideSource> {
        self.current_override
    }
    
    /// Check if trading is allowed
    pub fn can_trade(&self) -> bool {
        self.state.can_trade()
    }
    
    /// Get override duration so far
    pub fn override_duration_ns(&self) -> u64 {
        if self.state == PrivilegedState::Active && self.override_start_ns > 0 {
            crate::utils::time::get_hardware_timestamp() - self.override_start_ns
        } else {
            0
        }
    }
    
    /// Get recovery remaining
    pub fn recovery_remaining_ns(&self) -> u64 {
        if self.state == PrivilegedState::Recovery && self.recovery_until_ns > 0 {
            let now = crate::utils::time::get_hardware_timestamp();
            if now < self.recovery_until_ns {
                self.recovery_until_ns - now
            } else {
                0
            }
        } else {
            0
        }
    }
    
    /// Reset handler
    pub fn reset(&mut self) {
        self.state = PrivilegedState::Normal;
        self.current_override = None;
        self.override_start_ns = 0;
        self.override_count = 0;
        self.recovery_until_ns = 0;
        self.watching_until_ns = 0;
    }
    
    /// Get statistics
    pub fn stats(&self) -> PrivilegedStats {
        PrivilegedStats {
            state: self.state,
            override_source: self.current_override,
            override_count: self.override_count,
            override_duration_ns: self.override_duration_ns(),
            recovery_remaining_ns: self.recovery_remaining_ns(),
            can_trade: self.can_trade(),
        }
    }
}

/// Privileged statistics
#[derive(Debug, Clone)]
pub struct PrivilegedStats {
    pub state: PrivilegedState,
    pub override_source: Option<OverrideSource>,
    pub override_count: u32,
    pub override_duration_ns: u64,
    pub recovery_remaining_ns: u64,
    pub can_trade: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_privileged_state_machine() {
        let config = PrivilegedConfig::default();
        let mut handler = PrivilegedHandler::new(config);
        
        let now = crate::utils::time::get_hardware_timestamp();
        
        // Initially normal
        assert_eq!(handler.state(), PrivilegedState::Normal);
        assert!(handler.can_trade());
        
        // Trigger override
        handler.update(Some(OverrideSource::CentralBank), now);
        assert_eq!(handler.state(), PrivilegedState::Watching);
        
        // After watching period
        handler.update(Some(OverrideSource::CentralBank), now + 15_000_000_000);
        assert_eq!(handler.state(), PrivilegedState::Active);
        assert!(!handler.can_trade());
        
        // Clear trigger - enter recovery
        handler.update(None, now + 16_000_000_000);
        assert_eq!(handler.state(), PrivilegedState::Recovery);
        
        // Recovery completes
        handler.update(None, now + 80_000_000_000);
        assert_eq!(handler.state(), PrivilegedState::Normal);
    }
    
    #[test]
    fn test_lock_condition() {
        let config = PrivilegedConfig {
            max_consecutive_overrides: 2,
            auto_reset: false,
            ..Default::default()
        };
        
        let mut handler = PrivilegedHandler::new(config);
        let now = crate::utils::time::get_hardware_timestamp();
        
        // First override
        handler.update(Some(OverrideSource::CentralBank), now);
        handler.update(Some(OverrideSource::CentralBank), now + 15_000_000_000);
        handler.update(None, now + 20_000_000_000);
        handler.update(None, now + 80_000_000_000);
        assert_eq!(handler.state(), PrivilegedState::Normal);
        
        // Second override
        handler.update(Some(OverrideSource::CentralBank), now + 100_000_000_000);
        handler.update(Some(OverrideSource::CentralBank), now + 115_000_000_000);
        handler.update(None, now + 120_000_000_000);
        handler.update(None, now + 180_000_000_000);
        
        // Should be locked after 2 overrides
        assert_eq!(handler.state(), PrivilegedState::Locked);
        
        // Manual reset
        assert!(handler.manual_reset());
        assert_eq!(handler.state(), PrivilegedState::Normal);
    }
    
    #[test]
    fn test_state_encoding() {
        assert_eq!(PrivilegedState::Normal.as_char(), 'N');
        assert_eq!(PrivilegedState::Watching.as_char(), 'W');
        assert_eq!(PrivilegedState::Active.as_char(), 'A');
        assert_eq!(PrivilegedState::Locked.as_char(), 'L');
        assert_eq!(PrivilegedState::Recovery.as_char(), 'R');
        
        assert_eq!(PrivilegedState::from_char('W'), Some(PrivilegedState::Watching));
        assert_eq!(PrivilegedState::from_char('X'), None);
    }
}
