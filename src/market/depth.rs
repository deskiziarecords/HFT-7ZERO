// ============================================================
// MARKET DEPTH ANALYSIS
// ============================================================
// Order book depth profiles
// Liquidity distribution
// Volume-weighted metrics
// ============================================================

use super::*;
use std::collections::VecDeque;

/// Depth level with cumulative volume
#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
pub struct DepthLevel {
    pub price: f64,
    pub volume: f64,
    pub cumulative_volume: f64,
    pub order_count: u32,
    pub is_bid: bool,
    _padding: [u8; 12],
}

/// Market depth profile
#[derive(Debug, Clone)]
pub struct DepthProfile {
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
    pub timestamp_ns: u64,
    pub instrument_id: u32,
}

impl DepthProfile {
    /// Create depth profile from order book
    pub fn from_order_book(book: &OrderBook, max_depth: usize) -> Self {
        let (bid_levels, ask_levels) = book.top_levels(max_depth);
        
        let mut bids = Vec::with_capacity(bid_levels.len());
        let mut cumulative = 0.0;
        
        for level in bid_levels {
            cumulative += level.volume;
            bids.push(DepthLevel {
                price: level.price,
                volume: level.volume,
                cumulative_volume: cumulative,
                order_count: level.order_count,
                is_bid: true,
                _padding: [0; 12],
            });
        }
        
        let mut asks = Vec::with_capacity(ask_levels.len());
        cumulative = 0.0;
        
        for level in ask_levels {
            cumulative += level.volume;
            asks.push(DepthLevel {
                price: level.price,
                volume: level.volume,
                cumulative_volume: cumulative,
                order_count: level.order_count,
                is_bid: false,
                _padding: [0; 12],
            });
        }
        
        Self {
            bids,
            asks,
            timestamp_ns: book.timestamp_ns(),
            instrument_id: book.instrument_id,
        }
    }
    
    /// Get total bid depth
    pub fn total_bid_volume(&self) -> f64 {
        self.bids.last().map(|l| l.cumulative_volume).unwrap_or(0.0)
    }
    
    /// Get total ask depth
    pub fn total_ask_volume(&self) -> f64 {
        self.asks.last().map(|l| l.cumulative_volume).unwrap_or(0.0)
    }
    
    /// Get volume-weighted average price (VWAP) for bids
    pub fn bid_vwap(&self) -> f64 {
        let total_vol = self.total_bid_volume();
        if total_vol == 0.0 {
            return 0.0;
        }
        
        let weighted_sum: f64 = self.bids.iter()
            .map(|l| l.price * l.volume)
            .sum();
        
        weighted_sum / total_vol
    }
    
    /// Get volume-weighted average price (VWAP) for asks
    pub fn ask_vwap(&self) -> f64 {
        let total_vol = self.total_ask_volume();
        if total_vol == 0.0 {
            return 0.0;
        }
        
        let weighted_sum: f64 = self.asks.iter()
            .map(|l| l.price * l.volume)
            .sum();
        
        weighted_sum / total_vol
    }
    
    /// Get depth at price level
    pub fn depth_at_price(&self, price: f64) -> f64 {
        // Find bid depth
        for level in &self.bids {
            if (level.price - price).abs() < 0.0001 {
                return level.volume;
            }
        }
        
        // Find ask depth
        for level in &self.asks {
            if (level.price - price).abs() < 0.0001 {
                return level.volume;
            }
        }
        
        0.0
    }
    
    /// Get cumulative depth up to price
    pub fn cumulative_depth_up_to(&self, price: f64, is_bid: bool) -> f64 {
        if is_bid {
            self.bids.iter()
                .take_while(|l| l.price >= price)
                .map(|l| l.volume)
                .sum()
        } else {
            self.asks.iter()
                .take_while(|l| l.price <= price)
                .map(|l| l.volume)
                .sum()
        }
    }
    
    /// Get depth imbalance at price level
    pub fn depth_imbalance(&self, depth_ticks: usize) -> f64 {
        let bid_depth = self.bids.iter()
            .take(depth_ticks)
            .map(|l| l.volume)
            .sum::<f64>();
        
        let ask_depth = self.asks.iter()
            .take(depth_ticks)
            .map(|l| l.volume)
            .sum::<f64>();
        
        if bid_depth + ask_depth > 0.0 {
            (bid_depth - ask_depth) / (bid_depth + ask_depth)
        } else {
            0.0
        }
    }
    
    /// Get liquidity at various price distances
    pub fn liquidity_distribution(&self, max_distance_ticks: usize, tick_size: f64) -> Vec<(f64, f64)> {
        let mut distribution = Vec::with_capacity(max_distance_ticks);
        
        for ticks in 1..=max_distance_ticks {
            let distance = ticks as f64 * tick_size;
            let bid_liquidity: f64 = self.bids.iter()
                .take_while(|l| (self.bids[0].price - l.price).abs() <= distance)
                .map(|l| l.volume)
                .sum();
            
            let ask_liquidity: f64 = self.asks.iter()
                .take_while(|l| (l.price - self.asks[0].price).abs() <= distance)
                .map(|l| l.volume)
                .sum();
            
            distribution.push((distance, bid_liquidity + ask_liquidity));
        }
        
        distribution
    }
}

/// Market depth with historical tracking
pub struct MarketDepth {
    current: DepthProfile,
    history: VecDeque<DepthProfile>,
    max_history: usize,
}

impl MarketDepth {
    pub fn new(max_history: usize) -> Self {
        Self {
            current: DepthProfile {
                bids: Vec::new(),
                asks: Vec::new(),
                timestamp_ns: 0,
                instrument_id: 0,
            },
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }
    
    pub fn update(&mut self, book: &OrderBook, max_depth: usize) {
        self.current = DepthProfile::from_order_book(book, max_depth);
        
        self.history.push_back(self.current.clone());
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
    
    pub fn current(&self) -> &DepthProfile {
        &self.current
    }
    
    pub fn history(&self) -> &VecDeque<DepthProfile> {
        &self.history
    }
    
    /// Get depth change between snapshots
    pub fn depth_change(&self, lookback: usize) -> Option<DepthChange> {
        if self.history.len() < lookback + 1 {
            return None;
        }
        
        let old = &self.history[self.history.len() - lookback - 1];
        let new = &self.current;
        
        Some(DepthChange {
            bid_volume_delta: new.total_bid_volume() - old.total_bid_volume(),
            ask_volume_delta: new.total_ask_volume() - old.total_ask_volume(),
            bid_vwap_delta: new.bid_vwap() - old.bid_vwap(),
            ask_vwap_delta: new.ask_vwap() - old.ask_vwap(),
            timestamp_ns: new.timestamp_ns,
        })
    }
}

/// Depth change between snapshots
#[derive(Debug, Clone, Copy)]
pub struct DepthChange {
    pub bid_volume_delta: f64,
    pub ask_volume_delta: f64,
    pub bid_vwap_delta: f64,
    pub ask_vwap_delta: f64,
    pub timestamp_ns: u64,
}

/// Depth features for ML models
#[derive(Debug, Clone)]
pub struct DepthFeatures {
    pub imbalance_1: f64,
    pub imbalance_5: f64,
    pub imbalance_10: f64,
    pub slope_bid_5: f64,
    pub slope_ask_5: f64,
    pub curvature_bid: f64,
    pub curvature_ask: f64,
    pub total_depth_ratio: f64,
    pub vwap_spread: f64,
}

impl DepthFeatures {
    pub fn from_profile(profile: &DepthProfile) -> Self {
        let imbalance_1 = profile.depth_imbalance(1);
        let imbalance_5 = profile.depth_imbalance(5);
        let imbalance_10 = profile.depth_imbalance(10);
        
        // Calculate slope of first 5 levels
        let slope_bid_5 = if profile.bids.len() >= 5 {
            let prices: Vec<f64> = profile.bids.iter().take(5).map(|l| l.price).collect();
            let volumes: Vec<f64> = profile.bids.iter().take(5).map(|l| l.volume).collect();
            Self::linear_slope(&prices, &volumes)
        } else {
            0.0
        };
        
        let slope_ask_5 = if profile.asks.len() >= 5 {
            let prices: Vec<f64> = profile.asks.iter().take(5).map(|l| l.price).collect();
            let volumes: Vec<f64> = profile.asks.iter().take(5).map(|l| l.volume).collect();
            Self::linear_slope(&prices, &volumes)
        } else {
            0.0
        };
        
        let total_depth_ratio = profile.total_bid_volume() / (profile.total_ask_volume() + 1e-8);
        let vwap_spread = profile.ask_vwap() - profile.bid_vwap();
        
        Self {
            imbalance_1,
            imbalance_5,
            imbalance_10,
            slope_bid_5,
            slope_ask_5,
            curvature_bid: 0.0,  // Would require more levels
            curvature_ask: 0.0,
            total_depth_ratio,
            vwap_spread,
        }
    }
    
    fn linear_slope(x: &[f64], y: &[f64]) -> f64 {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_depth_profile() {
        let mut book = OrderBook::new(1, 0.01);
        
        // Add some depth
        for i in 0..10 {
            let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0 * (10 - i) as f64, 1000, 1);
            let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0 * (10 - i) as f64, 1000, 1);
            book.update(&bid);
            book.update(&ask);
        }
        
        let profile = DepthProfile::from_order_book(&book, 10);
        
        assert_eq!(profile.bids.len(), 10);
        assert_eq!(profile.asks.len(), 10);
        assert!(profile.total_bid_volume() > 0.0);
        assert!(profile.total_ask_volume() > 0.0);
        
        let features = DepthFeatures::from_profile(&profile);
        assert!(features.imbalance_1.abs() < 1.0);
    }
}
