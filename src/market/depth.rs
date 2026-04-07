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
            (bid_depth - ask_depth) / (bid_depth
