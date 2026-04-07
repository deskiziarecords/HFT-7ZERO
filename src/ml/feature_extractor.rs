// ============================================================
// FEATURE EXTRACTOR
// ============================================================
// Real-time feature extraction from market data
// Normalization and scaling
// Window-based aggregation
// ============================================================

use super::*;
use crate::market::{OrderBook, Tick, DepthProfile};
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;

/// Feature set configuration
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    pub window_sizes: Vec<usize>,      // Multiple time windows
    pub include_depth_features: bool,
    pub include_microstructure: bool,
    pub include_order_flow: bool,
    pub include_volatility: bool,
    pub normalize_features: bool,
    pub num_price_levels: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            window_sizes: vec![10, 20, 50, 100],
            include_depth_features: true,
            include_microstructure: true,
            include_order_flow: true,
            include_volatility: true,
            normalize_features: true,
            num_price_levels: 10,
        }
    }
}

/// Extracted features
#[derive(Debug, Clone)]
pub struct FeatureSet {
    pub timestamp_ns: u64,
    pub instrument_id: u32,
    pub features: Vec<f32>,
    pub feature_names: Vec<String>,
    pub window_sizes: Vec<usize>,
}

/// Market features structure
#[derive(Debug, Clone, Default)]
pub struct MarketFeatures {
    // Price features
    pub mid_price: f64,
    pub spread: f64,
    pub spread_ticks: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    
    // Volume features
    pub bid_volume_1: f64,
    pub ask_volume_1: f64,
    pub bid_volume_5: f64,
    pub ask_volume_5: f64,
    pub bid_volume_10: f64,
    pub ask_volume_10: f64,
    pub total_volume: f64,
    
    // Imbalance features
    pub order_imbalance_1: f64,
    pub order_imbalance_5: f64,
    pub order_imbalance_10: f64,
    pub volume_imbalance: f64,
    
    // Microstructure features
    pub trade_flow: f64,
    pub cancellation_rate: f64,
    pub order_arrival_rate: f64,
    pub effective_spread: f64,
    pub realized_spread: f64,
    
    // Volatility features
    pub return_1: f64,
    pub return_5: f64,
    pub return_10: f64,
    pub volatility_20: f64,
    pub high_low_ratio: f64,
    
    // Momentum features
    pub price_momentum_10: f64,
    pub price_momentum_20: f64,
    pub volume_momentum: f64,
    
    // Seasonality
    pub hour_of_day: f64,
    pub minute_of_hour: f64,
    pub day_of_week: f64,
}

/// Main feature extractor
pub struct FeatureExtractor {
    config: FeatureConfig,
    price_history: VecDeque<f64>,
    volume_history: VecDeque<f64>,
    trade_history: VecDeque<Tick>,
    spread_history: VecDeque<f64>,
    depth_cache: Option<DepthProfile>,
    normalization_stats: NormalizationStats,
}

/// Normalization statistics
#[derive(Debug, Clone)]
pub struct NormalizationStats {
    pub means: Vec<f32>,
    pub stds: Vec<f32>,
    pub fitted: bool,
}

impl Default for NormalizationStats {
    fn default() -> Self {
        Self {
            means: Vec::new(),
            stds: Vec::new(),
            fitted: false,
        }
    }
}

impl FeatureExtractor {
    /// Create new feature extractor
    pub fn new(config: FeatureConfig) -> Self {
        let max_window = config.window_sizes.iter().max().copied().unwrap_or(100);
        
        Self {
            config,
            price_history: VecDeque::with_capacity(max_window),
            volume_history: VecDeque::with_capacity(max_window),
            trade_history: VecDeque::with_capacity(max_window),
            spread_history: VecDeque::with_capacity(max_window),
            depth_cache: None,
            normalization_stats: NormalizationStats::default(),
        }
    }
    
    /// Update with new tick
    pub fn update(&mut self, tick: &Tick) {
        self.price_history.push_back(tick.price);
        self.volume_history.push_back(tick.volume);
        self.spread_history.push_back(0.0); // Would get from order book
        
        if tick.is_trade() {
            self.trade_history.push_back(*tick);
        }
        
        // Maintain window size
        let max_window = self.config.window_sizes.iter().max().copied().unwrap_or(100);
        while self.price_history.len() > max_window {
            self.price_history.pop_front();
            self.volume_history.pop_front();
            self.spread_history.pop_front();
        }
        while self.trade_history.len() > max_window {
            self.trade_history.pop_front();
        }
    }
    
    /// Update with order book snapshot
    pub fn update_depth(&mut self, depth: DepthProfile) {
        self.depth_cache = Some(depth);
    }
    
    /// Extract features from current state
    pub fn extract(&self, instrument_id: u32) -> FeatureSet {
        let market_features = self.compute_market_features();
        let mut features = self.features_to_vector(&market_features);
        
        // Add windowed features
        for &window in &self.config.window_sizes {
            let window_features = self.compute_window_features(window);
            features.extend(window_features);
        }
        
        // Add depth features
        if self.config.include_depth_features {
            if let Some(depth) = &self.depth_cache {
                let depth_features = self.extract_depth_features(depth);
                features.extend(depth_features);
            }
        }
        
        // Normalize if configured
        let features = if self.config.normalize_features && self.normalization_stats.fitted {
            self.normalize(&features)
        } else {
            features
        };
        
        FeatureSet {
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            instrument_id,
            features,
            feature_names: self.generate_feature_names(),
            window_sizes: self.config.window_sizes.clone(),
        }
    }
    
    /// Compute core market features
    fn compute_market_features(&self) -> MarketFeatures {
        let mut features = MarketFeatures::default();
        
        // Price features
        features.mid_price = self.price_history.back().copied().unwrap_or(0.0);
        features.spread = self.spread_history.back().copied().unwrap_or(0.0);
        
        // Volume features
        features.bid_volume_1 = self.volume_history.back().copied().unwrap_or(0.0);
        
        // Returns
        if self.price_history.len() >= 2 {
            let last = *self.price_history.back().unwrap();
            let prev = self.price_history[self.price_history.len() - 2];
            features.return_1 = (last - prev) / prev;
        }
        
        if self.price_history.len() >= 5 {
            let last = *self.price_history.back().unwrap();
            let prev_5 = self.price_history[self.price_history.len() - 5];
            features.return_5 = (last - prev_5) / prev_5;
        }
        
        if self.price_history.len() >= 10 {
            let last = *self.price_history.back().unwrap();
            let prev_10 = self.price_history[self.price_history.len() - 10];
            features.return_10 = (last - prev_10) / prev_10;
        }
        
        // Momentum
        if self.price_history.len() >= 10 {
            let recent_avg: f64 = self.price_history.iter().rev().take(5).sum::<f64>() / 5.0;
            let older_avg: f64 = self.price_history.iter().rev().skip(5).take(5).sum::<f64>() / 5.0;
            features.price_momentum_10 = (recent_avg - older_avg) / older_avg;
        }
        
        // Volatility
        if self.price_history.len() >= 20 {
            let returns: Vec<f64> = self.price_history.windows(2)
                .map(|w| (w[1] - w[0]) / w[0])
                .collect();
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
            features
