// ============================================================
// EXECUTION MODULE
// ============================================================
// Stealth execution with anti-detection mechanisms
// Order fragmentation and randomization
// Jitter injection for timing obfuscation
// Smart order routing and venue management
// ============================================================

pub mod stealth;
pub mod fragmentation;
pub mod jitter;
pub mod order_manager;
pub mod venue_routing;
pub mod smart_router;
pub mod execution_engine;

pub use stealth::{StealthExecutor, StealthConfig, ExecutionProfile, DetectionRisk};
pub use fragmentation::{Fragmenter, FragmentConfig, OrderFragment, FragmentStrategy};
pub use jitter::{JitterGenerator, JitterConfig, JitterType, TimingObfuscator};
pub use order_manager::{OrderManager, Order, OrderStatus, OrderType, TimeInForce};
pub use venue_routing::{VenueRouter, Venue, VenueSelector, RoutingRule};
pub use smart_router::{SmartOrderRouter, RouterConfig, RouteDecision};
pub use execution_engine::{ExecutionEngine, EngineConfig, ExecutionResult};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

/// Execution configuration
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub stealth: StealthConfig,
    pub fragmentation: FragmentConfig,
    pub jitter: JitterConfig,
    pub max_slippage_pips: f64,
    pub min_volume_pct: f64,
    pub max_volume_pct: f64,
    pub venue_timeout_ms: u64,
    pub retry_attempts: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            stealth: StealthConfig::default(),
            fragmentation: FragmentConfig::default(),
            jitter: JitterConfig::default(),
            max_slippage_pips: 1.5,
            min_volume_pct: 0.01,
            max_volume_pct: 0.05,
            venue_timeout_ms: 100,
            retry_attempts: 3,
        }
    }
}

/// Execution statistics
#[derive(Debug, Default, Clone)]
pub struct ExecutionStats {
    pub total_orders: u64,
    pub filled_orders: u64,
    pub cancelled_orders: u64,
    pub rejected_orders: u64,
    pub avg_fill_time_ns: u64,
    pub avg_slippage_pips: f64,
    pub detection_probability: f64,
    pub total_volume: f64,
    pub total_value: f64,
}

/// Global execution state
pub static EXECUTION_STATE: once_cell::sync::Lazy<Arc<ExecutionState>> = 
    once_cell::sync::Lazy::new(|| Arc::new(ExecutionState::new()));

/// Execution state manager
pub struct ExecutionState {
    pub orders: DashMap<u64, Order>,
    pub stats: RwLock<ExecutionStats>,
    pub active_fragments: DashMap<u64, Vec<OrderFragment>>,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            orders: DashMap::with_capacity(10000),
            stats: RwLock::new(ExecutionStats::default()),
            active_fragments: DashMap::with_capacity(1000),
        }
    }
    
    pub fn record_order(&self, order: Order) {
        self.orders.insert(order.order_id, order);
    }
    
    pub fn update_order(&self, order_id: u64, status: OrderStatus) {
        if let Some(mut order) = self.orders.get_mut(&order_id) {
            order.status = status;
        }
    }
    
    pub fn update_stats(&self, stats: ExecutionStats) {
        *self.stats.write() = stats;
    }
    
    pub fn get_stats(&self) -> ExecutionStats {
        self.stats.read().clone()
    }
}
