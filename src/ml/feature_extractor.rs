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
            features.volatility_20 = variance.sqrt();
        }
        
        // Seasonality
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let hour = (now.as_secs() % 86400) / 3600;
        let minute = (now.as_secs() % 3600) / 60;
        let day = now.as_secs() / 86400 % 7;
        
        features.hour_of_day = hour as f64 / 24.0;
        features.minute_of_hour = minute as f64 / 60.0;
        features.day_of_week = day as f64 / 7.0;
        
        features
    }
    
    /// Compute window-based features
    fn compute_window_features(&self, window: usize) -> Vec<f32> {
        let mut features = Vec::new();
        
        if self.price_history.len() >= window {
            let prices: Vec<f64> = self.price_history.iter().rev().take(window).copied().collect();
            let volumes: Vec<f64> = self.volume_history.iter().rev().take(window).copied().collect();
            
            // Mean and std of returns
            let returns: Vec<f64> = prices.windows(2)
                .map(|w| (w[1] - w[0]) / w[0])
                .collect();
            
            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            let std_return = (returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64).sqrt();
            
            features.push(mean_return as f32);
            features.push(std_return as f32);
            
            // Volume statistics
            let mean_volume = volumes.iter().sum::<f64>() / volumes.len() as f64;
            let std_volume = (volumes.iter().map(|v| (v - mean_volume).powi(2)).sum::<f64>() / volumes.len() as f64).sqrt();
            
            features.push(mean_volume as f32);
            features.push(std_volume as f32);
            
            // Price trend
            if window >= 10 {
                let x: Vec<f64> = (0..window).map(|i| i as f64).collect();
                let slope = self.linear_regression(&prices, &x);
                features.push(slope as f32);
            }
        } else {
            // Padding for insufficient data
            features.extend(vec![0.0; 5]);
        }
        
        features
    }
    
    /// Extract depth profile features
    fn extract_depth_features(&self, depth: &DepthProfile) -> Vec<f32> {
        let mut features = Vec::new();
        
        // Imbalance at various depths
        features.push(depth.depth_imbalance(1) as f32);
        features.push(depth.depth_imbalance(5) as f32);
        features.push(depth.depth_imbalance(10) as f32);
        
        // VWAP features
        features.push(depth.bid_vwap() as f32);
        features.push(depth.ask_vwap() as f32);
        features.push((depth.ask_vwap() - depth.bid_vwap()) as f32);
        
        // Total depth ratio
        let total_bid = depth.total_bid_volume();
        let total_ask = depth.total_ask_volume();
        features.push((total_bid / (total_ask + 1e-8)) as f32);
        
        // Slope of first N levels
        if depth.bids.len() >= 5 {
            let bid_prices: Vec<f64> = depth.bids.iter().take(5).map(|l| l.price).collect();
            let bid_volumes: Vec<f64> = depth.bids.iter().take(5).map(|l| l.volume).collect();
            let bid_slope = self.linear_regression(&bid_volumes, &bid_prices);
            features.push(bid_slope as f32);
        } else {
            features.push(0.0);
        }
        
        if depth.asks.len() >= 5 {
            let ask_prices: Vec<f64> = depth.asks.iter().take(5).map(|l| l.price).collect();
            let ask_volumes: Vec<f64> = depth.asks.iter().take(5).map(|l| l.volume).collect();
            let ask_slope = self.linear_regression(&ask_volumes, &ask_prices);
            features.push(ask_slope as f32);
        } else {
            features.push(0.0);
        }
        
        features
    }
    
    /// Convert MarketFeatures to vector
    fn features_to_vector(&self, features: &MarketFeatures) -> Vec<f32> {
        let mut vec = Vec::new();
        vec.push(features.mid_price as f32);
        vec.push(features.spread as f32);
        vec.push(features.spread_ticks as f32);
        vec.push(features.best_bid as f32);
        vec.push(features.best_ask as f32);
        vec.push(features.bid_volume_1 as f32);
        vec.push(features.ask_volume_1 as f32);
        vec.push(features.order_imbalance_1 as f32);
        vec.push(features.order_imbalance_5 as f32);
        vec.push(features.order_imbalance_10 as f32);
        vec.push(features.return_1 as f32);
        vec.push(features.return_5 as f32);
        vec.push(features.return_10 as f32);
        vec.push(features.volatility_20 as f32);
        vec.push(features.price_momentum_10 as f32);
        vec.push(features.price_momentum_20 as f32);
        vec.push(features.hour_of_day as f32);
        vec.push(features.minute_of_hour as f32);
        vec.push(features.day_of_week as f32);
        vec
    }
    
    /// Linear regression slope
    fn linear_regression(&self, y: &[f64], x: &[f64]) -> f64 {
        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
        let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();
        
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-8 {
            0.0
        } else {
            (n * sum_xy - sum_x * sum_y) / denominator
        }
    }
    
    /// Fit normalization statistics from historical data
    pub fn fit_normalization(&mut self, feature_history: &[Vec<f32>]) {
        if feature_history.is_empty() {
            return;
        }
        
        let num_features = feature_history[0].len();
        let mut sums = vec![0.0; num_features];
        let mut sums_sq = vec![0.0; num_features];
        
        for features in feature_history {
            for (i, &val) in features.iter().enumerate() {
                sums[i] += val as f64;
                sums_sq[i] += val as f64 * val as f64;
            }
        }
        
        let n = feature_history.len() as f64;
        let mut means = vec![0.0; num_features];
        let mut stds = vec![0.0; num_features];
        
        for i in 0..num_features {
            means[i] = sums[i] / n;
            let variance = (sums_sq[i] / n) - (means[i] * means[i]);
            stds[i] = variance.sqrt().max(1e-8);
        }
        
        self.normalization_stats = NormalizationStats {
            means: means.iter().map(|&m| m as f32).collect(),
            stds: stds.iter().map(|&s| s as f32).collect(),
            fitted: true,
        };
    }
    
    /// Normalize features
    fn normalize(&self, features: &[f32]) -> Vec<f32> {
        features.iter()
            .enumerate()
            .map(|(i, &val)| {
                let mean = self.normalization_stats.means.get(i).copied().unwrap_or(0.0);
                let std = self.normalization_stats.stds.get(i).copied().unwrap_or(1.0);
                (val - mean) / std
            })
            .collect()
    }
    
    /// Generate feature names for debugging
    fn generate_feature_names(&self) -> Vec<String> {
        let mut names = vec![
            "mid_price".to_string(),
            "spread".to_string(),
            "spread_ticks".to_string(),
            "best_bid".to_string(),
            "best_ask".to_string(),
            "bid_volume_1".to_string(),
            "ask_volume_1".to_string(),
            "order_imbalance_1".to_string(),
            "order_imbalance_5".to_string(),
            "order_imbalance_10".to_string(),
            "return_1".to_string(),
            "return_5".to_string(),
            "return_10".to_string(),
            "volatility_20".to_string(),
            "price_momentum_10".to_string(),
            "price_momentum_20".to_string(),
            "hour_of_day".to_string(),
            "minute_of_hour".to_string(),
            "day_of_week".to_string(),
        ];
        
        // Add window features
        for &window in &self.config.window_sizes {
            names.push(format!("window_{}_mean_return", window));
            names.push(format!("window_{}_std_return", window));
            names.push(format!("window_{}_mean_volume", window));
            names.push(format!("window_{}_std_volume", window));
            names.push(format!("window_{}_trend", window));
        }
        
        // Add depth features
        if self.config.include_depth_features {
            names.extend(vec![
                "depth_imbalance_1".to_string(),
                "depth_imbalance_5".to_string(),
                "depth_imbalance_10".to_string(),
                "bid_vwap".to_string(),
                "ask_vwap".to_string(),
                "vwap_spread".to_string(),
                "depth_ratio".to_string(),
                "bid_slope".to_string(),
                "ask_slope".to_string(),
            ]);
        }
        
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature_extraction() {
        let config = FeatureConfig::default();
        let mut extractor = FeatureExtractor::new(config);
        
        // Add some ticks
        for i in 0..100 {
            let tick = Tick::bid(100.0 + i as f64 * 0.01, 1000.0, i * 1000, 1);
            extractor.update(&tick);
        }
        
        let features = extractor.extract(1);
        assert!(!features.features.is_empty());
        assert_eq!(features.instrument_id, 1);
    }
}
