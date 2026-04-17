pub struct InterruptHandler;
pub struct InterruptState { pub value: f64 }
pub struct InterruptConfig;
pub struct MacroEvent;
pub enum MacroSeverity { Low, Medium, High }

impl InterruptHandler {
    pub fn new(_cfg: InterruptConfig) -> Self { Self }
    pub fn update(&self, _events: &[MacroEvent], _now: u64) -> super::state_vector::InterruptState {
        super::state_vector::InterruptState::default()
    }
}
