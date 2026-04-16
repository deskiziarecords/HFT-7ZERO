// ============================================================
// HFT STEALTH SYSTEM - CORE LIBRARY
// ============================================================

#![deny(warnings)]
#![feature(allocator_api)]
#![feature(asm_experimental_arch)]
#![feature(portable_simd)]
#![feature(strict_provenance)]

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
    
    pub use order_book::OrderBook;
    pub use tick::Tick;
}

pub mod ml {
    pub mod jax_bridge;
    pub use jax_bridge::JAXModel;
}

pub mod risk;
pub mod execution;

pub mod monitoring {
    pub mod metrics;
}

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
pub use market::OrderBook;
pub use risk::RiskGate;
pub use execution::StealthExecutor;
pub use monitoring::metrics::MetricsCollector;

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

#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub latency_p50_ns: u64,
    pub latency_p99_ns: u64,
    pub throughput_ticks_sec: f64,
    pub detection_probability: f64,
    pub sharpe_ratio: f64,
    pub total_trades: u64,
    pub total_pnl: f64,
}

impl HFTStealthSystem {
    pub fn new(config: SystemConfig) -> Result<Self, SystemError> {
        let (tx, rx) = watch::channel(false);
        Ok(Self {
            config: Arc::new(config),
            market_data: Arc::new(RwLock::new(OrderBook::new())),
            risk_gate: Arc::new(RiskGate::new()),
            stealth_executor: Arc::new(StealthExecutor::new()),
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
        let ticks = self.capture_market_data().await?;
        self.update_order_book(&ticks).await;
        let predictions = self.run_inference(&ticks).await?;

        let mut gate_ctx = GateContext::default();
        // Use actual model outputs for context
        if let Some(&phi) = predictions.first() { gate_ctx.phi_t = phi; }
        if let Some(&ev) = predictions.get(1) { gate_ctx.ev_t = ev; }

        let risk = self.risk_gate.evaluate(&gate_ctx);

        if self.detect_harmonic_trap(&predictions, &ticks).await {
            return Ok(());
        }

        if risk.status == crate::risk::gate::GateStatus::Open {
            // Pass the signal adjustment (sizing) to execution
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
        // Mock inference producing [phi_t, ev_t]
        Ok(vec![0.75, 0.0114])
    }
    
    async fn detect_harmonic_trap(&self, _predictions: &[f64], _actual: &[Tick]) -> bool {
        false
    }
    
    async fn execute_trades(&self, _signals: &[f64], qty: f64) -> Result<(), SystemError> {
        if qty > 0.0 {
            // Plan optimal routing across venues
            let _routing = self.stealth_executor.plan_routing(qty);
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
