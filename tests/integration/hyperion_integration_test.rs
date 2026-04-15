use hft_7zero::risk::ev_atr::*;
use hft_7zero::execution::schur_router::*;
use nalgebra::{DMatrix, DVector};

#[test]
fn test_ev_atr_confluence() {
    let model = EVATRModel::new(EVATRParams::default());
    let ev_t = 0.0114;
    let atr_t = 0.008;
    let phi_t = 0.75;

    let qt = model.compute_q_t(ev_t, atr_t, phi_t);
    // f_kelly = 0.0114 / (3.0 * 0.015 * 0.005) = 50.666...
    // g_vol = (0.005 / 0.008)^0.5 = 0.7905...
    // h_conf = 0.75^1.5 = 0.6495...
    // C_max = 1000
    // Result = 50.66 * 0.79 * 0.6495 * 1000 = 26012.8
    assert!(qt > 26000.0 && qt < 26100.0, "qt was {}", qt);
}

#[test]
fn test_schur_routing() {
    let venues = vec![
        Venue { id: 0, latency_ms: 0.1, fees: 0.0001 },
        Venue { id: 1, latency_ms: 0.2, fees: 0.0002 },
    ];
    let params = RoutingParams {
        slippage_gamma: vec![0.1, 0.05],
        slippage_delta: vec![1.5, 1.5],
        correlation_decay: 0.01,
        adelic_rho: 1000.0, // Relaxed for testing
        adelic_max_nonzero: 2,
        blowup_kappa: 1000.0,
    };
    let router = SchurRouter::new(venues, params);
    let ofi = DMatrix::from_element(2, 2, 0.1);
    let prev_w = DVector::from_element(2, 0.5);

    let result = router.optimize(100.0, &ofi, &prev_w);
    assert!(result.adelic_valid);
    assert!((result.weights.iter().sum::<f64>() - 1.0).abs() < 1e-6);
}
