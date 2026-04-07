// ============================================================
// MACHINE LEARNING MODULE
// ============================================================
// JAX/XLA integration for high-performance inference
// Batch processing with sub-millisecond latency
// Feature extraction from market data
// Model versioning and hot-swapping
// ============================================================

pub mod jax_bridge;
pub mod batch_inference;
pub mod feature_extractor;
pub mod model_cache;
pub mod tensor_ops;
pub mod online_learning;

pub use jax_bridge::{JAXModel, JAXConfig, ModelOutput, InferenceMode};
pub use batch_inference::{BatchInferenceEngine, BatchConfig, InferenceBatch};
pub use feature_extractor::{FeatureExtractor, FeatureSet, MarketFeatures};
pub use model_cache::{ModelCache, CachedModel, ModelVersion};
pub use tensor_ops::{Tensor, TensorView, TensorOps};
pub use online_learning::{OnlineLearner, LearningUpdate, GradientBuffer};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// ML configuration
#[derive(Debug, Clone)]
pub struct MLConfig {
    pub model_path: String,
    pub batch_size: usize,
    pub max_batch_delay_ns: u64,
    pub feature_window_size: usize,
    pub inference_timeout_ms: u64,
    pub use_gpu: bool,
    pub gpu_device_id: i32,
    pub enable_profiling: bool,
}

impl Default for MLConfig {
    fn default() -> Self {
        Self {
            model_path: "models/production.xla".to_string(),
            batch_size: 32,
            max_batch_delay_ns: 100_000, // 100 microseconds
            feature_window_size: 256,
            inference_timeout_ms: 1,
            use_gpu: true,
            gpu_device_id: 0,
            enable_profiling: false,
        }
    }
}

/// ML performance metrics
#[derive(Debug, Default, Clone)]
pub struct MLMetrics {
    pub inferences_total: u64,
    pub inferences_failed: u64,
    pub avg_inference_time_ns: u64,
    pub p99_inference_time_ns: u64,
    pub batch_size_avg: f64,
    pub cache_hit_rate: f64,
    pub throughput_per_sec: f64,
}

/// Global ML state
pub struct MLState {
    pub models: DashMap<String, Arc<JAXModel>>,
    pub feature_extractors: DashMap<u32, Arc<FeatureExtractor>>,
    pub metrics: Arc<RwLock<MLMetrics>>,
    pub config: MLConfig,
}

impl MLState {
    pub fn new(config: MLConfig) -> Self {
        Self {
            models: DashMap::new(),
            feature_extractors: DashMap::new(),
            metrics: Arc::new(RwLock::new(MLMetrics::default())),
            config,
        }
    }
    
    pub fn register_model(&self, name: String, model: JAXModel) {
        self.models.insert(name, Arc::new(model));
    }
    
    pub fn get_model(&self, name: &str) -> Option<Arc<JAXModel>> {
        self.models.get(name).map(|m| m.clone())
    }
}

/// Prediction result with confidence
#[derive(Debug, Clone)]
pub struct Prediction {
    pub value: f64,
    pub confidence: f64,
    pub timestamp_ns: u64,
    pub model_version: String,
    pub inference_time_ns: u64,
}

/// Batch prediction result
#[derive(Debug)]
pub struct BatchPrediction {
    pub predictions: Vec<Prediction>,
    pub batch_size: usize,
    pub total_time_ns: u64,
}
