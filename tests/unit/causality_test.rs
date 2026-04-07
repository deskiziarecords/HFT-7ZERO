// ============================================================
// CAUSALITY MODULE UNIT TESTS
// ============================================================
// Granger causality tests
// Transfer entropy validation
// CCM convergence checking
// Spearman correlation accuracy
// ============================================================

use hft_stealth_system::causality::*;

// ============================================================
// GRANGER CAUSALITY TESTS
// ============================================================

#[test]
fn test_granger_causality_detection() {
    let config = CausalityConfig::default();
    let mut granger = GrangerCausality::new(config);
    
    // Generate data where X causes Y
    let n = 500;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).sin()).collect();
    let mut y: Vec<f64> = x.iter().map(|&v| v * 0.5).collect();
    
    for i in 1..n {
        y[i] += 0.1 * (i as f64).sin();
    }
    
    let result = granger.test(&y, &x, 2);
    assert!(result.is_causal, "Should detect causality");
    assert!(result.f_statistic > 10.0, "F-statistic too low: {}", result.f_statistic);
    assert!(result.p_value < 0.05, "P-value too high: {}", result.p_value);
}

#[test]
fn test_granger_no_causality() {
    let config = CausalityConfig::default();
    let mut granger = GrangerCausality::new(config);
    
    // Generate independent time series
    let n = 500;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).sin()).collect();
    let y: Vec<f64> = (0..n).map(|i| (i as f64 * 0.013).cos()).collect();
    
    let result = granger.test(&y, &x, 2);
    assert!(!result.is_causal, "Should not detect false causality");
}

#[test]
fn test_optimal_lag_selection() {
    let config = CausalityConfig::default();
    let mut granger = GrangerCausality::new(config);
    
    let n = 1000;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).sin()).collect();
    let mut y: Vec<f64> = x.iter().map(|&v| v * 0.5).collect();
    
    // Add lag-3 dependency
    for i in 3..n {
        y[i] += 0.3 * x[i - 3];
    }
    
    let optimal_lag = granger.find_optimal_lag(&y, &x, 10);
    assert_eq!(optimal_lag, 3, "Should select lag 3, got {}", optimal_lag);
}

// ============================================================
// TRANSFER ENTROPY TESTS
// ============================================================

#[test]
fn test_transfer_entropy_detection() {
    let config = TEConfig::default();
    let mut te = TransferEntropy::new(config);
    
    let n = 1000;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).sin()).collect();
    let mut y: Vec<f64> = x.iter().map(|&v| v * 0.8).collect();
    
    for i in 1..n {
        y[i] += 0.1 * (i as f64).sin();
    }
    
    let result = te.calculate(&y, &x, 1);
    assert!(result.is_significant, "Transfer entropy should detect causality");
    assert!(result.te_value > 0.01, "TE value too low: {}", result.te_value);
}

#[test]
fn test_optimal_lag_te() {
    let config = TEConfig::default();
    let mut te = TransferEntropy::new(config);
    
    let n = 1000;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).sin()).collect();
    let mut y: Vec<f64> = x.iter().map(|&v| v * 0.5).collect();
    
    // Add lag-2 dependency
    for i in 2..n {
        y[i] += 0.3 * x[i - 2];
    }
    
    let optimal_lag = te.find_optimal_lag(&y, &x, 10);
    assert_eq!(optimal_lag, 2, "Should select lag 2, got {}", optimal_lag);
}

// ============================================================
// CONVERGENT CROSS MAPPING TESTS
// ============================================================

#[test]
fn test_ccm_causality() {
    let config = CCMConfig::default();
    let mut ccm = ConvergentCrossMapping::new(config);
    
    // Generate coupled logistic maps
    let n = 500;
    let mut x = vec![0.5; n];
    let mut y = vec![0.3; n];
    
    for i in 1..n {
        x[i] = 3.8 * x[i-1] * (1.0 - x[i-1]);
        y[i] = 3.8 * y[i-1] * (1.0 - y[i-1]) + 0.2 * x[i-1];
    }
    
    let result = ccm.test(&y, &x);
    println!("CCM rho: {:.4}, slope: {:.4}", result.rho, result.convergence_slope);
    
    // Should detect causality with positive convergence
    assert!(result.convergence_slope > 0.0, "Convergence slope should be positive");
}

#[test]
fn test_bidirectional_ccm() {
    let config = CCMConfig::default();
    let mut ccm = ConvergentCrossMapping::new(config);
    
    // Generate bidirectionally coupled systems
    let n = 500;
    let mut x = vec![0.5; n];
    let mut y = vec![0.3; n];
    
    for i in 1..n {
        x[i] = 3.8 * x[i-1] * (1.0 - x[i-1]) + 0.1 * y[i-1];
        y[i] = 3.8 * y[i-1] * (1.0 - y[i-1]) + 0.1 * x[i-1];
    }
    
    let (x_to_y, y_to_x) = ccm.test_bidirectional(&y, &x);
    println!("X→Y: {}, Y→X: {}", x_to_y, y_to_x);
}

// ============================================================
// SPEARMAN CORRELATION TESTS
// ============================================================

#[test]
fn test_spearman_correlation() {
    let config = CausalityConfig::default();
    let mut spearman = SpearmanCorrelation::new(config);
    
    let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let y: Vec<f64> = x.iter().map(|&v| v * 2.0 + 1.0).collect();
    
    let result = spearman.calculate_with_lag(&x, &y, 10);
    assert!((result.rho - 1.0).abs() < 0.01, "Perfect correlation should be 1.0");
    assert_eq!(result.optimal_lag, 0, "Optimal lag should be 0");
}

#[test]
fn test_lagged_correlation() {
    let config = CausalityConfig::default();
    let spearman = SpearmanCorrelation::new(config);
    
    let x: Vec<f64> = (0..200).map(|i| i as f64).collect();
    let mut y: Vec<f64> = vec![0.0; 200];
    
    // Shift by 10
    for i in 10..200 {
        y[i] = x[i - 10] * 2.0;
    }
    
    let lagged = spearman.lagged_correlation(&x, &y, 20);
    assert_eq!(lagged.max_lag, 10, "Max correlation should be at lag 10");
    assert!((lagged.max_correlation - 1.0).abs() < 0.01);
}

// ============================================================
// SIGNAL FUSION TESTS
// ============================================================

#[test]
fn test_signal_fusion() {
    let config = FusionConfig::default();
    let mut fusion = SignalFusion::new(config);
    
    fusion.register_method("granger".to_string(), 0.4);
    fusion.register_method("te".to_string(), 0.3);
    fusion.register_method("ccm".to_string(), 0.3);
    
    let components = vec![
        ("granger".to_string(), 0.8, 0.4),
        ("te".to_string(), 0.7, 0.3),
        ("ccm".to_string(), 0.6, 0.3),
    ];
    
    let result = fusion.fuse(0.5, components, 1.0);
    assert!(result.value > 0.6 && result.value < 0.8);
    assert!(result.confidence > 0.0);
}

#[test]
fn test_temporal_decay() {
    let config = FusionConfig::default();
    let fusion = SignalFusion::new(config);
    
    let score_short = fusion.lead_lag_score(0.9, 0.9, 1.0);
    let score_long = fusion.lead_lag_score(0.9, 0.9, 100.0);
    
    assert!(score_short > score_long, "Short lag should have higher score");
}

#[test]
fn test_conditional_beta() {
    let config = FusionConfig::default();
    let fusion = SignalFusion::new(config);
    
    let beta_valid = fusion.conditional_beta(1.0, 30.0, false);
    assert_eq!(beta_valid, 1.0);
    
    let beta_long = fusion.conditional_beta(1.0, 200.0, false);
    assert_eq!(beta_long, 0.0);
    
    let beta_exhausted = fusion.conditional_beta(1.0, 30.0, true);
    assert_eq!(beta_exhausted, 0.0);
}

// ============================================================
// KALMAN FUSION TESTS
// ============================================================

#[test]
fn test_kalman_fusion() {
    let config = FusionConfig::default();
    let mut fusion = SignalFusion::new(config);
    
    let measurements = vec![1.0, 1.1, 0.9, 1.05, 0.95, 1.02, 0.98];
    let mut filtered = Vec::new();
    
    for &m in &measurements {
        let f = fusion.kalman_fuse(m, 0.1);
        filtered.push(f);
    }
    
    // Filtered values should be smoother than measurements
    let measurement_std = std_dev(&measurements);
    let filtered_std = std_dev(&filtered);
    
    assert!(filtered_std < measurement_std, "Kalman filter should reduce variance");
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn std_dev(data: &[f64]) -> f64 {
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
    variance.sqrt()
}
