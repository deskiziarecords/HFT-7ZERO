// ============================================================
// SIGNAL FUSION (P_fused)
// ============================================================
// Multi-method causality fusion
// Adaptive weighting based on performance
// Decay function for temporal relevance
// ============================================================

use super::*;
use std::collections::VecDeque;

/// Fusion configuration
#[derive(Debug, Clone)]
pub struct FusionConfig {
    pub decay_rate: f64,           // e^{-0.08τ} decay
    pub min_weight: f64,
    pub max_weight: f64,
    pub learning_rate: f64,
    pub history_window: usize,
    pub fusion_method: FusionMethod,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            decay_rate: 0.08,
            min_weight: 0.1,
            max_weight: 0.9,
            learning_rate: 0.01,
            history_window: 100,
            fusion_method: FusionMethod::Adaptive,
        }
    }
}

/// Fusion method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionMethod {
    Simple,      // Simple average
    Weighted,    // Fixed weights
    Adaptive,    // Adaptive weights based on past performance
    Bayesian,    // Bayesian model averaging
    Kalman,      // Kalman filter fusion
}

/// Fused signal
#[derive(Debug, Clone)]
pub struct FusedSignal {
    pub value: f64,
    pub confidence: f64,
    pub components: Vec<(String, f64, f64)>, // (method, value, weight)
    pub timestamp_ns: u64,
    pub latency_ns: u64,
}

/// Adaptive weight for each method
#[derive(Debug, Clone)]
pub struct AdaptiveWeight {
    pub method: String,
    pub weight: f64,
    pub performance: VecDeque<f64>,
    pub avg_performance: f64,
}

/// Signal fusion engine
pub struct SignalFusion {
    config: FusionConfig,
    weights: Vec<AdaptiveWeight>,
    prediction_history: VecDeque<FusedSignal>,
    kalman_state: Option<KalmanState>,
}

/// Kalman filter state for fusion
#[derive(Debug, Clone)]
struct KalmanState {
    x: f64,      // State estimate
    p: f64,      // Error covariance
    q: f64,      // Process noise
    r: f64,      // Measurement noise
}

impl SignalFusion {
    /// Create new signal fusion engine
    pub fn new(config: FusionConfig) -> Self {
        Self {
            config,
            weights: Vec::new(),
            prediction_history: VecDeque::with_capacity(config.history_window),
            kalman_state: None,
        }
    }
    
    /// Register a causality method
    pub fn register_method(&mut self, name: String, initial_weight: f64) {
        self.weights.push(AdaptiveWeight {
            method: name,
            weight: initial_weight.clamp(self.config.min_weight, self.config.max_weight),
            performance: VecDeque::with_capacity(self.config.history_window),
            avg_performance: 0.0,
        });
    }

    /// Fuse multiple predictions
    /// P_fused = (1-w)P_IPDA + w·max(P_lead · P_trans · e^{-0.08τ})
    pub fn fuse(&mut self, p_ipda: f64, components: Vec<(String, f64, f64)>, tau_seconds: f64) -> FusedSignal {
        let start_ns = crate::utils::time::get_hardware_timestamp();

        // Apply temporal decay
        let decay = (-self.config.decay_rate * tau_seconds).exp();

        let mut fused_value = 0.0;
        let mut total_weight = 0.0;
        let mut components_with_weight = Vec::new();

        // Calculate adaptive weights based on performance
        for (method, value, base_weight) in components {
            let adaptive_weight = self.get_adaptive_weight(&method);
            let weight = base_weight * adaptive_weight * decay;

            fused_value += value * weight;
            total_weight += weight;

            components_with_weight.push((method, value, weight));
        }

        // Apply IPDA component with base weight
        let w = self.get_ipda_weight();
        let final_value = (1.0 - w) * p_ipda + w * (fused_value / (total_weight + 1e-8));

        // Calculate confidence based on weight distribution
        let confidence = self.calculate_confidence(&components_with_weight);

        let latency_ns = crate::utils::time::get_hardware_timestamp() - start_ns;

        let signal = FusedSignal {
            value: final_value,
            confidence,
            components: components_with_weight,
            timestamp_ns: start_ns,
            latency_ns,
        };

        // Store history for adaptive learning
        self.prediction_history.push_back(signal.clone());
        while self.prediction_history.len() > self.config.history_window {
            self.prediction_history.pop_front();
        }

        signal
    }

    /// Get adaptive weight for a method based on past performance
    fn get_adaptive_weight(&mut self, method: &str) -> f64 {
        for weight in &mut self.weights {
            if weight.method == method {
                if weight.avg_performance > 0.0 {
                    // Higher weight for better performing methods
                    return (weight.avg_performance * 0.5 + 0.5)
                        .clamp(self.config.min_weight, self.config.max_weight);
                }
                return weight.weight;
            }
        }
        0.5
    }

    /// Get IPDA weight (can be adaptive based on market conditions)
    fn get_ipda_weight(&self) -> f64 {
        // IPDA weight decreases when other methods are confident
        if let Some(last) = self.prediction_history.back() {
            if last.confidence > 0.8 {
                return 0.3;
            }
        }
        0.5
    }

    /// Calculate confidence of fused prediction
    fn calculate_confidence(&self, components: &[(String, f64, f64)]) -> f64 {
        if components.is_empty() {
            return 0.0;
        }

        // Confidence based on weight entropy and agreement
        let total_weight: f64 = components.iter().map(|(_, _, w)| w).sum();
        let normalized_weights: Vec<f64> = components.iter()
            .map(|(_, _, w)| w / total_weight)
            .collect();

        // Entropy of weights (lower entropy = higher confidence)
        let entropy = -normalized_weights.iter()
            .map(|&w| if w > 0.0 { w * w.ln() } else { 0.0 })
            .sum::<f64>();

        // Agreement between components (variance of predictions)
        let values: Vec<f64> = components.iter().map(|(_, v, _)| *v).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // Confidence is high when entropy is low and predictions agree
        let confidence = 1.0 / (1.0 + entropy) * (1.0 / (1.0 + std_dev));
        confidence.clamp(0.0, 1.0)
    }

    /// Update weights based on prediction error
    pub fn update_weights(&mut self, actual_value: f64) {
        if let Some(last_prediction) = self.prediction_history.back() {
            let error = (actual_value - last_prediction.value).abs();

            for weight in &mut self.weights {
                // Find component weight for this method
                let component_weight = last_prediction.components.iter()
                    .find(|(m, _, _)| m == &weight.method)
                    .map(|(_, _, w)| *w)
                    .unwrap_or(0.0);

                if component_weight > 0.0 {
                    let performance = 1.0 / (1.0 + error);
                    weight.performance.push_back(performance);

                    while weight.performance.len() > self.config.history_window {
                        weight.performance.pop_front();
                    }

                    weight.avg_performance = weight.performance.iter().sum::<f64>() /
                                             weight.performance.len() as f64;

                    // Update weight based on performance
                    let new_weight = weight.weight + self.config.learning_rate * (performance - 0.5);
                    weight.weight = new_weight.clamp(self.config.min_weight, self.config.max_weight);
                }
            }
        }
    }

    /// Kalman filter fusion for real-time signals
    pub fn kalman_fuse(&mut self, measurement: f64, measurement_variance: f64) -> f64 {
        if self.kalman_state.is_none() {
            self.kalman_state = Some(KalmanState {
                x: measurement,
                p: 1.0,
                q: 0.01,
                r: measurement_variance,
            });
            return measurement;
        }

        let mut state = self.kalman_state.take().unwrap();

        // Prediction step
        let x_pred = state.x;
        let p_pred = state.p + state.q;

        // Update step
        let k = p_pred / (p_pred + state.r);
        let x_new = x_pred + k * (measurement - x_pred);
        let p_new = (1.0 - k) * p_pred;

        self.kalman_state = Some(KalmanState {
            x: x_new,
            p: p_new,
            q: state.q,
            r: state.r,
        });

        x_new
    }

    /// Bayesian model averaging
    pub fn bayesian_fuse(&self, predictions: &[(f64, f64)]) -> f64 {
        // predictions: (value, variance)
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for &(value, variance) in predictions {
            let precision = 1.0 / (variance + 1e-8);
            numerator += value * precision;
            denominator += precision;
        }

        numerator / (denominator + 1e-8)
    }

    /// Compute conditional beta: β_cond = β₀·𝕀[τ ≤ 180 ∧ exhaustion=0]
    pub fn conditional_beta(&self, beta_0: f64, tau_seconds: f64, exhaustion: bool) -> f64 {
        if tau_seconds <= 180.0 && !exhaustion {
            beta_0
        } else {
            0.0
        }
    }

    /// Compute lead-lag relationship with decay
    pub fn lead_lag_score(&self, p_lead: f64, p_trans: f64, tau_seconds: f64) -> f64 {
        p_lead * p_trans * (-self.config.decay_rate * tau_seconds).exp()
    }

    /// Get fusion statistics
    pub fn stats(&self) -> FusionStats {
        FusionStats {
            num_predictions: self.prediction_history.len(),
            avg_confidence: self.prediction_history.iter()
                .map(|s| s.confidence)
                .sum::<f64>() / self.prediction_history.len() as f64,
            weights: self.weights.clone(),
            last_timestamp: self.prediction_history.back().map(|s| s.timestamp_ns).unwrap_or(0),
        }
    }
}

/// Fusion statistics
#[derive(Debug, Clone)]
pub struct FusionStats {
    pub num_predictions: usize,
    pub avg_confidence: f64,
    pub weights: Vec<AdaptiveWeight>,
    pub last_timestamp: u64,
}

/// Multi-level causal fusion for complex relationships
pub struct MultiLevelFusion {
    level1: SignalFusion,   // Direct causality
    level2: SignalFusion,   // Lagged causality
    level3: SignalFusion,   // Nonlinear causality
}

impl MultiLevelFusion {
    pub fn new(config: FusionConfig) -> Self {
        Self {
            level1: SignalFusion::new(config.clone()),
            level2: SignalFusion::new(config.clone()),
            level3: SignalFusion::new(config),
        }
    }

    pub fn fuse_all(&mut self,
                    granger: f64,
                    transfer_entropy: f64,
                    ccm: f64,
                    spearman: f64,
                    tau_seconds: f64) -> FusedSignal {

        let mut components = Vec::new();
        components.push(("granger".to_string(), granger, 0.3));
        components.push(("transfer_entropy".to_string(), transfer_entropy, 0.3));
        components.push(("ccm".to_string(), ccm, 0.2));
        components.push(("spearman".to_string(), spearman, 0.2));

        self.level1.fuse(0.5, components, tau_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        println!("Fused value: {:.4}, confidence: {:.4}", result.value, result.confidence);

        assert!(result.value > 0.6);
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_kalman_fusion() {
        let config = FusionConfig::default();
        let mut fusion = SignalFusion::new(config);

        let measurements = vec![1.0, 1.1, 0.9, 1.05, 0.95];

        for &m in &measurements {
            let filtered = fusion.kalman_fuse(m, 0.1);
            println!("Measurement: {}, Filtered: {}", m, filtered);
        }
    }
}
