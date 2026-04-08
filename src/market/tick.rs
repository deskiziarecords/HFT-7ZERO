// ============================================================
// MARKET TICK
// ============================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TickType {
    Bid,
    Ask,
    Trade,
    Snapshot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tick {
    pub instrument_id: u32,
    pub price: f64,
    pub volume: f64,
    pub tick_type: TickType,
    pub timestamp_ns: u64,
}

impl Default for Tick {
    fn default() -> Self {
        Self {
            instrument_id: 0,
            price: 0.0,
            volume: 0.0,
            tick_type: TickType::Trade,
            timestamp_ns: 0,
        }
    }
}
