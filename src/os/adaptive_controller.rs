use std::collections::HashMap;
use crate::os::OperatorResult;

pub struct AdaptiveController {
    pub layer_performance: HashMap<String, f64>,
    pub lambda_weights: [f64; 6],
}

impl AdaptiveController {
    pub fn new() -> Self {
        Self {
            layer_performance: HashMap::new(),
            lambda_weights: [1.0; 6],
        }
    }

    pub fn update(&mut self, layer_outputs: &HashMap<String, OperatorResult>) {
        if let Some(r) = layer_outputs.get("L5") {
            if r.value > 0.8 {
                self.lambda_weights[4] *= 0.95;
            }
        }
    }

    pub fn get_weights(&self) -> [f64; 6] {
        self.lambda_weights
    }
}
