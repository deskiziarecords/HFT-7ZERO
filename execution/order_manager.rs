// ============================================================
// ORDER MANAGER
// ============================================================
// Order lifecycle management
// State tracking and persistence
// Fill monitoring and reporting
// ============================================================

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::utils::time::get_hardware_timestamp;

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    Iceberg,
    Pegged,
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
    BuySell,  // Two-sided order
}

/// Time in force
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    Day,
    GoodTillCancel,
    ImmediateOrCancel,
    FillOrKill,
    GoodTillDate(u64),  // Timestamp
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
    Suspended,
}

/// Main order structure
#[derive(Debug, Clone)]
pub struct Order {
    pub order_id: u64,
    pub client_order_id: u64,
    pub instrument_id: u32,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub volume: f64,
    pub filled_volume: f64,
    pub limit_price: f64,
    pub stop_price: f64,
    pub time_in_force: TimeInForce,
    pub status: OrderStatus,
    pub created_at_ns: u64,
    pub updated_at_ns: u64,
    pub filled_at_ns: Option<u64>,
    pub avg_fill_price: f64,
    pub venue: String,
    pub tags: HashMap<String, String>,
    pub expected_slippage: f64,
    pub tick_size: f64,
    pub cancel_after_ms: Option<u64>,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            order_id: 0,
            client_order_id: 0,
            instrument_id: 0,
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            volume: 0.0,
            filled_volume: 0.0,
            limit_price: 0.0,
            stop_price: 0.0,
            time_in_force: TimeInForce::Day,
            status: OrderStatus::New,
            created_at_ns: get_hardware_timestamp(),
            updated_at_ns: get_hardware_timestamp(),
            filled_at_ns: None,
            avg_fill_price: 0.0,
            venue: String::new(),
            tags: HashMap::new(),
            expected_slippage: 0.0,
            tick_size: 0.01,
            cancel_after_ms: None,
        }
    }
}

impl Order {
    /// Create new buy order
    pub fn buy(instrument_id: u32, volume: f64, limit_price: f64) -> Self {
        Self {
            order_id: Self::generate_id(),
            client_order_id: Self::generate_id(),
            instrument_id,
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            volume,
            limit_price,
            ..Default::default()
        }
    }
    
    /// Create new sell order
    pub fn sell(instrument_id: u32, volume: f64, limit_price: f64) -> Self {
        Self {
            order_id: Self::generate_id(),
            client_order_id: Self::generate_id(),
            instrument_id,
            order_type: OrderType::Limit,
            side: OrderSide::Sell,
            volume,
            limit_price,
            ..Default::default()
        }
    }
    
    /// Generate unique order ID
    fn generate_id() -> u64 {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }
    
    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.filled_volume >= self.volume - 1e-8
    }
    
    /// Get remaining volume
    pub fn remaining_volume(&self) -> f64 {
        (self.volume - self.filled_volume).max(0.0)
    }
    
    /// Update fill
    pub fn add_fill(&mut self, fill_volume: f64, fill_price: f64) {
        let total_value = self.avg_fill_price * self.filled_volume + fill_price * fill_volume;
        self.filled_volume += fill_volume;
        self.avg_fill_price = total_value / self.filled_volume;
        self.updated_at_ns = get_hardware_timestamp();
        
        if self.is_filled() {
            self.status = OrderStatus::Filled;
            self.filled_at_ns = Some(self.updated_at_ns);
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }
    
    /// Cancel order
    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
        self.updated_at_ns = get_hardware_timestamp();
    }
    
    /// Calculate slippage
    pub fn slippage(&self, reference_price: f64) -> f64 {
        let execution_price = if self.filled_volume > 0.0 {
            self.avg_fill_price
        } else {
            self.limit_price
        };
        
        match self.side {
            OrderSide::Buy => (execution_price - reference_price).abs(),
            OrderSide::Sell => (reference_price - execution_price).abs(),
            _ => 0.0,
        }
    }
}

/// Order manager for tracking all orders
pub struct OrderManager {
    orders: DashMap<u64, Order>,
    active_orders: DashMap<u64, Order>,
    filled_orders: DashMap<u64, Order>,
    cancelled_orders: DashMap<u64, Order>,
    stats: RwLock<OrderManagerStats>,
    max_history: usize,
}

/// Order manager statistics
#[derive(Debug, Default, Clone)]
pub struct OrderManagerStats {
    pub total_orders: u64,
    pub active_orders: u64,
    pub filled_orders: u64,
    pub cancelled_orders: u64,
    pub rejected_orders: u64,
    pub avg_fill_time_ns: u64,
    pub total_volume: f64,
    pub total_value: f64,
}

impl OrderManager {
    /// Create new order manager
    pub fn new(max_history: usize) -> Self {
        Self {
            orders: DashMap::with_capacity(max_history),
            active_orders: DashMap::with_capacity(1000),
            filled_orders: DashMap::with_capacity(max_history),
            cancelled_orders: DashMap::with_capacity(max_history),
            stats: RwLock::new(OrderManagerStats::default()),
            max_history,
        }
    }
    
    /// Submit new order
    pub fn submit(&self, mut order: Order) -> u64 {
        order.status = OrderStatus::Pending;
        order.created_at_ns = get_hardware_timestamp();
        
        self.orders.insert(order.order_id, order.clone());
        self.active_orders.insert(order.order_id, order);
        
        self.update_stats();
        order.order_id
    }
    
    /// Accept order (sent to venue)
    pub fn accept(&self, order_id: u64) -> bool {
        if let Some(mut order) = self.active_orders.get_mut(&order_id) {
            order.status = OrderStatus::Open;
            order.updated_at_ns = get_hardware_timestamp();
            self.update_stats();
            true
        } else {
            false
        }
    }
    
    /// Update order fill
    pub fn fill(&self, order_id: u64, fill_volume: f64, fill_price: f64) -> bool {
        if let Some(mut order) = self.orders.get_mut(&order_id) {
            order.add_fill(fill_volume, fill_price);
            
            if order.is_filled() {
                // Move from active to filled
                self.active_orders.remove(&order_id);
                self.filled_orders.insert(order_id, order.clone());
            }
            
            self.update_stats();
            true
        } else {
            false
        }
    }
    
    /// Cancel order
    pub fn cancel(&self, order_id: u64) -> bool {
        if let Some(mut order) = self.active_orders.get_mut(&order_id) {
            order.cancel();
            self.active_orders.remove(&order_id);
            self.cancelled_orders.insert(order_id, order.clone());
            self.update_stats();
            true
        } else {
            false
        }
    }
    
    /// Reject order
    pub fn reject(&self, order_id: u64, reason: &str) {
        if let Some(mut order) = self.orders.get_mut(&order_id) {
            order.status = OrderStatus::Rejected;
            order.tags.insert("reject_reason".to_string(), reason.to_string());
            self.active_orders.remove(&order_id);
            self.update_stats();
        }
    }
    
    /// Get order by ID
    pub fn get_order(&self, order_id: u64) -> Option<Order> {
        self.orders.get(&order_id).map(|o| o.clone())
    }
    
    /// Get all active orders
    pub fn get_active_orders(&self) -> Vec<Order> {
        self.active_orders.iter().map(|entry| entry.value().clone()).collect()
    }
    
    /// Get orders for instrument
    pub fn get_orders_for_instrument(&self, instrument_id: u32) -> Vec<Order> {
        self.orders.iter()
            .filter(|entry| entry.instrument_id == instrument_id)
            .map(|entry| entry.clone())
            .collect()
    }
    
    /// Get total exposure
    pub fn total_exposure(&self) -> f64 {
        self.active_orders.iter()
            .map(|entry| entry.remaining_volume() * entry.limit_price)
            .sum()
    }
    
    /// Cancel all orders for instrument
    pub fn cancel_all(&self, instrument_id: u32) -> usize {
        let mut cancelled = 0;
        let to_cancel: Vec<u64> = self.active_orders.iter()
            .filter(|entry| entry.instrument_id == instrument_id)
            .map(|entry| *entry.key())
            .collect();
        
        for order_id in to_cancel {
            if self.cancel(order_id) {
                cancelled += 1;
            }
        }
        
        cancelled
    }
    
    /// Update statistics
    fn update_stats(&self) {
        let mut stats = self.stats.write();
        
        stats.active_orders = self.active_orders.len() as u64;
        stats.filled_orders = self.filled_orders.len() as u64;
        stats.cancelled_orders = self.cancelled_orders.len() as u64;
        stats.total_orders = self.orders.len() as u64;
        
        // Calculate average fill time
        let fill_times: Vec<u64> = self.filled_orders.iter()
            .filter_map(|entry| {
                entry.filled_at_ns.map(|filled| filled - entry.created_at_ns)
            })
            .collect();
        
        if !fill_times.is_empty() {
            stats.avg_fill_time_ns = fill_times.iter().sum::<u64>() / fill_times.len() as u64;
        }
        
        // Calculate total volume and value
        stats.total_volume = self.filled_orders.iter()
            .map(|entry| entry.filled_volume)
            .sum();
        
        stats.total_value = self.filled_orders.iter()
            .map(|entry| entry.avg_fill_price * entry.filled_volume)
            .sum();
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> OrderManagerStats {
        self.stats.read().clone()
    }
    
    /// Clean old orders
    pub fn cleanup(&self, older_than_ns: u64) -> usize {
        let now = get_hardware_timestamp();
        let cutoff = now - older_than_ns;
        
        let mut removed = 0;
        let to_remove: Vec<u64> = self.orders.iter()
            .filter(|entry| entry.updated_at_ns < cutoff)
            .map(|entry| *entry.key())
            .collect();
        
        for order_id in to_remove {
            if self.orders.remove(&order_id).is_some() {
                removed += 1;
            }
            self.filled_orders.remove(&order_id);
            self.cancelled_orders.remove(&order_id);
        }
        
        removed
    }
    
    /// Check for expired orders
    pub fn check_expirations(&self) -> Vec<u64> {
        let now = get_hardware_timestamp();
        let mut expired = Vec::new();
        
        for entry in self.active_orders.iter() {
            let order = entry.value();
            let should_expire = match order.time_in_force {
                TimeInForce::Day => {
                    // Expire at end of trading day (simplified)
                    let day_ns = 24 * 60 * 60 * 1_000_000_000u64;
                    now - order.created_at_ns > day_ns
                }
                TimeInForce::GoodTillDate(expiry) => now > expiry,
                TimeInForce::ImmediateOrCancel => true,
                TimeInForce::FillOrKill => !order.is_filled(),
                _ => false,
            };
            
            if should_expire && !order.is_filled() {
                expired.push(order.order_id);
            }
        }
        
        for order_id in &expired {
            if let Some(mut order) = self.orders.get_mut(order_id) {
                order.status = OrderStatus::Expired;
                self.active_orders.remove(order_id);
            }
        }
        
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_lifecycle() {
        let manager = OrderManager::new(1000);
        
        let order = Order::buy(1, 1.0, 100.0);
        let order_id = manager.submit(order);
        
        assert!(manager.accept(order_id));
        
        manager.fill(order_id, 0.5, 100.0);
        let order = manager.get_order(order_id).unwrap();
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.filled_volume, 0.5);
        
        manager.fill(order_id, 0.5, 100.0);
        let order = manager.get_order(order_id).unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        
        let stats = manager.get_stats();
        assert_eq!(stats.filled_orders, 1);
    }
    
    #[test]
    fn test_cancel_orders() {
        let manager = OrderManager::new(1000);
        
        let order1 = Order::buy(1, 1.0, 100.0);
        let order2 = Order::sell(1, 1.0, 101.0);
        
        manager.submit(order1);
        manager.submit(order2);
        
        assert_eq!(manager.get_active_orders().len(), 2);
        
        manager.cancel_all(1);
        assert_eq!(manager.get_active_orders().len(), 0);
        
        let stats = manager.get_stats();
        assert_eq!(stats.cancelled_orders, 2);
    }
}
