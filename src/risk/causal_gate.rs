use crate::risk::gate::{RiskGate, GateDecision, GateContext, GateStatus};
use crate::causality::fusion::{SignalFusion, FusionConfig};

impl RiskGate {
    pub fn evaluate_with_causality(
        &self,
        ctx: &GateContext,
        p_lead: f64,
        p_trans: f64,
        tau: f64
    ) -> GateDecision {
        let mut decision = self.evaluate(ctx);

        let mut fusion = SignalFusion::new(FusionConfig::default());
        let components = vec![
            ("lead".to_string(), p_lead, 0.4),
            ("trans".to_string(), p_trans, 0.3),
        ];

        let fused = fusion.fuse(ctx.phi_t, components, tau);

        if fused.value < 0.6 {
             decision.status = GateStatus::Closed;
        }

        decision
    }
}
