#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus { Healthy, Degraded, Unhealthy }
pub struct HealthChecker;
impl HealthChecker {
    pub fn new() -> Self { Self }
    pub fn overall_status(&self) -> HealthStatus { HealthStatus::Healthy }
    pub async fn run_checks(&self) {}
}
pub struct ComponentHealth;
