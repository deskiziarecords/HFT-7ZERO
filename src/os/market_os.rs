use super::*;
use crate::market::{OrderBook, Tick};
use std::collections::HashMap;
use parking_lot::RwLock;
use crate::monitoring::interaction_logger::InteractionLogger;

pub struct MarketOS {
    pub config: OSConfig,
    pub state: Arc<RwLock<OSState>>,
    pub last_operator_updates: RwLock<HashMap<OperatorType, u64>>,
    pub logger: Arc<InteractionLogger>,
}

impl MarketOS {
    pub fn new(config: OSConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(OSState::default())),
            last_operator_updates: RwLock::new(HashMap::new()),
            logger: Arc::new(InteractionLogger::new()),
        }
    }

    pub fn operator_l1(&self, _book: &OrderBook) -> OperatorResult {
        // Log interaction: Layer 0 (Market Data) -> Layer 1
        self.logger.log("Layer0", "Layer1", UpdateFrequency::VeryLow);
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "L1".into() }
    }
    
    pub fn operator_l2(&self, _book: &OrderBook, __ticks: &[Tick]) -> OperatorResult {
        // Log interaction: Layer 1 -> Layer 2
        self.logger.log("Layer1", "Layer2", UpdateFrequency::Low);
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "L2".into() }
    }
    
    pub fn operator_l3(&self, __ticks: &[Tick]) -> OperatorResult {
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "L3".into() }
    }
    
    pub fn operator_l4(&self, _book: &OrderBook) -> OperatorResult {
        self.logger.log("Layer2", "Layer4", UpdateFrequency::Medium);
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "L4".into() }
    }
    
    pub fn operator_l5(&self, _book: &OrderBook) -> OperatorResult {
        self.logger.log("Layer4", "Layer5", UpdateFrequency::High);
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "L5".into() }
    }
    
    pub fn operator_l6(&self) -> OperatorResult {
        let state = self.state.read();
        if state.theta {
             return OperatorResult { success: false, triggered: true, state_change: false, latency_ns: 0, value: 1.0, message: "BANKRUPTCY".to_string() };
        }
        OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.0, message: "Safe".to_string() }
    }

    pub fn execute_pipeline_by_frequency(&self, book: &OrderBook, _ticks: &[Tick]) -> OperatorResult {
        let now = crate::utils::time::get_hardware_timestamp();
        let mut updates = self.last_operator_updates.write();

        // L1: Hourly for POC
        let l1_last = updates.get(&OperatorType::L1).cloned().unwrap_or(0);
        if now - l1_last > 3600 * 1_000_000_000 {
            self.operator_l1(book);
            updates.insert(OperatorType::L1, now);
        }
        
        // L5: High
        self.operator_l5(book)
    }
}

#[derive(Debug, Clone)]
pub struct OSMetrics;

#[derive(Debug, Clone)]
pub struct OperatorResult {
    pub success: bool,
    pub triggered: bool,
    pub state_change: bool,
    pub latency_ns: u64,
    pub value: f64,
    pub message: String,
}
