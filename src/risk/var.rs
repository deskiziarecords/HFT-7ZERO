// ============================================================
// VALUE AT RISK (VaR)
// ============================================================
// Historical and Parametric VaR calculation
// ============================================================

use crate::market::OrderBook;
use crate::risk::pnl::Position;
use dashmap::DashMap;

pub trait ValueAtRisk: Send + Sync {
    fn calculate(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String>;
    fn expected_shortfall(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String>;
}

pub struct HistoricalVaR {
    pub confidence: f64,
    pub horizon_seconds: u64,
}

impl HistoricalVaR {
    pub fn new(confidence: f64, horizon_seconds: u64) -> Self {
        Self { confidence, horizon_seconds }
    }
}

impl ValueAtRisk for HistoricalVaR {
    fn calculate(&self, _positions: &DashMap<u32, Position>, _book: &OrderBook, _confidence: f64) -> Result<f64, String> {
        Ok(0.0)
    }
    fn expected_shortfall(&self, _positions: &DashMap<u32, Position>, _book: &OrderBook, _confidence: f64) -> Result<f64, String> {
        Ok(0.0)
    }
}

pub struct ParametricVaR;
