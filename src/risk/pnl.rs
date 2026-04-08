// ============================================================
// PNL CALCULATOR
// ============================================================
// Real-time profit and loss tracking
// ============================================================


use crate::market::OrderBook;
use dashmap::DashMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub instrument_id: u32,
    pub quantity: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct TradeRecord {
    pub trade_id: u64,
    pub instrument_id: u32,
    pub price: f64,
    pub size: f64,
    pub side: u8,
    pub timestamp_ns: u64,
}

pub struct PnLCalculator {
    pub total_realized_pnl: f64,
    pub total_unrealized_pnl: f64,
}

impl PnLCalculator {
    pub fn new() -> Self {
        Self {
            total_realized_pnl: 0.0,
            total_unrealized_pnl: 0.0,
        }
    }

    pub fn calculate_total_pnl(&self, positions: &DashMap<u32, Position>, _book: &OrderBook) -> f64 {
        let mut unrealized = 0.0;
        for entry in positions.iter() {
            unrealized += entry.value().unrealized_pnl;
        }
        self.total_realized_pnl + unrealized
    }

    pub fn record_trade(&mut self, _trade: TradeRecord) {
        // Update realized PnL logic
    }

    pub fn reset_daily(&mut self) {
        self.total_realized_pnl = 0.0;
        self.total_unrealized_pnl = 0.0;
    }
}
