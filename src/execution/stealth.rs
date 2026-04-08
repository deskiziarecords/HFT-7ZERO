// ============================================================
// STEALTH EXECUTOR
// ============================================================

pub struct StealthExecutor;

impl StealthExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl crate::Component for StealthExecutor {
    fn name(&self) -> &'static str { "StealthExecutor" }
    fn start(&self) -> Result<(), crate::SystemError> { Ok(()) }
    fn stop(&self) -> Result<(), crate::SystemError> { Ok(()) }
    fn health_check(&self) -> crate::HealthStatus { crate::HealthStatus::Healthy }
}
