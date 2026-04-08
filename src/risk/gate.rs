// ============================================================
// RISK GATE
// ============================================================

use crate::risk::triggers::TriggerType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStatus {
    Open,
    Closed,
    Emergency,
}

pub struct GateDecision {
    pub status: GateStatus,
    pub triggered_gates: Vec<TriggerType>,
}

pub struct RiskGate {
    pub status: GateStatus,
}

pub struct GateContext {
    pub volatility_regime: u8,
    pub price_variation: f64,
    pub atr_20: f64,
    pub delta_threshold: f64,
    pub kurtosis: f64,
    pub drift_bias: f64,
    pub gamma: f64,
}

impl Default for GateContext {
    fn default() -> Self {
        Self {
            volatility_regime: 0,
            price_variation: 0.0,
            atr_20: 0.001,
            delta_threshold: 0.3,
            kurtosis: 3.0,
            drift_bias: 0.0,
            gamma: 0.2,
        }
    }
}

impl RiskGate {
    pub fn new() -> Self {
        Self {
            status: GateStatus::Open,
        }
    }
    
    pub fn evaluate(&self, ctx: &GateContext) -> GateDecision {
        let mut triggered = Vec::new();
        
        // λ₁: Volatility regime gate
        if ctx.volatility_regime == 2 && (ctx.price_variation / (ctx.atr_20 + 1e-8)) < ctx.delta_threshold {
            triggered.push(TriggerType::Lambda1);
        }

        // λ₂: Kurtosis/drift gate
        if (ctx.kurtosis - 1.0).abs() < 0.1 && ctx.drift_bias < ctx.gamma {
            triggered.push(TriggerType::Lambda2);
        }

        let status = if triggered.is_empty() { GateStatus::Open } else { GateStatus::Closed };

        GateDecision {
            status,
            triggered_gates: triggered,
        }
    }
}
