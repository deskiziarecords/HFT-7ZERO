// ============================================================
// ORDER BOOK IMPLEMENTATION
// ============================================================
// Lock-free order book with O(log N) operations
// Full depth with price levels
// Real-time updates with minimal latency
// ============================================================

use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::memory::cache_aligned::CacheAligned;

/// Order book side (bid or ask)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderBookSide {
    Bid,
    Ask,
}

/// Order book level (price and volume)
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct OrderBookLevel {
    pub price: f64,
    pub volume: f64,
    pub order_count: u32,
    pub is_bid: bool,
    pub exchange_id: u8,
    pub timestamp_ns: u64,
    _padding: [u8; 30],
}

impl Default for OrderBookLevel {
    fn default() -> Self {
        Self {
            price: 0.0,
            volume: 0.0,
            order_count: 0,
            is_bid: false,
            exchange_id: 0,
            timestamp_ns: 0,
            _padding: [0; 30],
        }
    }
}

/// Main order book structure
#[repr(C, align(64))]
pub struct OrderBook {
    bids: BTreeMap<u64, OrderBookLevel>,  // Price encoded as fixed-point
    asks: BTreeMap<u64, OrderBookLevel>,
    best_bid: CacheAligned<AtomicU64>,
    best_ask: CacheAligned<AtomicU64>,
    bid_depth: CacheAligned<AtomicU64>,
    ask_depth: CacheAligned<AtomicU64>,
    spread_ticks: CacheAligned<AtomicU64>,
    sequence: CacheAligned<AtomicU64>,
    timestamp_ns: CacheAligned<AtomicU64>,
    instrument_id: u32,
    tick_size: f64,
    price_scale: f64,
}

impl OrderBook {
    /// Create new order book
    pub fn new(instrument_id: u32, tick_size: f64) -> Self {
        let price_scale = 1.0 / tick_size;
        
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            best_bid: CacheAligned::new(AtomicU64::new(0)),
            best_ask: CacheAligned::new(AtomicU64::new(u64::MAX)),
            bid_depth: CacheAligned::new(AtomicU64::new(0)),
            ask_depth: CacheAligned::new(AtomicU64::new(0)),
            spread_ticks: CacheAligned::new(AtomicU64::new(0)),
            sequence: CacheAligned::new(AtomicU64::new(0)),
            timestamp_ns: CacheAligned::new(AtomicU64::new(0)),
            instrument_id,
            tick_size,
            price_scale,
        }
    }
    
    /// Convert price to fixed-point key
    #[inline(always)]
    fn price_to_key(&self, price: f64) -> u64 {
        (price * self.price_scale) as u64
    }
    
    /// Convert key back to price
    #[inline(always)]
    fn key_to_price(&self, key: u64) -> f64 {
        key as f64 * self.tick_size
    }
    
    /// Update order book with tick
    pub fn update(&mut self, tick: &Tick) {
        self.sequence.fetch_add(1, Ordering::Release);
        self.timestamp_ns.store(tick.timestamp_ns, Ordering::Release);
        
        match tick.tick_type {
            TickType::Bid => self.update_bid(tick),
            TickType::Ask => self.update_ask(tick),
            TickType::Trade => self.update_trade(tick),
            TickType::Snapshot => self.update_snapshot(tick),
        }
        
        self.update_best_prices();
    }
    
    fn update_bid(&mut self, tick: &Tick) {
        let key = self.price_to_key(tick.price);
        
        if tick.volume == 0.0 {
            // Remove level
            self.bids.remove(&key);
        } else {
            // Add or update level
            self.bids.insert(key, OrderBookLevel {
                price: tick.price,
                volume: tick.volume,
                order_count: 1,
                is_bid: true,
                exchange_id: tick.exchange_id,
                timestamp_ns: tick.timestamp_ns,
                _padding: [0; 30],
            });
        }
    }
    
    fn update_ask(&mut self, tick: &Tick) {
        let key = self.price_to_key(tick.price);
        
        if tick.volume == 0.0 {
            self.asks.remove(&key);
        } else {
            self.asks.insert(key, OrderBookLevel {
                price: tick.price,
                volume: tick.volume,
                order_count: 1,
                is_bid: false,
                exchange_id: tick.exchange_id,
                timestamp_ns: tick.timestamp_ns,
                _padding: [0; 30],
            });
        }
    }
    
    fn update_trade(&mut self, tick: &Tick) {
        // Trade doesn't change order book, but updates last trade price
        // Could be used for VWAP calculations
    }
    
    fn update_snapshot(&mut self, tick: &Tick) {
        // Full order book snapshot
        // Parse and replace entire book
        // Implementation depends on exchange protocol
    }
    
    fn update_best_prices(&mut self) {
        // Update best bid
        if let Some((&key, _)) = self.bids.iter().rev().next() {
            self.best_bid.get().store(key, Ordering::Release);
        } else {
            self.best_bid.get().store(0, Ordering::Release);
        }
        
        // Update best ask
        if let Some((&key, _)) = self.asks.iter().next() {
            self.best_ask.get().store(key, Ordering::Release);
        } else {
            self.best_ask.get().store(u64::MAX, Ordering::Release);
        }
        
        // Update spread
        let best_bid = self.best_bid.get().load(Ordering::Acquire);
        let best_ask = self.best_ask.get().load(Ordering::Acquire);
        
        if best_bid > 0 && best_ask < u64::MAX {
            self.spread_ticks.get().store(best_ask - best_bid, Ordering::Release);
        }
        
        // Update depth
        let bid_depth: u64 = self.bids.values().map(|l| l.volume as u64).sum();
        let ask_depth: u64 = self.asks.values().map(|l| l.volume as u64).sum();
        
        self.bid_depth.get().store(bid_depth, Ordering::Release);
        self.ask_depth.get().store(ask_depth, Ordering::Release);
    }
    
    /// Get best bid price
    pub fn best_bid(&self) -> f64 {
        let key = self.best_bid.get().load(Ordering::Acquire);
        if key > 0 {
            self.key_to_price(key)
        } else {
            0.0
        }
    }
    
    /// Get best ask price
    pub fn best_ask(&self) -> f64 {
        let key = self.best_ask.get().load(Ordering::Acquire);
        if key < u64::MAX {
            self.key_to_price(key)
        } else {
            f64::INFINITY
        }
    }
    
    /// Get current spread
    pub fn spread(&self) -> f64 {
        let spread_ticks = self.spread_ticks.get().load(Ordering::Acquire);
        spread_ticks as f64 * self.tick_size
    }
    
    /// Get spread in ticks
    pub fn spread_ticks(&self) -> u64 {
        self.spread_ticks.get().load(Ordering::Acquire)
    }
    
    /// Get mid price
    pub fn mid_price(&self) -> f64 {
        (self.best_bid() + self.best_ask()) / 2.0
    }
    
    /// Get bid depth at level
    pub fn bid_at_depth(&self, depth: usize) -> Option<OrderBookLevel> {
        self.bids.values().rev().nth(depth).copied()
    }
    
    /// Get ask depth at level
    pub fn ask_at_depth(&self, depth: usize) -> Option<OrderBookLevel> {
        self.asks.values().nth(depth).copied()
    }
    
    /// Get total bid volume
    pub fn total_bid_volume(&self) -> f64 {
        self.bids.values().map(|l| l.volume).sum()
    }
    
    /// Get total ask volume
    pub fn total_ask_volume(&self) -> f64 {
        self.asks.values().map(|l| l.volume).sum()
    }
    
    /// Get order imbalance
    pub fn order_imbalance(&self) -> f64 {
        let bid_vol = self.total_bid_volume();
        let ask_vol = self.total_ask_volume();
        
        if bid_vol + ask_vol > 0.0 {
            (bid_vol - ask_vol) / (bid_vol + ask_vol)
        } else {
            0.0
        }
    }
    
    /// Get weighted mid price (by volume)
    pub fn weighted_mid(&self) -> f64 {
        let bid_vol = self.total_bid_volume();
        let ask_vol = self.total_ask_volume();
        let total_vol = bid_vol + ask_vol;
        
        if total_vol > 0.0 {
            (self.best_bid() * ask_vol + self.best_ask() * bid_vol) / total_vol
        } else {
            self.mid_price()
        }
    }
    
    /// Get top N levels (for feature extraction)
    pub fn top_levels(&self, n: usize) -> (Vec<OrderBookLevel>, Vec<OrderBookLevel>) {
        let bids: Vec<OrderBookLevel> = self.bids.values().rev().take(n).copied().collect();
        let asks: Vec<OrderBookLevel> = self.asks.values().take(n).copied().collect();
        (bids, asks)
    }
    
    /// Clear order book
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.best_bid.get().store(0, Ordering::Release);
        self.best_ask.get().store(u64::MAX, Ordering::Release);
        self.bid_depth.get().store(0, Ordering::Release);
        self.ask_depth.get().store(0, Ordering::Release);
    }
    
    /// Get sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }
    
    /// Get timestamp
    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns.load(Ordering::Acquire)
    }
}

/// Order book side for updates
pub enum OrderBookSideUpdate {
    BidAdd { price: f64, volume: f64, order_id: u64 },
    BidRemove { price: f64, order_id: u64 },
    BidUpdate { price: f64, volume: f64, order_id: u64 },
    AskAdd { price: f64, volume: f64, order_id: u64 },
    AskRemove { price: f64, order_id: u64 },
    AskUpdate { price: f64, volume: f64, order_id: u64 },
    Trade { price: f64, volume: f64, aggressor_side: OrderBookSide },
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_book_basic() {
        let mut book = OrderBook::new(1, 0.01);
        
        // Add bids
        let bid_tick = Tick {
            price: 100.00,
            volume: 1000.0,
            timestamp_ns: 1000,
            exchange_id: 1,
            side: 0,
            tick_type: TickType::Bid,
            sequence: 1,
        };
        book.update(&bid_tick);
        
        assert_eq!(book.best_bid(), 100.00);
        
        // Add asks
        let ask_tick = Tick {
            price: 100.05,
            volume: 1000.0,
            timestamp_ns: 1000,
            exchange_id: 1,
            side: 1,
            tick_type: TickType::Ask,
            sequence: 2,
        };
        book.update(&ask_tick);
        
        assert_eq!(book.best_ask(), 100.05);
        assert_eq!(book.spread(), 0.05);
        assert_eq!(book.mid_price(), 100.025);
    }
    
    #[test]
    fn test_order_imbalance() {
        let mut book = OrderBook::new(1, 0.01);
        
        // Heavy bid side
        for i in 0..10 {
            let tick = Tick {
                price: 100.00 - i as f64 * 0.01,
                volume: 1000.0,
                timestamp_ns: 1000,
                exchange_id: 1,
                side: 0,
                tick_type: TickType::Bid,
                sequence: i,
            };
            book.update(&tick);
        }
        
        // Light ask side
        for i in 0..5 {
            let tick = Tick {
                price: 100.05 + i as f64 * 0.01,
                volume: 100.0,
                timestamp_ns: 1000,
                exchange_id: 1,
                side: 1,
                tick_type: TickType::Ask,
                sequence: 10 + i,
            };
            book.update(&tick);
        }
        
        // Should be positive imbalance (more bids)
        assert!(book.order_imbalance() > 0.0);
    }
}
