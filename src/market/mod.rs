// ============================================================
// MARKET DATA MODULE
// ============================================================
// Order book management
// Tick processing
// Market depth analysis
// Liquidity tracking
// ============================================================

pub mod order_book;
pub mod tick;
pub mod depth;
pub mod liquidity;
pub mod price_level;
pub mod market_microstructure;

pub use order_book::{OrderBook, OrderBookSide, OrderBookLevel};
pub use tick::{Tick, TickType, TickFlags};
pub use depth::{DepthProfile, DepthLevel, MarketDepth};
pub use liquidity::{LiquidityPool, LiquidityMetrics, OrderFlowImbalance};
pub use price_level::{PriceLevel, LevelUpdate, LevelAction};
pub use market_microstructure::{MicrostructureFeatures, SpreadDynamics, QueuePosition};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// Market data configuration
#[derive(Debug, Clone)]
pub struct MarketConfig {
    pub max_depth_levels: usize,
    pub tick_size: f64,
    pub lot_size: f64,
    pub exchange_id: u8,
    pub use_ws_feed: bool,
    pub snapshot_interval_ms: u64,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            max_depth_levels: 100,
            tick_size: 0.01,
            lot_size: 1.0,
            exchange_id: 0,
            use_ws_feed: false,
            snapshot_interval_ms: 100,
        }
    }
}

/// Global market state
pub struct MarketState {
    pub order_books: DashMap<u32, Arc<RwLock<OrderBook>>>,
    pub tick_history: DashMap<u32, VecDeque<Tick>>,
    pub config: MarketConfig,
}

impl MarketState {
    pub fn new(config: MarketConfig) -> Self {
        Self {
            order_books: DashMap::new(),
            tick_history: DashMap::new(),
            config,
        }
    }
    
    pub fn get_order_book(&self, instrument_id: u32) -> Option<Arc<RwLock<OrderBook>>> {
        self.order_books.get(&instrument_id).map(|book| book.clone())
    }
    
    pub fn update_tick(&self, instrument_id: u32, tick: Tick) {
        // Update order book
        if let Some(book) = self.get_order_book(instrument_id) {
            let mut book_guard = book.write();
            book_guard.update(&tick);
        }
        
        // Update tick history
        let mut history = self.tick_history
            .entry(instrument_id)
            .or_insert_with(|| VecDeque::with_capacity(10000));
        
        history.push_back(tick);
        while history.len() > 10000 {
            history.pop_front();
        }
    }
}

/// Market event types for pub/sub
#[derive(Debug, Clone)]
pub enum MarketEvent {
    Tick(Tick),
    Snapshot(OrderBook),
    Trade(f64, f64),  // price, volume
    DepthUpdate(usize, PriceLevel),
    SpreadChange(f64, f64),  // old_spread, new_spread
}

/// Market data subscriber trait
pub trait MarketSubscriber: Send + Sync {
    fn on_event(&self, event: &MarketEvent);
    fn on_batch(&self, events: &[MarketEvent]);
}

/// Event bus for market data
pub struct MarketEventBus {
    subscribers: DashMap<String, Vec<Box<dyn MarketSubscriber>>>,
}

impl MarketEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: DashMap::new(),
        }
    }
    
    pub fn subscribe(&self, name: String, subscriber: Box<dyn MarketSubscriber>) {
        self.subscribers.entry(name).or_insert_with(Vec::new).push(subscriber);
    }
    
    pub fn publish(&self, event: MarketEvent) {
        for subscribers in self.subscribers.iter() {
            for subscriber in subscribers.value() {
                subscriber.on_event(&event);
            }
        }
    }
    
    pub fn publish_batch(&self, events: &[MarketEvent]) {
        for subscribers in self.subscribers.iter() {
            for subscriber in subscribers.value() {
                subscriber.on_batch(events);
            }
        }
    }
}
