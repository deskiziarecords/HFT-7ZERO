// ============================================================
// STEALTH EXECUTION ENGINE
// ============================================================
// Anti-detection mechanisms for HFT
// Detection probability tracking (ℙ(detect) ≈ 0)
// Adaptive stealth based on market conditions
// ============================================================

use super::*;
use crate::market::OrderBook;
use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Stealth configuration
#[derive(Debug, Clone)]
pub struct StealthConfig {
    pub detection_threshold: f64,      // Target detection probability (≈0)
    pub max_participation_rate: f64,   // Max % of market volume
    pub min_iceberg_size: f64,         // Minimum iceberg chunk
    pub random_cancel_rate: f64,       // Rate of random cancellations
    pub spoofing_protection: bool,     // Anti-spoofing detection
    pub volume_shaping: bool,          // Volume profile matching
    pub time_randomization: bool,      // Random execution timing
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            detection_threshold: 0.001,  // 0.1% detection target
            max_participation_rate: 0.02, // 2% of market volume
            min_iceberg_size: 0.1,        // 0.1 lots minimum
            random_cancel_rate: 0.05,     // 5% random cancellations
            spoofing_protection: true,
            volume_shaping: true,
            time_randomization: true,
        }
    }
}

/// Execution profile types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionProfile {
    Stealth,        // Maximum stealth, slower execution
    Aggressive,     // Faster execution, higher detection risk
    Adaptive,       // Adapts to market conditions
    Passive,        // Only passive orders
    Iceberg,        // Iceberg orders only
}

/// Detection risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionRisk {
    None,       // ℙ ≈ 0
    Low,        // ℙ < 0.01
    Medium,     // ℙ < 0.05
    High,       // ℙ < 0.10
    Critical,   // ℙ > 0.10
}

/// Stealth execution engine
pub struct StealthExecutor {
    config: StealthConfig,
    profile: ExecutionProfile,
    detection_risk: AtomicU64,  // Stored as fixed-point (0-1000)
    orders_executed: AtomicU64,
    total_volume: AtomicU64,
    rng: fastrand::Rng,
    last_risk_update: Instant,
}

impl StealthExecutor {
    /// Create new stealth executor
    pub fn new(config: StealthConfig) -> Self {
        Self {
            config,
            profile: ExecutionProfile::Stealth,
            detection_risk: AtomicU64::new(0),
            orders_executed: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
            rng: fastrand::Rng::new(),
            last_risk_update: Instant::now(),
        }
    }
    
    /// Check if execution is allowed based on stealth constraints
    /// Gate Open conditions: V ∈ [0.01, 0.05], Δp_slip ≤ [0.5, 1.5] pips
    pub fn gate_check(&self, volume: f64, slippage_pips: f64, book: &OrderBook) -> bool {
        // Volume constraint
        let min_volume = self.config.min_iceberg_size.max(0.01);
        let max_volume = 0.05;
        
        if volume < min_volume || volume > max_volume {
            tracing::debug!("Volume out of range: {:.4}", volume);
            return false;
        }
        
        // Slippage constraint
        let min_slippage = 0.5;
        let max_slippage = 1.5;
        
        if slippage_pips < min_slippage || slippage_pips > max_slippage {
            tracing::debug!("Slippage out of range: {:.2} pips", slippage_pips);
            return false;
        }
        
        // Participation rate constraint
        let market_volume = book.total_bid_volume() + book.total_ask_volume();
        let participation_rate = volume / (market_volume + 1e-8);
        
        if participation_rate > self.config.max_participation_rate {
            tracing::debug!("Participation rate too high: {:.4}", participation_rate);
            return false;
        }
        
        // Detection risk check
        if self.current_detection_risk() >= DetectionRisk::Medium {
            tracing::warn!("Detection risk too high, blocking execution");
            return false;
        }
        
        true
    }
    
    /// Execute order with stealth techniques
    pub fn execute(&mut self, order: &mut Order, book: &OrderBook) -> ExecutionResult {
        let start = Instant::now();
        
        // Apply stealth modifications
        self.apply_stealth_modifications(order);
        
        // Check gate
        if !self.gate_check(order.volume, order.expected_slippage, book) {
            return ExecutionResult::Rejected("Gate conditions not met".to_string());
        }
        
        // Determine execution strategy based on profile
        let result = match self.profile {
            ExecutionProfile::Stealth => self.execute_stealth(order, book),
            ExecutionProfile::Aggressive => self.execute_aggressive(order, book),
            ExecutionProfile::Adaptive => self.execute_adaptive(order, book),
            ExecutionProfile::Passive => self.execute_passive(order, book),
            ExecutionProfile::Iceberg => self.execute_iceberg(order, book),
        };
        
        // Update statistics
        if result.is_success() {
            self.orders_executed.fetch_add(1, Ordering::Relaxed);
            self.total_volume.fetch_add((order.volume * 1000.0) as u64, Ordering::Relaxed);
            self.update_detection_risk();
        }
        
        let elapsed = start.elapsed();
        tracing::debug!("Stealth execution completed in {:?}", elapsed);
        
        result
    }
    
    /// Apply stealth modifications to order
    fn apply_stealth_modifications(&self, order: &mut Order) {
        // Randomize order size slightly
        if self.config.volume_shaping {
            let noise = 1.0 + (self.rng.f64() - 0.5) * 0.1;
            order.volume = (order.volume * noise).max(0.01).min(0.05);
        }
        
        // Randomize price (within tick size)
        let tick_noise = (self.rng.f64() - 0.5) * 0.5;
        order.limit_price += tick_noise * order.tick_size;
        
        // Random cancel probability
        if self.config.random_cancel_rate > 0.0 {
            if self.rng.f64() < self.config.random_cancel_rate {
                order.cancel_after_ms = Some(self.rng.u64(10..100));
            }
        }
    }
    
    /// Stealth execution (maximally covert)
    fn execute_stealth(&mut self, order: &Order, book: &OrderBook) -> ExecutionResult {
        // Use smallest possible chunks
        let chunk_size = order.volume * 0.1;
        let num_chunks = (order.volume / chunk_size).ceil() as usize;
        
        let mut fragments = Vec::with_capacity(num_chunks);
        for i in 0..num_chunks {
            let volume = if i == num_chunks - 1 {
                order.volume - (num_chunks - 1) as f64 * chunk_size
            } else {
                chunk_size
            };
            
            fragments.push(OrderFragment {
                fragment_id: self.rng.u64(..),
                volume,
                price: order.limit_price + (self.rng.f64() - 0.5) * order.tick_size,
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                delay_us: self.rng.u64(50..500),  // Random jitter
                venue: self.select_venue(book),
            });
        }
        
        ExecutionResult::StealthFragments(fragments)
    }
    
    /// Aggressive execution (higher detection risk)
    fn execute_aggressive(&mut self, order: &Order, book: &OrderBook) -> ExecutionResult {
        // Larger chunks, faster execution
        let chunk_size = order.volume * 0.5;
        let num_chunks = 2;
        
        let mut fragments = Vec::with_capacity(num_chunks);
        for i in 0..num_chunks {
            let volume = if i == num_chunks - 1 {
                order.volume - chunk_size
            } else {
                chunk_size
            };
            
            fragments.push(OrderFragment {
                fragment_id: self.rng.u64(..),
                volume,
                price: order.limit_price,
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                delay_us: self.rng.u64(10..100),  // Less jitter
                venue: self.select_venue(book),
            });
        }
        
        ExecutionResult::AggressiveFragments(fragments)
    }
    
    /// Adaptive execution based on market conditions
    fn execute_adaptive(&mut self, order: &Order, book: &OrderBook) -> ExecutionResult {
        // Adjust based on volatility and liquidity
        let spread = book.spread();
        let volatility = self.estimate_volatility(book);
        
        let chunk_size = if volatility > 0.001 {
            order.volume * 0.05  // More fragments in high volatility
        } else if spread < 0.01 {
            order.volume * 0.2   // Larger chunks in liquid markets
        } else {
            order.volume * 0.1
        };
        
        let num_chunks = (order.volume / chunk_size).ceil() as usize;
        
        let mut fragments = Vec::with_capacity(num_chunks);
        for i in 0..num_chunks {
            let volume = if i == num_chunks - 1 {
                order.volume - (num_chunks - 1) as f64 * chunk_size
            } else {
                chunk_size
            };
            
            // Adaptive jitter based on market noise
            let jitter_us = if volatility > 0.001 {
                self.rng.u64(100..1000)  // More jitter in volatile markets
            } else {
                self.rng.u64(20..200)
            };
            
            fragments.push(OrderFragment {
                fragment_id: self.rng.u64(..),
                volume,
                price: order.limit_price + (self.rng.f64() - 0.5) * order.tick_size * 0.5,
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                delay_us: jitter_us,
                venue: self.select_venue(book),
            });
        }
        
        ExecutionResult::AdaptiveFragments(fragments)
    }
    
    /// Passive execution (only limit orders)
    fn execute_passive(&mut self, order: &Order, book: &OrderBook) -> ExecutionResult {
        // Place at better price to increase fill probability
        let better_price = if order.side == 0 {  // Buy
            book.best_bid()
        } else {  // Sell
            book.best_ask()
        };
        
        let fragments = vec![OrderFragment {
            fragment_id: self.rng.u64(..),
            volume: order.volume,
            price: better_price,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            delay_us: 0,
            venue: self.select_venue(book),
        }];
        
        ExecutionResult::PassiveOrder(fragments)
    }
    
    /// Iceberg execution (visible tip, hidden remainder)
    fn execute_iceberg(&mut self, order: &Order, book: &OrderBook) -> ExecutionResult {
        let tip_size = self.config.min_iceberg_size.max(order.volume * 0.1);
        let remaining = order.volume - tip_size;
        
        let mut fragments = vec![OrderFragment {
            fragment_id: self.rng.u64(..),
            volume: tip_size,
            price: order.limit_price,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            delay_us: 0,
            venue: self.select_venue(book),
        }];
        
        // Hidden remainder to be executed later
        if remaining > 0.0 {
            fragments.push(OrderFragment {
                fragment_id: self.rng.u64(..),
                volume: remaining,
                price: order.limit_price,
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                delay_us: self.rng.u64(100..1000),
                venue: self.select_venue(book),
            });
        }
        
        ExecutionResult::IcebergOrder(fragments)
    }
    
    /// Select optimal venue for execution
    fn select_venue(&self, book: &OrderBook) -> String {
        // Simplified venue selection
        // In production, would consider liquidity, fees, latency
        let venues = ["NYSE", "NASDAQ", "CME", "ICE", "LSE"];
        let idx = self.rng.usize(0..venues.len());
        venues[idx].to_string()
    }
    
    /// Estimate current market volatility
    fn estimate_volatility(&self, book: &OrderBook) -> f64 {
        // Simplified volatility estimate from spread
        book.spread() / book.mid_price()
    }
    
    /// Update detection risk estimate
    fn update_detection_risk(&self) {
        // Risk factors:
        // - Order frequency
        // - Volume concentration
        // - Pattern regularity
        // - Time correlation
        
        let orders = self.orders_executed.load(Ordering::Relaxed);
        let volume = self.total_volume.load(Ordering::Relaxed);
        
        // Exponential moving average of risk
        let base_risk = (orders as f64 * 0.0001 + volume as f64 * 0.000001).min(1.0);
        let current_risk = self.detection_risk.load(Ordering::Relaxed) as f64 / 1000.0;
        let new_risk = current_risk * 0.99 + base_risk * 0.01;
        
        self.detection_risk.store((new_risk * 1000.0) as u64, Ordering::Relaxed);
    }
    
    /// Get current detection probability
    pub fn current_detection_risk(&self) -> DetectionRisk {
        let risk = self.detection_risk.load(Ordering::Relaxed) as f64 / 1000.0;
        
        if risk < 0.001 {
            DetectionRisk::None
        } else if risk < 0.01 {
            DetectionRisk::Low
        } else if risk < 0.05 {
            DetectionRisk::Medium
        } else if risk < 0.10 {
            DetectionRisk::High
        } else {
            DetectionRisk::Critical
        }
    }
    
    /// Set execution profile
    pub fn set_profile(&mut self, profile: ExecutionProfile) {
        self.profile = profile;
    }
    
    /// Get execution statistics
    pub fn stats(&self) -> StealthStats {
        StealthStats {
            orders_executed: self.orders_executed.load(Ordering::Relaxed),
            total_volume: self.total_volume.load(Ordering::Relaxed) as f64 / 1000.0,
            detection_risk: self.current_detection_risk(),
            profile: self.profile,
        }
    }
    
    /// Reset stealth state
    pub fn reset(&mut self) {
        self.detection_risk.store(0, Ordering::Relaxed);
        self.orders_executed.store(0, Ordering::Relaxed);
        self.total_volume.store(0, Ordering::Relaxed);
    }
}

/// Stealth execution statistics
#[derive(Debug, Clone)]
pub struct StealthStats {
    pub orders_executed: u64,
    pub total_volume: f64,
    pub detection_risk: DetectionRisk,
    pub profile: ExecutionProfile,
}

/// Execution result types
#[derive(Debug)]
pub enum ExecutionResult {
    Success { order_id: u64, fill_price: f64, fill_volume: f64, latency_ns: u64 },
    PartialFill { order_id: u64, filled: f64, remaining: f64, reason: String },
    Rejected(String),
    Failed(String),
    StealthFragments(Vec<OrderFragment>),
    AggressiveFragments(Vec<OrderFragment>),
    AdaptiveFragments(Vec<OrderFragment>),
    PassiveOrder(Vec<OrderFragment>),
    IcebergOrder(Vec<OrderFragment>),
}

impl ExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success { .. } | 
                       ExecutionResult::StealthFragments(_) |
                       ExecutionResult::AggressiveFragments(_) |
                       ExecutionResult::AdaptiveFragments(_) |
                       ExecutionResult::PassiveOrder(_) |
                       ExecutionResult::IcebergOrder(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::OrderBook;
    
    #[test]
    fn test_stealth_executor() {
        let config = StealthConfig::default();
        let mut executor = StealthExecutor::new(config);
        
        let mut order = Order {
            order_id: 1,
            volume: 0.025,
            limit_price: 100.00,
            side: 0,
            tick_size: 0.01,
            expected_slippage: 1.0,
            ..Default::default()
        };
        
        let book = OrderBook::new(1, 0.01);
        
        let result = executor.execute(&mut order, &book);
        assert!(result.is_success());
        
        let stats = executor.stats();
        assert_eq!(stats.orders_executed, 1);
    }
}
