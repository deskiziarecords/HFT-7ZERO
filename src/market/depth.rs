// ============================================================
// MARKET DEPTH
// ============================================================

use crate::market::order_book::OrderBook;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthLevel {
    pub price: f64,
    pub volume: f64,
    pub cumulative_volume: f64,
    pub order_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthProfile {
    pub instrument_id: u32,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
    pub imbalance: f64,
    pub timestamp_ns: u64,
}

impl DepthProfile {
    pub fn from_book(book: &OrderBook, max_depth: usize) -> Self {
        let (bid_levels, ask_levels) = book.top_levels(max_depth);
        
        let mut bids = Vec::with_capacity(bid_levels.len());
        let mut cumulative = 0.0;
        for level in bid_levels {
            cumulative += level.1;
            bids.push(DepthLevel {
                price: level.0,
                volume: level.1,
                cumulative_volume: cumulative,
                order_count: 0,
            });
        }
        
        let mut asks = Vec::with_capacity(ask_levels.len());
        cumulative = 0.0;
        for level in ask_levels {
            cumulative += level.1;
            asks.push(DepthLevel {
                price: level.0,
                volume: level.1,
                cumulative_volume: cumulative,
                order_count: 0,
            });
        }
        
        Self {
            instrument_id: book.instrument_id,
            bids,
            asks,
            imbalance: 0.0,
            timestamp_ns: book.timestamp_ns,
        }
    }
}
