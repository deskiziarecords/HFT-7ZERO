#[derive(Debug, Clone)]
pub enum LimitBreach { DailyLoss { loss: f64, limit: f64 }, Position { id: u32, size: f64 }, TradeExecuted { trade_id: u64, size: f64 } }
pub struct PositionLimits;
pub struct RiskLimits;
impl PositionLimits {
    pub fn new(_: f64, _: f64) -> Self { Self }
    pub fn update_position(&self, _: u32, _: f64) {}
    pub fn check_position_limits(&self) -> Option<LimitBreach> { None }
}
