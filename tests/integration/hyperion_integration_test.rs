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
        adelic_rho: 1000.0,
        adelic_max_nonzero: 2,
        blowup_kappa: 1000.0,
    };
    let router = SchurRouter::new(venues, params);
    let ofi = DMatrix::from_element(2, 2, 0.1);
    let prev_w = DVector::from_element(2, 0.5);

    let result = router.optimize(100.0, &ofi, &prev_w).unwrap();
    assert!(result.adelic_valid);
    assert!((result.weights.iter().sum::<f64>() - 1.0).abs() < 1e-6);
}

#[test]
fn test_schur_routing_empty_venues() {
    let router = SchurRouter::new(vec![], RoutingParams {
        slippage_gamma: vec![],
        slippage_delta: vec![],
        correlation_decay: 0.0,
        adelic_rho: 0.0,
        adelic_max_nonzero: 0,
        blowup_kappa: 0.0,
    });
    let result = router.optimize(100.0, &DMatrix::zeros(0, 0), &DVector::zeros(0));
    assert!(result.is_none());
}
