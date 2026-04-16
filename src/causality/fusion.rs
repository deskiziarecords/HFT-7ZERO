use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct FusionConfig {
    pub decay_rate: f64,
    pub min_weight: f64,
    pub max_weight: f64,
    pub learning_rate: f64,
    pub history_window: usize,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            decay_rate: 0.08,
            min_weight: 0.1,
            max_weight: 0.9,
            learning_rate: 0.01,
            history_window: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FusedSignal {
    pub value: f64,
    pub confidence: f64,
    pub components: Vec<(String, f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct AdaptiveWeight {
    pub method: String,
    pub weight: f64,
    pub performance: VecDeque<f64>,
    pub avg_performance: f64,
}

pub struct SignalFusion {
    pub config: FusionConfig,
    pub weights: Vec<AdaptiveWeight>,
    pub prediction_history: VecDeque<FusedSignal>,
}

impl SignalFusion {
    pub fn new(config: FusionConfig) -> Self {
        Self {
            config: config.clone(),
            weights: Vec::new(),
            prediction_history: VecDeque::with_capacity(config.history_window),
        }
    }
    
    pub fn fuse(&mut self, p_ipda: f64, components: Vec<(String, f64, f64)>, tau_seconds: f64) -> FusedSignal {
        let decay = (-self.config.decay_rate * tau_seconds).exp();
        let mut fused_value = 0.0;
        let mut total_weight = 0.0;
        for (_, value, weight) in &components {
            fused_value += value * weight * decay;
            total_weight += weight * decay;
        }
        let w = 0.5;
        let final_value = (1.0 - w) * p_ipda + w * (fused_value / (total_weight + 1e-8));
        FusedSignal {
            value: final_value,
            confidence: 0.8,
            components,
        }
    }
}

pub struct MultiLevelFusion {
    pub level1: SignalFusion,
    pub level2: SignalFusion,
    pub level3: SignalFusion,
}

impl MultiLevelFusion {
    pub fn new(config: FusionConfig) -> Self {
        Self {
            level1: SignalFusion::new(config.clone()),
            level2: SignalFusion::new(config.clone()),
            level3: SignalFusion::new(config),
        }
    }
}
