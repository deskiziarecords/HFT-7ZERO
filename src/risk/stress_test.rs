pub struct StressTester;
pub struct StressScenario;
pub struct ScenarioResult;
impl StressTester {
    pub fn new() -> Self { Self }
    pub fn run_all_scenarios(&self, _: &crate::market::OrderBook, _: &dashmap::DashMap<u32, super::Position>) -> Vec<ScenarioResult> { vec![] }
}
