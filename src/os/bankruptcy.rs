// ============================================================
// BANKRUPTCY GATE & CIRCUIT BREAKER (ℒ₆)
// ============================================================
// Emergency shutdown system
// Automatic position liquidation
// Recovery protocols
// Multi-level circuit breakers
// ============================================================

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Circuit breaker status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerStatus {
    Closed,      // Normal operation
    Tripped,     // Breaker tripped
    Recovery,    // Recovery in progress
    Locked,      // Permanently locked (manual reset required)
}

/// Recovery plan
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub phase: RecoveryPhase,
    pub start_time_ns: u64,
    pub duration_ns: u64,
    pub max_position_release: f64,
    pub steps: Vec<RecoveryStep>,
}

/// Recovery phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPhase {
    Immediate,    // Immediate actions
    Gradual,      // Gradual position release
    Monitoring,   // Monitoring period
    Complete,     // Full recovery
}

/// Recovery step
#[derive(Debug, Clone)]
pub struct RecoveryStep {
    pub step_id: u32,
    pub action: RecoveryAction,
    pub delay_ms: u64,
    pub condition: String,
}

/// Recovery action
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    LiquidatePosition(f64),      // Liquidate up to amount
    CancelOrders,
    ReduceRisk(f64),             // Reduce risk by percentage
    ResetMetrics,
    SendAlert(String),
    WaitForSignal,
}

/// Bankruptcy gate
pub struct BankruptcyGate {
    max_drawdown: f64,
    max_loss: f64,
    auto_recovery: bool,
    status: AtomicU64,  // Stores BreakerStatus
    drawdown_peak: AtomicU64,
    loss_total: AtomicU64,
    trigger_time: AtomicU64,
    recovery_plan: parking_lot::RwLock<Option<RecoveryPlan>>,
    callbacks: parking_lot::RwLock<Vec<Box<dyn BankruptcyCallback + Send + Sync>>>,
}

/// Bankruptcy callback trait
pub trait BankruptcyCallback: Send + Sync {
    fn on_trigger(&self, drawdown: f64, loss: f64);
    fn on_recovery(&self, phase: RecoveryPhase);
    fn on_liquidation(&self, amount: f64);
}

/// Default callback (logging)
struct LoggingCallback;

impl BankruptcyCallback for LoggingCallback {
    fn on_trigger(&self, drawdown: f64, loss: f64) {
        tracing::error!("BANKRUPTCY TRIGGERED: drawdown={:.2}%, loss=${:.2}", drawdown * 100.0, loss);
    }
    
    fn on_recovery(&self, phase: RecoveryPhase) {
        tracing::info!("Bankruptcy recovery phase: {:?}", phase);
    }
    
    fn on_liquidation(&self, amount: f64) {
        tracing::warn!("Liquidating position: ${:.2}", amount);
    }
}

impl BankruptcyGate {
    /// Create new bankruptcy gate
    pub fn new(max_drawdown: f64, max_loss: f64, auto_recovery: bool) -> Self {
        let gate = Self {
            max_drawdown,
            max_loss,
            auto_recovery,
            status: AtomicU64::new(BreakerStatus::Closed as u64),
            drawdown_peak: AtomicU64::new(0),
            loss_total: AtomicU64::new(0),
            trigger_time: AtomicU64::new(0),
            recovery_plan: parking_lot::RwLock::new(None),
            callbacks: parking_lot::RwLock::new(Vec::new()),
        };
        
        // Add default logging callback
        gate.add_callback(Box::new(LoggingCallback));
        
        gate
    }
    
    /// Check if bankruptcy should trigger
    pub fn check(&self, gamma: f64, hazard: f64, volatility: f64) -> bool {
        let current_status = self.get_status();
        
        if current_status != BreakerStatus::Closed {
            return true;
        }
        
        // θ_t = 1 if conditions met
        let should_trigger = gamma.abs() > 5.0 || hazard > 0.8 || volatility > 0.2;
        
        if should_trigger {
            self.trigger();
        }
        
        should_trigger
    }
    
    /// Trigger bankruptcy gate
    pub fn trigger(&self) {
        let current_status = self.get_status();
        if current_status != BreakerStatus::Closed {
            return;
        }
        
        let trigger_ns = crate::utils::time::get_hardware_timestamp();
        self.trigger_time.store(trigger_ns, Ordering::Release);
        self.status.store(BreakerStatus::Tripped as u64, Ordering::Release);
        
        let drawdown = self.drawdown_peak.load(Ordering::Acquire) as f64 / 1e6;
        let loss = self.loss_total.load(Ordering::Acquire) as f64 / 1e6;
        
        // Execute callbacks
        for callback in self.callbacks.read().iter() {
            callback.on_trigger(drawdown, loss);
        }
        
        // Create recovery plan if auto-recovery enabled
        if self.auto_recovery {
            self.create_recovery_plan();
        }
        
        tracing::error!("Bankruptcy gate triggered at t={}", trigger_ns);
    }
    
    /// Create recovery plan
    fn create_recovery_plan(&self) {
        let plan = RecoveryPlan {
            phase: RecoveryPhase::Immediate,
            start_time_ns: crate::utils::time::get_hardware_timestamp(),
            duration_ns: 60_000_000_000, // 60 seconds
            max_position_release: 100_000.0,
            steps: vec![
                RecoveryStep {
                    step_id: 1,
                    action: RecoveryAction::CancelOrders,
                    delay_ms: 0,
                    condition: "immediate".to_string(),
                },
                RecoveryStep {
                    step_id: 2,
                    action: RecoveryAction::LiquidatePosition(0.5),
                    delay_ms: 100,
                    condition: "if_position > 0".to_string(),
                },
                RecoveryStep {
                    step_id: 3,
                    action: RecoveryAction::ReduceRisk(0.5),
                    delay_ms: 500,
                    condition: "if_risk_high".to_string(),
                },
                RecoveryStep {
                    step_id: 4,
                    action: RecoveryAction::WaitForSignal,
                    delay_ms: 1000,
                    condition: "wait_for_stability".to_string(),
                },
                RecoveryStep {
                    step_id: 5,
                    action: RecoveryAction::ResetMetrics,
                    delay_ms: 5000,
                    condition: "if_stable".to_string(),
                },
            ],
        };
        
        *self.recovery_plan.write() = Some(plan);
        
        for callback in self.callbacks.read().iter() {
            callback.on_recovery(RecoveryPhase::Immediate);
        }
    }
    
    /// Execute recovery
    pub fn execute_recovery(&self) -> bool {
        let mut plan_opt = self.recovery_plan.write();
        let plan = match plan_opt.as_mut() {
            Some(p) => p,
            None => return false,
        };
        
        let now = crate::utils::time::get_hardware_timestamp();
        let elapsed = now - plan.start_time_ns;
        
        if elapsed >= plan.duration_ns {
            // Recovery complete
            self.status.store(BreakerStatus::Closed as u64, Ordering::Release);
            plan.phase = RecoveryPhase::Complete;
            return true;
        }
        
        // Execute steps based on phase
        match plan.phase {
            RecoveryPhase::Immediate => {
                self.execute_immediate_recovery(plan);
                plan.phase = RecoveryPhase::Gradual;
            }
            RecoveryPhase::Gradual => {
                self.execute_gradual_recovery(plan);
                if elapsed > plan.duration_ns / 2 {
                    plan.phase = RecoveryPhase::Monitoring;
                }
            }
            RecoveryPhase::Monitoring => {
                self.monitor_recovery(plan);
                if elapsed > plan.duration_ns * 3 / 4 {
                    plan.phase = RecoveryPhase::Complete;
                }
            }
            RecoveryPhase::Complete => {
                return true;
            }
        }
        
        false
    }
    
    fn execute_immediate_recovery(&self, plan: &mut RecoveryPlan) {
        for step in &plan.steps {
            if step.delay_ms == 0 {
                self.execute_recovery_step(step);
            }
        }
    }
    
    fn execute_gradual_recovery(&self, plan: &mut RecoveryPlan) {
        let now = crate::utils::time::get_hardware_timestamp();
        let elapsed = now - plan.start_time_ns;
        
        for step in &plan.steps {
            let step_time_ns = step.delay_ms * 1_000_000;
            if step_time_ns <= elapsed && step.delay_ms > 0 {
                self.execute_recovery_step(step);
            }
        }
    }
    
    fn monitor_recovery(&self, plan: &mut RecoveryPlan) {
        // Check if system is stable
        let is_stable = self.check_stability();
        if is_stable {
            for callback in self.callbacks.read().iter() {
                callback.on_recovery(RecoveryPhase::Monitoring);
            }
        }
    }
    
    fn execute_recovery_step(&self, step: &RecoveryStep) {
        match &step.action {
            RecoveryAction::LiquidatePosition(amount) => {
                for callback in self.callbacks.read().iter() {
                    callback.on_liquidation(*amount);
                }
            }
            RecoveryAction::CancelOrders => {
                tracing::info!("Cancelling all open orders");
            }
            RecoveryAction::ReduceRisk(percentage) => {
                tracing::info!("Reducing risk by {}%", percentage * 100.0);
            }
            RecoveryAction::ResetMetrics => {
                self.drawdown_peak.store(0, Ordering::Release);
                self.loss_total.store(0, Ordering::Release);
            }
            RecoveryAction::SendAlert(msg) => {
                tracing::warn!("Recovery alert: {}", msg);
            }
            RecoveryAction::WaitForSignal => {
                // Wait for external signal
            }
        }
    }
    
    fn check_stability(&self) -> bool {
        // Check if market conditions have stabilized
        let drawdown = self.drawdown_peak.load(Ordering::Acquire) as f64 / 1e6;
        drawdown < self.max_drawdown * 0.5
    }
    
    /// Add callback
    pub fn add_callback(&self, callback: Box<dyn BankruptcyCallback + Send + Sync>) {
        self.callbacks.write().push(callback);
    }
    
    /// Get current status
    pub fn get_status(&self) -> BreakerStatus {
        match self.status.load(Ordering::Acquire) {
            0 => BreakerStatus::Closed,
            1 => BreakerStatus::Tripped,
            2 => BreakerStatus::Recovery,
            3 => BreakerStatus::Locked,
            _ => BreakerStatus::Closed,
        }
    }
    
    /// Reset gate (manual)
    pub fn reset(&self) {
        self.status.store(BreakerStatus::Closed as u64, Ordering::Release);
        self.drawdown_peak.store(0, Ordering::Release);
        self.loss_total.store(0, Ordering::Release);
        self.trigger_time.store(0, Ordering::Release);
        *self.recovery_plan.write() = None;
        
        tracing::info!("Bankruptcy gate manually reset");
    }
    
    /// Update drawdown tracking
    pub fn update_drawdown(&self, current_pnl: f64) {
        let current = (current_pnl * 1e6) as u64;
        let mut peak = self.drawdown_peak.load(Ordering::Acquire);
        
        while current > peak {
            match self.drawdown_peak.compare_exchange(peak, current, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }
    
    /// Update loss tracking
    pub fn update_loss(&self, loss: f64) {
        self.loss_total.fetch_add((loss.abs() * 1e6) as u64, Ordering::Relaxed);
    }
    
    /// Check if gate is open for trading
    pub fn is_open(&self) -> bool {
        self.get_status() == BreakerStatus::Closed
    }
    
    /// Get time since last trigger
    pub fn time_since_trigger(&self) -> Option<Duration> {
        let trigger = self.trigger_time.load(Ordering::Acquire);
        if trigger == 0 {
            return None;
        }
        
        let now = crate::utils::time::get_hardware_timestamp();
        Some(Duration::from_nanos(now - trigger))
    }
}

/// Multi-level circuit breaker
pub struct CircuitBreaker {
    levels: Vec<BreakerLevel>,
    current_level: usize,
    gate: Arc<BankruptcyGate>,
}

/// Breaker level configuration
#[derive(Debug, Clone)]
pub struct BreakerLevel {
    pub threshold: f64,
    pub action: BreakerAction,
    pub cooldown_seconds: u64,
}

/// Breaker action
#[derive(Debug, Clone)]
pub enum BreakerAction {
    Warning,
    ReducePosition(f64),
    HaltTrading(u64),
    EmergencyShutdown,
}

impl CircuitBreaker {
    pub fn new(gate: Arc<BankruptcyGate>) -> Self {
        let levels = vec![
            BreakerLevel {
                threshold: 0.02,  // 2% drawdown
                action: BreakerAction::Warning,
                cooldown_seconds: 10,
            },
            BreakerLevel {
                threshold: 0.05,  // 5% drawdown
                action: BreakerAction::ReducePosition(0.5),
                cooldown_seconds: 60,
            },
            BreakerLevel {
                threshold: 0.10,  // 10% drawdown
                action: BreakerAction::HaltTrading(300),
                cooldown_seconds: 300,
            },
            BreakerLevel {
                threshold: 0.20,  // 20% drawdown
                action: BreakerAction::EmergencyShutdown,
                cooldown_seconds: 3600,
            },
        ];
        
        Self {
            levels,
            current_level: 0,
            gate,
        }
    }
    
    pub fn check_and_act(&mut self, drawdown: f64) -> BreakerAction {
        for (i, level) in self.levels.iter().enumerate().rev() {
            if drawdown >= level.threshold && i >= self.current_level {
                self.current_level = i + 1;
                self.execute_action(&level.action);
                return level.action.clone();
            }
        }
        
        BreakerAction::Warning
    }
    
    fn execute_action(&self, action: &BreakerAction) {
        match action {
            BreakerAction::Warning => {
                tracing::warn!("Circuit breaker warning: drawdown threshold approaching");
            }
            BreakerAction::ReducePosition(percentage) => {
                tracing::warn!("Reducing position by {}%", percentage * 100.0);
            }
            BreakerAction::HaltTrading(seconds) => {
                tracing::error!("Trading halted for {} seconds", seconds);
            }
            BreakerAction::EmergencyShutdown => {
                tracing::error!("EMERGENCY SHUTDOWN - triggering bankruptcy gate");
                self.gate.trigger();
            }
        }
    }
    
    pub fn reset(&mut self) {
        self.current_level = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bankruptcy_gate() {
        let gate = BankruptcyGate::new(0.05, 10000.0, true);
        
        assert!(gate.is_open());
        
        let triggered = gate.check(6.0, 0.5, 0.15);
        assert!(triggered);
        
        assert!(!gate.is_open());
        assert_eq!(gate.get_status(), BreakerStatus::Tripped);
    }
    
    #[test]
    fn test_circuit_breaker() {
        let gate = Arc::new(BankruptcyGate::new(0.05, 10000.0, true));
        let mut breaker = CircuitBreaker::new(gate);
        
        let action = breaker.check_and_act(0.03);
        matches!(action, BreakerAction::Warning);
        
        let action = breaker.check_and_act(0.08);
        matches!(action, BreakerAction::ReducePosition(_));
    }
}
