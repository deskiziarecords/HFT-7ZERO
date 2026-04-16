// ============================================================
// HFT STEALTH SYSTEM - CORE LIBRARY
// ============================================================

#![feature(float_gamma)]
#![feature(allocator_api)]
#![feature(asm_experimental_arch)]
#![feature(portable_simd)]
#![feature(strict_provenance)]
#![deny(warnings)]

pub mod memory {
    pub mod allocator;
    pub mod cache_aligned;
    pub mod zero_copy;
    pub mod numa;
    
    pub use allocator::{HFTAllocator, ArenaAllocator, ObjectPool};
    pub use cache_aligned::CacheAligned;
    pub use zero_copy::{ZeroCopyBuffer, SharedMemoryRegion, RingBuffer};
}

pub mod io {
    pub mod io_uring;
    pub mod packet_capture;
    pub mod ring_buffer;
    pub mod network;
    pub mod timestamp;
    
    pub use io_uring::{IoUringDriver, IoUringConfig};
    pub use packet_capture::PacketCapture;
    pub use ring_buffer::{MPSCRingBuffer, SPSCRingBuffer};
}

pub mod market {
    pub mod order_book;
    pub mod tick;
    pub mod depth;
    pub mod liquidity;
    pub mod price_level;
    
    pub use order_book::{OrderBook, OrderBookLevel};
    pub use tick::Tick;
}

pub mod ml {
    pub mod jax_bridge;
    pub use jax_bridge::JAXModel;
}

pub mod causality {
    pub mod granger;
    pub mod transfer_entropy;
    pub mod ccm;
    pub mod spearman;
    pub mod fusion;
    pub mod lag_selection;
    pub mod causality_network;
}

pub mod risk;
pub mod execution;
pub mod os;
pub mod monitoring;

pub mod config {
    pub mod settings;
    pub mod constants;
    pub mod instruments;
    pub mod dynamic_config;
    
    pub use settings::SystemConfig;
}

pub mod utils {
    pub mod time;
    pub mod math;
    pub mod stats;
    pub mod logger;
    pub mod thread_pool;
    pub mod profiler;
    
    pub use time::PreciseTime;
}

pub use config::SystemConfig;
pub use market::{OrderBook, OrderBookLevel};
pub use risk::RiskGate;
pub use execution::StealthExecutor;
pub use monitoring::metrics::{MetricsCollector, SystemMetrics};

use std::sync::Arc;
use std::time::{Instant, Duration};
use parking_lot::RwLock;
use dashmap::DashMap;
use tokio::sync::watch;
use crate::market::tick::Tick;
use crate::risk::gate::GateContext;
use crate::utils::time::PreciseTime;

#[allow(dead_code)]
pub struct HFTStealthSystem {
    pub(crate) config: Arc<SystemConfig>,
    pub(crate) market_data: Arc<RwLock<OrderBook>>,
    pub(crate) risk_gate: Arc<RiskGate>,
    pub(crate) stealth_executor: Arc<StealthExecutor>,
    pub(crate) market_os: Arc<crate::os::MarketOS>,
    pub(crate) adaptive_controller: Arc<RwLock<crate::os::AdaptiveController>>,
    pub(crate) metrics: Arc<MetricsCollector>,
    pub(crate) components: DashMap<String, Box<dyn Component>>,
    pub(crate) shutdown_tx: watch::Sender<bool>,
    pub(crate) shutdown_rx: watch::Receiver<bool>,
}

pub trait Component: Send + Sync {
    fn name(&self) -> &'static str;
    fn start(&self) -> Result<(), SystemError>;
    fn stop(&self) -> Result<(), SystemError>;
    fn health_check(&self) -> HealthStatus;
}

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Latency violation: {0}ns exceeded budget")]
    LatencyViolation(u64),
    #[error("Risk gate triggered: {0}")]
    RiskGateTriggered(String),
    #[error("I/O error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Model inference failed: {0}")]
    InferenceError(String),
    #[error("Execution rejected: {0}")]
    ExecutionRejected(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Starting,
    Stopping,
}

impl HFTStealthSystem {
    pub fn new(config: SystemConfig) -> Result<Self, SystemError> {
        let (tx, rx) = watch::channel(false);
        Ok(Self {
            config: Arc::new(config),
            market_data: Arc::new(RwLock::new(OrderBook::new())),
            risk_gate: Arc::new(RiskGate::new()),
            stealth_executor: Arc::new(StealthExecutor::new()),
            market_os: crate::os::MARKET_OS.clone(),
            adaptive_controller: Arc::new(RwLock::new(crate::os::AdaptiveController::new())),
            metrics: Arc::new(MetricsCollector::new()),
            components: DashMap::new(),
            shutdown_tx: tx,
            shutdown_rx: rx,
        })
    }
    
    pub fn register_component<C: Component + 'static>(&self, component: C) {
        self.components.insert(component.name().to_string(), Box::new(component));
    }
    
    pub async fn start(&self) -> Result<(), SystemError> {
        for entry in self.components.iter() {
            entry.value().start()?;
        }
        self.verify_latency_budget().await?;
        self.run_event_loop().await;
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<(), SystemError> {
        let _ = self.shutdown_tx.send(true);
        for entry in self.components.iter() {
            let _ = entry.value().stop();
        }
        Ok(())
    }
    
    async fn verify_latency_budget(&self) -> Result<(), SystemError> {
        let start = Instant::now();
        let latency_ns = start.elapsed().as_nanos() as u64;
        if latency_ns > 1_000_000 {
            return Err(SystemError::LatencyViolation(latency_ns));
        }
        Ok(())
    }
    
    async fn run_event_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_micros(100));
        let mut shutdown_rx = self.shutdown_rx.clone();
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_cycle().await {
                        self.metrics.record_error(e.to_string());
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() { break; }
                }
            }
        }
    }
    
    async fn process_cycle(&self) -> Result<(), SystemError> {
        let cycle_start = PreciseTime::now();

        // 1. L6 FIRST (Highest Priority - Bankruptcy)
        let bankruptcy = self.market_os.operator_l6();
        if bankruptcy.triggered {
            return Ok(()); // Halt everything
        }

        let ticks = self.capture_market_data().await?;

        // 2. L3 SECOND (Interrupts/News)
        let interrupt = self.market_os.operator_l3(&ticks);
        if interrupt.triggered {
             // Handle interrupt (e.g. adjust aggression)
        }

        // 3. Then other layers based on frequency
        self.market_os.execute_pipeline_by_frequency(&self.market_data.read(), &ticks);
        // 4. Layer 7: Adaptive Controller
        {
            let mut controller = self.adaptive_controller.write();
            let mut outputs = std::collections::HashMap::new();
            // Populate with latest operator results (simulated here for POC)
            outputs.insert("L5".to_string(), crate::os::OperatorResult { success: true, triggered: false, state_change: false, latency_ns: 0, value: 0.85, message: "".into() });
            controller.update(&outputs);
        }

        self.update_order_book(&ticks).await;
        let predictions = self.run_inference(&ticks).await?;

        let mut gate_ctx = GateContext::default();
        if let Some(&phi) = predictions.first() { gate_ctx.phi_t = phi; }
        if let Some(&ev) = predictions.get(1) { gate_ctx.ev_t = ev; }

        // Evaluate with causality fusion
        // p_lead and p_trans would come from inference in production
        let risk = self.risk_gate.evaluate_with_causality(&gate_ctx, 0.867, 0.623, 0.5);

        if self.detect_harmonic_trap(&predictions, &ticks).await {
            return Ok(());
        }

        if risk.status == crate::risk::gate::GateStatus::Open {
            self.execute_trades(&predictions, risk.signal_adjustment).await?;
        }

        let cycle_latency = cycle_start.elapsed_nanos();
        self.metrics.record_latency("cycle", cycle_latency);
        Ok(())
    }
    
    async fn capture_market_data(&self) -> Result<Vec<Tick>, SystemError> {
        Ok(Vec::new())
    }
    
    async fn update_order_book(&self, _ticks: &[Tick]) {
    }
    
    async fn run_inference(&self, _ticks: &[Tick]) -> Result<Vec<f64>, SystemError> {
        Ok(vec![0.75, 0.0114])
    }
    
    async fn detect_harmonic_trap(&self, _predictions: &[f64], _actual: &[Tick]) -> bool {
        false
    }
    
    async fn execute_trades(&self, _signals: &[f64], qty: f64) -> Result<(), SystemError> {
        if qty > 0.0 {
            if let Some(routing) = self.stealth_executor.plan_routing(qty) {
                 tracing::debug!("Optimal routing weights: {:?}", routing.weights);
            }
        }
        Ok(())
    }
    
    pub fn get_metrics(&self) -> SystemMetrics {
        self.metrics.snapshot()
    }
    
    pub fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
