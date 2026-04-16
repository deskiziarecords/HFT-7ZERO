use crate::execution::schur_router::{SchurRouter, RoutingResult, RoutingParams, Venue};
use nalgebra::{DMatrix, DVector};

pub struct StealthExecutor {
    pub schur_router: SchurRouter,
}

impl StealthExecutor {
    pub fn new() -> Self {
        // In production, venues would be loaded from SystemConfig
        let venues = vec![
            Venue { id: 0, latency_ms: 0.1, fees: 0.0001 },
            Venue { id: 1, latency_ms: 0.2, fees: 0.0002 },
        ];
        let params = RoutingParams {
            slippage_gamma: vec![0.1, 0.05],
            slippage_delta: vec![1.5, 1.5],
            correlation_decay: 0.01,
            adelic_rho: 3.5,
            adelic_max_nonzero: 3,
            blowup_kappa: 3.0,
        };
        Self {
            schur_router: SchurRouter::new(venues, params),
        }
    }

    pub fn plan_routing(&self, total_qty: f64) -> Option<RoutingResult> {
        let n = self.schur_router.venues.len();
        if n == 0 { return None; }

        let ofi = DMatrix::from_element(n, n, 0.1);
        let prev_w = DVector::from_element(n, 1.0 / (n as f64));

        self.schur_router.optimize(total_qty, &ofi, &prev_w)
    }
}

impl crate::Component for StealthExecutor {
    fn name(&self) -> &'static str { "StealthExecutor" }
    fn start(&self) -> Result<(), crate::SystemError> { Ok(()) }
    fn stop(&self) -> Result<(), crate::SystemError> { Ok(()) }
    fn health_check(&self) -> crate::HealthStatus { crate::HealthStatus::Healthy }
}
