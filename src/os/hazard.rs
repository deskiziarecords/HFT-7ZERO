use crate::market::{Tick, tick::TickType};
pub struct OrderFlowAnalyzer;
impl OrderFlowAnalyzer {
    pub fn new() -> Self { Self }
    pub fn cancel_exec_imbalance(&self, ticks: &[Tick]) -> f64 {
        let mut cancels = 0.0;
        let mut execs = 0.0;
        for tick in ticks {
            match tick.tick_type {
                TickType::Cancel => cancels += tick.volume,
                TickType::Trade => execs += tick.volume,
                _ => {}
            }
        }
        (execs - cancels) / (execs + cancels + 1e-8)
    }
}
pub struct HazardRate { _alpha: [f64; 3], _decay: f64 }
impl HazardRate { pub fn new(alpha: [f64; 3], decay: f64) -> Self { Self { _alpha: alpha, _decay: decay } } }
