pub struct CollectorHandler;
pub struct CollectorState { pub value: f64 }
pub struct CollectorConfig;
pub struct SweepTarget;
pub enum SweepDirection { Long, Short }

impl CollectorHandler {
    pub fn new(_cfg: CollectorConfig) -> Self { Self }
    pub fn update(
        &self,
        _targets: &[SweepTarget],
        _mid: f64,
        _atr: f64,
        _gamma: f64,
        _veto: bool,
        _now: u64
    ) -> super::state_vector::CollectorState {
        super::state_vector::CollectorState::default()
    }
}
