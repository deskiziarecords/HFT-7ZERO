// ============================================================
// RISK GATE
// ============================================================
// 6-layer risk gate system
// Hardware-accelerated checks
// Sub-microsecond decision latency
// ============================================================

use super::*;
use crate::signal::harmonic_detector::HarmonicTrapDetector;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;

/// Gate status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStatus {
    Open,       // Trading allowed
    Closed,     // Trading blocked
    Partial,    // Partial restrictions
    Emergency,  // Emergency shutdown
}

/// Gate decision with reason
#[derive(Debug, Clone)]
pub struct GateDecision {
    pub status: GateStatus,
    pub triggered_gates: Vec<TriggerType>,
    pub reason: String,
    pub timestamp_ns: u64,
}

/// Main risk gate (6 layers)
pub struct RiskGate {
    // Layer 1: Volatility regime gate
    volatility_regime: AtomicU64,
    time_in_regime: AtomicU64,
    
    // Layer 2: Kurtosis/drift gate
    kurtosis_threshold: f64,
    drift_threshold: f64,
    
    // Layer 3: Harmonic trap gate
    harmonic_detector: HarmonicTrapDetector,
    
    // Layer 4: Fill probability gate
    fill_prob_threshold: f64,
    
    // Layer 5: Potential gradient gate
    potential_history: Arc<RwLock<Vec<f64>>>,
    
    // Layer 6: Candle body ratio gate
    body_ratio_threshold: f64,
    
    // State
    status: Arc<RwLock<GateStatus>>,
    last_trigger: AtomicU64,
    trigger_count: AtomicU64,
}

impl RiskGate {
    /// Create new risk gate
    pub fn new() -> Self {
        Self {
            volatility_regime: AtomicU64::new(0),
            time_in_regime: AtomicU64::new(0),
            kurtosis_threshold: 1.0,
            drift_threshold: 0.2,
            harmonic_detector: HarmonicTrapDetector::new(256),
            fill_prob_threshold: 0.6,
            potential_history: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            body_ratio_threshold: 0.7,
            status: Arc::new(RwLock::new(GateStatus::Open)),
            last_trigger: AtomicU64::new(0),
            trigger_count: AtomicU64::new(0),
        }
    }
    
    /// Evaluate all gates
    pub fn evaluate(&self, ctx: &GateContext) -> GateDecision {
        let start_time = crate::utils::time::get_hardware_timestamp();
        
        let mut triggered = Vec::new();
        
        // λ₁: Volatility regime gate
        if self.check_lambda1(ctx) {
            triggered.push(TriggerType::Lambda1);
        }
        
        // λ₂: Kurtosis/drift gate
        if self.check_lambda2(ctx) {
            triggered.push(TriggerType::Lambda2);
        }
        
        // λ₃: Harmonic trap gate
        if self.check_lambda3(ctx) {
            triggered.push(TriggerType::Lambda3);
        }
        
        // λ₄: Fill probability gate
        if self.check_lambda4(ctx) {
            triggered.push(TriggerType::Lambda4);
        }
        
        // λ₅: Potential gradient gate
        if self.check_lambda5(ctx) {
            triggered.push(TriggerType::Lambda5);
        }
        
        // λ₆: Candle body ratio gate
        if self.check_lambda6(ctx) {
            triggered.push(TriggerType::Lambda6);
        }
        
        // Determine final status
        let status = if triggered.len() >= 3 {
            GateStatus::Emergency
        } else if !triggered.is_empty() {
            GateStatus::Closed
        } else {
            GateStatus::Open
        };
        
        // Update state
        if status != GateStatus::Open {
            self.last_trigger.store(start_time, Ordering::Release);
            self.trigger_count.fetch_add(1, Ordering::Relaxed);
        }
        
        *self.status.write() = status;
        
        let reason = if triggered.is_empty() {
            "All gates passed".to_string()
        } else {
            format!("Triggered gates: {:?}", triggered)
        };
        
        GateDecision {
            status,
            triggered_gates: triggered,
            reason,
            timestamp_ns: start_time,
        }
    }
    
    /// λ₁: σ=2 ∧ τ_stay > τ_max ∧ (∫|∇P|dt / ATR₂₀) < δ
    fn check_lambda1(&self, ctx: &GateContext) -> bool {
        if ctx.volatility_regime != 2 {
            return false;
        }
        
        let time_in_regime = self.time_in_regime.load(Ordering::Acquire);
        if time_in_regime < ctx.tau_max_ns {
            return false;
        }
        
        let ratio = ctx.price_variation / (ctx.atr_20 + 1e-8);
        ratio < ctx.delta_threshold
    }
    
    /// λ₂: K(t)=1 ∧ 𝔼[sign(r)] < γ
    fn check_lambda2(&self, ctx: &GateContext) -> bool {
        let kurtosis_near_1 = (ctx.kurtosis - 1.0).abs() < 0.1;
        let drift_low = ctx.drift_bias < self.drift_threshold;
        
        kurtosis_near_1 && drift_low
    }
    
    /// λ₃: ∠(f̂_pred/f̂_act) > π/2
    fn check_lambda3(&self, ctx: &GateContext) -> bool {
        if ctx.predicted_prices.is_empty() || ctx.actual_prices.is_empty() {
            return false;
        }
        
        self.harmonic_detector.detect_trap(&ctx.predicted_prices, &ctx.actual_prices)
    }
    
    /// λ₄: φ_t > 0.6 ∧ 𝔼[P&L | φ_t > 0.6] < -ATR₁₀
    fn check_lambda4(&self, ctx: &GateContext) -> bool {
        if ctx.fill_probability <= self.fill_prob_threshold {
            return false;
        }
        
        ctx.conditional_pnl < -ctx.atr_10
    }
    
    /// λ₅: ∇U(P_t) · ∇U_hist(P_t) < 0
    fn check_lambda5(&self, ctx: &GateContext) -> bool {
        // U(P) = -log(depth)
        let grad_current = -1.0 / (ctx.current_depth + 1e-8);
        
        let grad_hist = {
            let history = self.potential_history.read();
            if history.is_empty() {
                return false;
            }
            let avg_depth = history.iter().sum::<f64>() / history.len() as f64;
            -1.0 / (avg_depth + 1e-8)
        };
        
        // Update history
        {
            let mut history = self.potential_history.write();
            history.push(ctx.current_depth);
            while history.len() > 1000 {
                history.remove(0);
            }
        }
        
        grad_current * grad_hist < 0.0
    }
    
    /// λ₆: ratio_body > 0.7 ∧ conflict
    fn check_lambda6(&self, ctx: &GateContext) -> bool {
        ctx.candle_body_ratio > self.body_ratio_threshold && ctx.order_book_conflict
    }
    
    /// Update time in regime (called externally)
    pub fn update_time_in_regime(&self, elapsed_ns: u64) {
        self.time_in_regime.fetch_add(elapsed_ns, Ordering::Relaxed);
    }
    
    /// Reset time in regime on regime change
    pub fn reset_time_in_regime(&self) {
        self.time_in_regime.store(0, Ordering::Release);
    }
    
    /// Get current gate status
    pub fn status(&self) -> GateStatus {
        *self.status.read()
    }
    
    /// Force gate closure (emergency)
    pub fn force_close(&self, reason: &str) {
        *self.status.write() = GateStatus::Emergency;
        tracing::error!("Risk gate force closed: {}", reason);
    }
    
    /// Get statistics
    pub fn stats(&self) -> GateStats {
        GateStats {
            status: self.status(),
            last_trigger_ns: self.last_trigger.load(Ordering::Acquire),
            total_triggers: self.trigger_count.load(Ordering::Relaxed),
        }
    }
}

/// Context for gate evaluation
#[derive(Debug, Clone)]
pub struct GateContext {
    pub volatility_regime: u8,
    pub tau_max_ns: u64,
    pub price_variation: f64,
    pub atr_20: f64,
    pub delta_threshold: f64,
    pub kurtosis: f64,
    pub drift_bias: f64,
    pub predicted_prices: Vec<f64>,
    pub actual_prices: Vec<f64>,
    pub fill_probability: f64,
    pub conditional_pnl: f64,
    pub atr_10: f64,
    pub current_depth: f64,
    pub candle_body_ratio: f64,
    pub order_book_conflict: bool,
}

impl Default for GateContext {
    fn default() -> Self {
        Self {
            volatility_regime: 0,
            tau_max_ns: 500_000_000, // 500ms
            price_variation: 0.0,
            atr_20: 0.001,
            delta_threshold: 0.3,
            kurtosis: 3.0,
            drift_bias: 0.0,
            predicted_prices: Vec::new(),
            actual_prices: Vec::new(),
            fill_probability: 0.5,
            conditional_pnl: 0.0,
            atr_10: 0.001,
            current_depth: 100000.0,
            candle_body_ratio: 0.5,
            order_book_conflict: false,
        }
    }
}

/// Gate statistics
#[derive(Debug, Clone)]
pub struct GateStats {
    pub status: GateStatus,
    pub last_trigger_ns: u64,
    pub total_triggers: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_risk_gate() {
        let gate = RiskGate::new();
        let ctx = GateContext::default();
        
        let decision = gate.evaluate(&ctx);
        assert_eq!(decision.status, GateStatus::Open);
    }
    
    #[test]
    fn test_lambda2_trigger() {
        let gate = RiskGate::new();
        let mut ctx = GateContext::default();
        ctx.kurtosis = 1.05;
        ctx.drift_bias = 0.1;
        
        let decision = gate.evaluate(&ctx);
        assert!(decision.triggered_gates.contains(&TriggerType::Lambda2));
    }
}
