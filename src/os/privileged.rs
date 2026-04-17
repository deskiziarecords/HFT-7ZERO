pub struct PrivilegedHandler;
pub struct PrivilegedState { pub value: f64 }
pub struct PrivilegedConfig;
pub enum OverrideSource { User, System, External }

impl PrivilegedHandler {
    pub fn new(_cfg: PrivilegedConfig) -> Self { Self }
    pub fn update(&self, _trigger: bool, _now: u64) -> super::state_vector::PrivilegedState {
        super::state_vector::PrivilegedState::default()
    }
}
