// ============================================================
// RISK GATE INTEGRATION TEST
// ============================================================
// Comprehensive testing of all 6 risk triggers
// Threshold validation
// Edge cases and boundary conditions
// ============================================================

use hft_stealth_system::*;
use std::time::Duration;

// ============================================================
// LAMBDA 1: VOLATILITY REGIME GATE
// ============================================================

#[test]
fn test_lambda1_volatility_regime() {
    let gate = RiskGate::new();
    
    // Test case: High volatility regime, long duration, low price variation
    let ctx = GateContext {
        volatility_regime: 2,
        tau_max_ns: 500_000_000,
        price_variation: 0.001,
        atr_20: 0.005,
        delta_threshold: 0.3,
        ..Default::default()
    };
    
    // Should NOT trigger (price_variation/ATR = 0.2 < 0.3, so should trigger)
    // Wait, condition is ratio < δ to trigger
    let decision = gate.evaluate(&ctx);
    assert!(decision.triggered_gates.contains(&TriggerType::Lambda1));
    
    // Test case: Low volatility regime - should not trigger
    let ctx_low = GateContext {
        volatility_regime: 1,
        ..ctx.clone()
    };
    let decision = gate.evaluate(&ctx_low);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda1));
    
    // Test case: High ratio - should not trigger
    let ctx_high_ratio = GateContext {
        price_variation: 0.002,
        atr_20: 0.001,
        ..ctx
    };
    let decision = gate.evaluate(&ctx_high_ratio);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda1));
}

// ============================================================
// LAMBDA 2: KURTOSIS/DRIFT GATE
// ============================================================

#[test]
fn test_lambda2_kurtosis_drift() {
    let gate = RiskGate::new();
    
    // Test case: Kurtosis near 1, low drift
    let ctx = GateContext {
        kurtosis: 1.05,
        drift_bias: 0.1,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(decision.triggered_gates.contains(&TriggerType::Lambda2));
    
    // Test case: Normal kurtosis (3.0) - should not trigger
    let ctx_normal = GateContext {
        kurtosis: 3.0,
        drift_bias: 0.5,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_normal);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda2));
    
    // Test case: High drift - should not trigger
    let ctx_high_drift = GateContext {
        kurtosis: 1.05,
        drift_bias: 0.5,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_high_drift);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda2));
}

// ============================================================
// LAMBDA 3: HARMONIC TRAP GATE
// ============================================================

#[test]
fn test_lambda3_harmonic_trap() {
    let gate = RiskGate::new();
    
    // In-phase signals (no trap)
    let in_phase_pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let in_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    let ctx = GateContext {
        predicted_prices: in_phase_pred,
        actual_prices: in_phase_act,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda3));
    
    // Out-of-phase signals (trap)
    let out_phase_act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + std::f64::consts::PI).sin()).collect();
    
    let ctx_trap = GateContext {
        predicted_prices: (0..256).map(|i| (i as f64 * 0.1).sin()).collect(),
        actual_prices: out_phase_act,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx_trap);
    assert!(decision.triggered_gates.contains(&TriggerType::Lambda3));
}

// ============================================================
// LAMBDA 4: FILL PROBABILITY GATE
// ============================================================

#[test]
fn test_lambda4_fill_probability() {
    let gate = RiskGate::new();
    
    // Test case: High fill probability, negative conditional PnL
    let ctx = GateContext {
        fill_probability: 0.8,
        conditional_pnl: -0.01,
        atr_10: 0.003,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(decision.triggered_gates.contains(&TriggerType::Lambda4));
    
    // Test case: Low fill probability - should not trigger
    let ctx_low_fill = GateContext {
        fill_probability: 0.5,
        conditional_pnl: -0.01,
        atr_10: 0.003,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_low_fill);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda4));
    
    // Test case: Positive conditional PnL - should not trigger
    let ctx_positive_pnl = GateContext {
        fill_probability: 0.8,
        conditional_pnl: 0.01,
        atr_10: 0.003,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_positive_pnl);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda4));
}

// ============================================================
// LAMBDA 5: POTENTIAL GRADIENT GATE
// ============================================================

#[test]
fn test_lambda5_potential_gradient() {
    let gate = RiskGate::new();
    
    // Test case: Diverging gradients
    let ctx = GateContext {
        current_depth: 1000.0,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    // First evaluation should have history, so may not trigger immediately
    
    // After multiple evaluations, check gradient sign change
    for i in 0..10 {
        let ctx = GateContext {
            current_depth: 1000.0 + (i as f64 * 100.0),
            ..Default::default()
        };
        gate.evaluate(&ctx);
    }
    
    // Now with different depth direction
    let ctx_divergent = GateContext {
        current_depth: 500.0,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_divergent);
    // May trigger depending on gradient product
}

// ============================================================
// LAMBDA 6: CANDLE BODY RATIO GATE
// ============================================================

#[test]
fn test_lambda6_candle_body() {
    let gate = RiskGate::new();
    
    // Test case: High body ratio with conflict
    let ctx = GateContext {
        candle_body_ratio: 0.8,
        order_book_conflict: true,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(decision.triggered_gates.contains(&TriggerType::Lambda6));
    
    // Test case: High body ratio without conflict - should not trigger
    let ctx_no_conflict = GateContext {
        candle_body_ratio: 0.8,
        order_book_conflict: false,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_no_conflict);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda6));
    
    // Test case: Low body ratio with conflict - should not trigger
    let ctx_low_ratio = GateContext {
        candle_body_ratio: 0.5,
        order_book_conflict: true,
        ..Default::default()
    };
    let decision = gate.evaluate(&ctx_low_ratio);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda6));
}

// ============================================================
// COMBINED TRIGGER TESTS
// ============================================================

#[test]
fn test_multiple_triggers() {
    let gate = RiskGate::new();
    
    // Create context that triggers multiple gates
    let ctx = GateContext {
        volatility_regime: 2,
        kurtosis: 1.05,
        drift_bias: 0.1,
        fill_probability: 0.8,
        conditional_pnl: -0.01,
        atr_10: 0.003,
        candle_body_ratio: 0.8,
        order_book_conflict: true,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(decision.triggered_gates.len() >= 3);
    assert_eq!(decision.status, GateStatus::Emergency);
}

// ============================================================
// EDGE CASE TESTS
// ============================================================

#[test]
fn test_boundary_conditions() {
    let gate = RiskGate::new();
    
    // Zero values
    let ctx_zero = GateContext {
        volatility_regime: 0,
        kurtosis: 0.0,
        drift_bias: 0.0,
        fill_probability: 0.0,
        conditional_pnl: 0.0,
        candle_body_ratio: 0.0,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx_zero);
    assert!(!decision.triggered_gates.contains(&TriggerType::Lambda2));
    
    // Maximum values
    let ctx_max = GateContext {
        volatility_regime: 2,
        kurtosis: 10.0,
        drift_bias: 1.0,
        fill_probability: 1.0,
        candle_body_ratio: 1.0,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx_max);
    // Should handle without panicking
}

// ============================================================
// STRESS TESTING
// ============================================================

#[test]
fn test_rapid_gate_evaluations() {
    let gate = RiskGate::new();
    
    for i in 0..10000 {
        let ctx = GateContext {
            volatility_regime: (i % 3) as u8,
            kurtosis: 1.0 + (i % 10) as f64 * 0.1,
            drift_bias: (i % 10) as f64 * 0.1,
            fill_probability: (i % 10) as f64 * 0.1,
            conditional_pnl: if i % 2 == 0 { 0.01 } else { -0.01 },
            candle_body_ratio: (i % 10) as f64 * 0.1,
            order_book_conflict: i % 3 == 0,
            ..Default::default()
        };
        
        let decision = gate.evaluate(&ctx);
        // Just ensure no panic
        assert!(decision.status == GateStatus::Open || 
                decision.status == GateStatus::Closed || 
                decision.status == GateStatus::Emergency);
    }
}

// ============================================================
// GATE RESET TESTS
// ============================================================

#[test]
fn test_gate_reset() {
    let gate = RiskGate::new();
    
    // Trigger some gates
    let ctx = GateContext {
        volatility_regime: 2,
        kurtosis: 1.05,
        drift_bias: 0.1,
        ..Default::default()
    };
    
    let decision = gate.evaluate(&ctx);
    assert!(decision.status != GateStatus::Open);
    
    // Reset time in regime (simulate regime change)
    gate.reset_time_in_regime();
    
    // Re-evaluate
    let decision2 = gate.evaluate(&ctx);
    // May still trigger other lambdas
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

#[test]
fn test_gate_stats() {
    let gate = RiskGate::new();
    
    // Initial stats
    let stats = gate.stats();
    assert_eq!(stats.total_triggers, 0);
    assert_eq!(stats.status, GateStatus::Open);
    
    // Trigger some gates
    let ctx = GateContext {
        volatility_regime: 2,
        kurtosis: 1.05,
        drift_bias: 0.1,
        ..Default::default()
    };
    
    gate.evaluate(&ctx);
    
    let stats = gate.stats();
    assert!(stats.total_triggers > 0);
    assert!(stats.last_trigger_ns > 0);
}
