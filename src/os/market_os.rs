// ============================================================
// MARKET OPERATING SYSTEM (L1-L6)
// ============================================================
// Main orchestrator for all market operators
// Real-time state management
// Operator composition and sequencing
// ============================================================

use super::*;
use crate::market::{OrderBook, Tick, DepthProfile};
use crate::utils::time::get_hardware_timestamp;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::RwLock;

/// Main Market OS orchestrator
pub struct MarketOS {
    config: OSConfig,
    state: Arc<RwLock<OSState>>,

    // Sub-components
    hazard: Arc<hazard::HazardRate>,
    liquidity: Arc<liquidity_field::NavierStokesLiquidity>,
    gamma: Arc<gamma_control::GammaController>,
    bankruptcy: Arc<bankruptcy::BankruptcyGate>,
    regime_detector: Arc<regime_detector::RegimeDetector>,
    order_flow: Arc<order_flow::OrderFlowAnalyzer>,

    // Metrics
    operator_latencies: AtomicU64,
    total_operations: AtomicU64,
    last_update_ns: AtomicU64,
}

impl MarketOS {
    /// Create new Market OS
    pub fn new(config: OSConfig) -> Self {
        let state = Arc::new(RwLock::new(OSState::default()));

        Self {
            hazard: Arc::new(hazard::HazardRate::new(config.hazard_alpha, config.hazard_decay)),
            liquidity: Arc::new(liquidity_field::NavierStokesLiquidity::new(
                config.field_resolution,
                config.liquidity_viscosity,
                config.liquidity_diffusion,
            )),
            gamma: Arc::new(gamma_control::GammaController::new(
                config.gamma_eta,
                config.gamma_kappa,
                config.gamma_target,
            )),
            bankruptcy: Arc::new(bankruptcy::BankruptcyGate::new(
                config.max_drawdown,
                config.max_loss,
                config.auto_recovery,
            )),
            regime_detector: Arc::new(regime_detector::RegimeDetector::new(
                config.regime_bounds.clone(),
                config.regime_threshold,
            )),
            order_flow: Arc::new(order_flow::OrderFlowAnalyzer::new()),
            operator_latencies: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            last_update_ns: AtomicU64::new(0),
            config,
            state,
        }
    }

    /// Execute full operator pipeline (L1 → L6)
    pub fn execute_pipeline(&self, book: &OrderBook, ticks: &[Tick]) -> OperatorResult {
        let start_ns = get_hardware_timestamp();

        // L1: Extract regime (ℛ_t)
        let regime_result = self.operator_l1(book);
        if !regime_result.success {
            return regime_result;
        }

        // L2: Update hazard rate (ḣ_t)
        let hazard_result = self.operator_l2(book, ticks);

        // L3: Apply macro shocks (σ_t)
        let shock_result = self.operator_l3(ticks);

        // L4: Update liquidity field (∂_t u + (u·∇)u = -∇p + ν∇²u + f_liq)
        let liquidity_result = self.operator_l4(book);

        // L5: Adjust gamma (Γ_{t+1} = 𝒩(Γ_t, κ_strike, Φ_fb))
        let gamma_result = self.operator_l5(book);

        // L6: Check bankruptcy gate (θ_t)
        let bankruptcy_result = self.operator_l6();

        let latency_ns = get_hardware_timestamp() - start_ns;
        self.operator_latencies.fetch_add(latency_ns, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Update state
        let mut state = self.state.write();
        state.timestamp_ns = start_ns;
        state.sequence += 1;

        OperatorResult {
            success: !bankruptcy_result.triggered,
            state_change: regime_result.state_change || gamma_result.state_change,
            latency_ns,
            value: state.hazard_rate,
            message: if bankruptcy_result.triggered {
                "Bankruptcy gate triggered".to_string()
            } else {
                "Pipeline complete".to_string()
            },
        }
    }
    
    /// ℒ₁: Extract safe regime regions ℛ_t = {ℬ₂₀, ℬ₄₀, ℬ₆₀} ⊂ ℝ²
    pub fn operator_l1(&self, book: &OrderBook) -> OperatorResult {
        let start = get_hardware_timestamp();

        let mut regimes = Vec::new();
        let depths = [20, 40, 60];

        for &depth in &depths {
            if depth < 100 {
                let bid = book.bid_at_depth(depth).map(|l| l.price).unwrap_or(0.0);
                let ask = book.ask_at_depth(depth).map(|l| l.price).unwrap_or(0.0);

                if ask > 0.0 && bid > 0.0 {
                    let spread = ask - bid;
                    if spread <= book.spread() * 2.0 {
                        regimes.push((bid, ask));
                    }
                }
            }
        }

        let mut state = self.state.write();
        let changed = state.regime != regimes;
        state.regime = regimes;

        OperatorResult {
            success: !state.regime.is_empty(),
            state_change: changed,
            latency_ns: get_hardware_timestamp() - start,
            value: state.regime.len() as f64,
            message: format!("Found {} safe regimes", state.regime.len()),
        }
    }
    
    /// ℒ₂: Hazard dynamics ḣ_t = f_Δ(δ_t, ℐ_ce) · 𝕀[h_t ∈ ℛ_t]
    pub fn operator_l2(&self, book: &OrderBook, ticks: &[Tick]) -> OperatorResult {
        let start = get_hardware_timestamp();

        // Check if hazard rate is in regime
        let state = self.state.read();
        let in_regime = state.regime.iter().any(|&(b, a)| {
            state.hazard_rate >= b && state.hazard_rate <= a
        });

        if !in_regime {
            return OperatorResult {
                success: true,
                state_change: false,
                latency_ns: get_hardware_timestamp() - start,
                value: state.hazard_rate,
                message: "Not in regime".to_string(),
            };
        }

        // Calculate imbalance ℐ_ce
        let imbalance = self.order_flow.cancel_exec_imbalance(ticks);

        // f_Δ(δ, ℐ) = α₀δ + α₁·max(0,ℐ) + α₂ℐ²
        let f_delta = self.config.hazard_alpha[0] * book.spread()
            + self.config.hazard_alpha[1] * imbalance.max(0.0)
            + self.config.hazard_alpha[2] * imbalance.powi(2);

        let dt = if self.last_update_ns.load(Ordering::Acquire) > 0 {
            (get_hardware_timestamp() - self.last_update_ns.load(Ordering::Acquire)) as f64 / 1e9
        } else {
            0.001 // Default 1ms
        };

        let mut state = self.state.write();
        let old_hazard = state.hazard_rate;
        state.hazard_rate += f_delta * dt;
        state.hazard_rate = state.hazard_rate.clamp(0.0, 1.0);

        self.last_update_ns.store(get_hardware_timestamp(), Ordering::Release);

        OperatorResult {
            success: true,
            state_change: (state.hazard_rate - old_hazard).abs() > 0.001,
            latency_ns: get_hardware_timestamp() - start,
            value: state.hazard_rate,
            message: format!("Hazard updated: {:.6}", state.hazard_rate),
        }
    }
    
    /// ℒ₃: Macro shock injection σ_{t+} = σ_t(1 + α·I(t))
    pub fn operator_l3(&self, ticks: &[Tick]) -> OperatorResult {
        let start = get_hardware_timestamp();

        // Detect macro events from ticks
        let macro_events: Vec<u64> = ticks.iter()
            .filter(|t| {
                // Check for macro event flags (simplified)
                t.flags & 0x1000 != 0
            })
            .map(|t| t.timestamp_ns)
            .collect();

        let now = get_hardware_timestamp();
        let recent_events: Vec<&u64> = macro_events.iter()
            .filter(|&&ts| now - ts < self.config.macro_event_window_ns)
            .collect();

        let impulse = recent_events.len() as f64;
        let mut state = self.state.write();
        let old_vol = state.volatility;
        state.volatility *= 1.0 + self.config.macro_shock_alpha * impulse;
        state.volatility = state.volatility.clamp(0.0001, 0.5);

        OperatorResult {
            success: true,
            state_change: (state.volatility - old_vol).abs() > 0.0001,
            latency_ns: get_hardware_timestamp() - start,
            value: state.volatility,
            message: format!("Volatility: {:.6}", state.volatility),
        }
    }

    /// ℒ₄: Navier-Stokes liquidity field ∂ₜu + (u·∇)u = -∇p + ν∇²u + f_liq
    pub fn operator_l4(&self, book: &OrderBook) -> OperatorResult {
        let start = get_hardware_timestamp();

        // Extract liquidity field from order book
        let (field, pressure) = self.extract_liquidity_field(book);

        // Update field using Navier-Stokes
        let (new_field, vorticity) = self.liquidity.update_field(&field, &pressure);

        let mut state = self.state.write();
        state.liquidity_field = new_field;
        state.vorticity = vorticity;

        OperatorResult {
            success: true,
            state_change: true,
            latency_ns: get_hardware_timestamp() - start,
            value: vorticity.iter().map(|&v| v.abs()).sum::<f64>() / vorticity.len() as f64,
            message: format!("Vorticity: {:.6}", state.vorticity.first().unwrap_or(&0.0)),
        }
    }
    
    /// ℒ₅: Gamma control Γ_{t+1} = 𝒩(Γ_t, κ_strike, Φ_fb)
    pub fn operator_l5(&self, book: &OrderBook) -> OperatorResult {
        let start = get_hardware_timestamp();

        // Calculate feedback Φ_fb from market
        let feedback = self.gamma.calculate_feedback(book);

        // Update gamma
        let mut state = self.state.write();
        let old_gamma = state.gamma;
        state.gamma = self.gamma.update(state.gamma, feedback);

        OperatorResult {
            success: true,
            state_change: (state.gamma - old_gamma).abs() > 0.001,
            latency_ns: get_hardware_timestamp() - start,
            value: state.gamma,
            message: format!("Gamma: {:.6}", state.gamma),
        }
    }
    
    /// ℒ₆: Bankruptcy gate θ_t ∈ {0,1} ⇒ (θ_t=1 ⇒ σ_{t+1}=0, ℛ_{t+1}=∅)
    pub fn operator_l6(&self) -> OperatorResult {
        let start = get_hardware_timestamp();

        let state = self.state.read();
        let should_trigger = self.bankruptcy.check(
            state.gamma.abs(),
            state.hazard_rate,
            state.volatility,
        );

        if should_trigger {
            let mut state = self.state.write();
            state.theta = true;
            state.volatility = 0.0;
            state.regime.clear();
            state.hazard_rate = 0.0;

            return OperatorResult {
                success: false,
                state_change: true,
                latency_ns: get_hardware_timestamp() - start,
                value: 1.0,
                message: "BANKRUPTCY GATE TRIGGERED".to_string(),
            };
        }

        OperatorResult {
            success: true,
            state_change: false,
            latency_ns: get_hardware_timestamp() - start,
            value: 0.0,
            message: "Gate open".to_string(),
        }
    }

    /// Extract liquidity field from order book
    fn extract_liquidity_field(&self, book: &OrderBook) -> (Vec<f64>, Vec<f64>) {
        let mut field = vec![0.0; self.config.field_resolution];
        let mut pressure = vec![0.0; self.config.field_resolution];

        let (bids, asks) = book.top_levels(self.config.field_resolution / 2);

        for (i, level) in bids.iter().enumerate() {
            if i < field.len() / 2 {
                field[i] = level.volume;
                pressure[i] = level.price;
            }
        }
        
        for (i, level) in asks.iter().enumerate() {
            let idx = field.len() / 2 + i;
            if idx < field.len() {
                field[idx] = -level.volume;
                pressure[idx] = level.price;
            }
        }

        (field, pressure)
    }

    /// Get current OS state
    pub fn state(&self) -> OSState {
        self.state.read().clone()
    }

    /// Get performance metrics
    pub fn metrics(&self) -> OSMetrics {
        let ops = self.total_operations.load(Ordering::Relaxed);
        let total_latency = self.operator_latencies.load(Ordering::Relaxed);

        OSMetrics {
            total_operations: ops,
            avg_latency_ns: if ops > 0 { total_latency / ops } else { 0 },
            current_hazard: self.state.read().hazard_rate,
            current_gamma: self.state.read().gamma,
            bankruptcy_triggered: self.state.read().theta,
        }
    }

    /// Reset OS state
    pub fn reset(&self) {
        let mut state = self.state.write();
        *state = OSState::default();
        self.bankruptcy.reset();
        self.total_operations.store(0, Ordering::Relaxed);
        self.operator_latencies.store(0, Ordering::Relaxed);
    }
}

/// OS performance metrics
#[derive(Debug, Clone)]
pub struct OSMetrics {
    pub total_operations: u64,
    pub avg_latency_ns: u64,
    pub current_hazard: f64,
    pub current_gamma: f64,
    pub bankruptcy_triggered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_creation() {
        let os = MarketOS::new(OSConfig::default());
        assert_eq!(os.state().regime.len(), 0);
    }
}
