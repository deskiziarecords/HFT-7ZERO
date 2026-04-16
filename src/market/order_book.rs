// ============================================================
// ORDER BOOK
// ============================================================

use crate::market::tick::{Tick, TickType};
use std::collections::BTreeMap;

pub struct OrderBook {
    pub instrument_id: u32,
    pub bids: BTreeMap<u64, f64>, // Price * 10000 -> Volume
    pub asks: BTreeMap<u64, f64>,
    pub last_price: f64,
    pub timestamp_ns: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            instrument_id: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_price: 0.0,
            timestamp_ns: 0,
        }
    }
    
    pub fn update(&mut self, tick: &Tick) {
        self.timestamp_ns = tick.timestamp_ns;
        let price_key = (tick.price * 10000.0) as u64;
        
        match tick.tick_type {
            TickType::Bid => {
                if tick.volume > 0.0 {
                    self.bids.insert(price_key, tick.volume);
                } else {
                    self.bids.remove(&price_key);
                }
            }
            TickType::Ask => {
                if tick.volume > 0.0 {
                    self.asks.insert(price_key, tick.volume);
                } else {
                    self.asks.remove(&price_key);
                }
            }
            TickType::Trade => {
                self.last_price = tick.price;
            }
            _ => {}
        }
    }
    
    pub fn top_levels(&self, depth: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let bid_vec: Vec<(f64, f64)> = self.bids.iter().rev().take(depth)
            .map(|(&p, &v)| (p as f64 / 10000.0, v)).collect();
        let ask_vec: Vec<(f64, f64)> = self.asks.iter().take(depth)
            .map(|(&p, &v)| (p as f64 / 10000.0, v)).collect();
        (bid_vec, ask_vec)
    }
    
    pub fn best_bid(&self) -> f64 {
        self.bids.keys().rev().next().map(|&p| p as f64 / 10000.0).unwrap_or(0.0)
    }
    
    pub fn best_ask(&self) -> f64 {
        self.asks.keys().next().map(|&p| p as f64 / 10000.0).unwrap_or(0.0)
    }
}

impl OrderBook {
    pub fn spread(&self) -> f64 {
        self.best_ask() - self.best_bid()
    }

    pub fn mid_price(&self) -> f64 {
        (self.best_bid() + self.best_ask()) / 2.0
    }

    pub fn total_bid_volume(&self) -> f64 {
        self.bids.values().sum()
    }

    pub fn total_ask_volume(&self) -> f64 {
        self.asks.values().sum()
    }
}

pub struct OrderBookLevel {
    pub price: f64,
    pub volume: f64,
}

impl OrderBook {
    pub fn bid_at_depth(&self, depth: usize) -> Option<OrderBookLevel> {
        self.bids.iter().rev().nth(depth).map(|(&p, &v)| OrderBookLevel { price: p as f64 / 10000.0, volume: v })
    }

    pub fn ask_at_depth(&self, depth: usize) -> Option<OrderBookLevel> {
        self.asks.iter().nth(depth).map(|(&p, &v)| OrderBookLevel { price: p as f64 / 10000.0, volume: v })
    }
}
