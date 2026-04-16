use std::sync::atomic::{AtomicU64, Ordering};



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerStatus { Closed = 0, Open = 1, Tripped = 2 }

pub struct BankruptcyGate {
    pub max_drawdown: f64,
    pub max_loss: f64,
    pub auto_recovery: bool,
    pub status: AtomicU64,
}

impl BankruptcyGate {
    pub fn new(max_drawdown: f64, max_loss: f64, auto_recovery: bool) -> Self {
        Self {
            max_drawdown, max_loss, auto_recovery,
            status: AtomicU64::new(BreakerStatus::Closed as u64),
        }
    }
    pub fn check(&self, _gamma: f64, _hazard: f64, _vol: f64) -> bool {
        self.status.load(Ordering::Acquire) != BreakerStatus::Closed as u64
    }
    pub fn reset(&self) {
        self.status.store(BreakerStatus::Closed as u64, Ordering::Release);
    }
}

pub struct CircuitBreaker;
pub struct RecoveryPlan;
