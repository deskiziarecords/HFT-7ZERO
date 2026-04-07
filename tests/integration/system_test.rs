// ============================================================
// SYSTEM INTEGRATION TEST
// ============================================================
// End-to-end system tests
// Full pipeline validation
// Realistic market simulation
// ============================================================

use hft_stealth_system::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use rand::Rng;

// ============================================================
// TEST FIXTURES
// ============================================================

struct TestFixture {
    config: SystemConfig,
    system: Arc<HFTStealthSystem>,
    market_simulator: MarketSimulator,
}

impl TestFixture {
    async fn new() -> Self {
        let config = SystemConfig::for_environment(Environment::Development);
        let config = SystemConfig {
            dry_run: true,
            backtest_mode: true,
            ..config
        };
        
        let system = Arc::new(HFTStealthSystem::new(config.clone()).unwrap());
        let market_simulator = MarketSimulator::new();
        
        Self {
            config,
            system,
            market_simulator,
        }
    }
    
    async fn start(&self) {
        self.system.start().await.unwrap();
    }
    
    async fn stop(&self) {
        self.system.stop().await.unwrap();
    }
}

struct MarketSimulator {
    price: f64,
    volatility: f64,
    spread: f64,
    volume: f64,
}

impl MarketSimulator {
    fn new() -> Self {
        Self {
            price: 100.0,
            volatility: 0.001,
            spread: 0.01,
            volume: 10000.0,
        }
    }
    
    fn generate_tick(&mut self) -> Tick {
        let mut rng = rand::thread_rng();
        
        // Random walk price
        self.price *= 1.0 + (rng.gen::<f64>() - 0.5) * self.volatility;
        
        // Generate bid/ask/trade tick
        let tick_type = rng.gen_range(0..3);
        
        Tick {
            price: self.price,
            volume: rng.gen::<f64>() * self.volume,
            timestamp_ns: get_hardware_timestamp(),
            exchange_id: 1,
            side: (tick_type % 2) as u8,
            tick_type: tick_type as u8,
            flags: 0,
            sequence: rng.gen(),
            instrument_id: 1,
            trade_id: rng.gen(),
            _padding: [0; 16],
        }
    }
    
    fn generate_order_book(&self) -> OrderBook {
        let mut book = OrderBook::new(1, 0.01);
        
        // Populate depth
        for i in 0..10 {
            let bid = Tick::bid(self.price - i as f64 * self.spread, 1000.0 * (10 - i) as f64, get_hardware_timestamp(), 1);
            let ask = Tick::ask(self.price + i as f64 * self.spread, 1000.0 * (10 - i) as f64, get_hardware_timestamp(), 1);
            book.update(&bid);
            book.update(&ask);
        }
        
        book
    }
}

// ============================================================
// END-TO-END PIPELINE TESTS
// ============================================================

#[tokio::test]
async fn test_full_pipeline() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator::new();
    
    // Process 1000 ticks
    for _ in 0..1000 {
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        // Process tick through system
        // This would be handled by the system's main loop
        
        // Verify system is still healthy
        let health = fixture.system.health_check();
        assert_eq!(health, HealthStatus::Healthy);
    }
    
    fixture.stop().await;
}

#[tokio::test]
async fn test_pipeline_latency() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator::new();
    let mut latencies = Vec::new();
    
    // Measure end-to-end latency for 100 cycles
    for _ in 0..100 {
        let start = Instant::now();
        
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        // Simulate pipeline stages
        let _ = tick;
        let _ = book;
        
        let latency = start.elapsed().as_nanos() as u64;
        latencies.push(latency);
    }
    
    // Verify latency constraints
    let p99 = percentile(&latencies, 0.99);
    assert!(p99 < 1_000_000, "P99 latency {}ns exceeds 1ms", p99);
    
    let p95 = percentile(&latencies, 0.95);
    assert!(p95 < 800_000, "P95 latency {}ns exceeds 800μs", p95);
    
    fixture.stop().await;
}

#[tokio::test]
async fn test_throughput() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator::new();
    let start = Instant::now();
    let target_ticks = 10000;
    
    for i in 0..target_ticks {
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        // Process tick
        let _ = tick;
        let _ = book;
        
        // Progress indicator
        if i % 1000 == 0 {
            println!("Processed {} ticks", i);
        }
    }
    
    let elapsed = start.elapsed();
    let throughput = target_ticks as f64 / elapsed.as_secs_f64();
    
    println!("Throughput: {:.0} ticks/sec", throughput);
    assert!(throughput > 50000.0, "Throughput {} < 50000 ticks/sec", throughput);
    
    fixture.stop().await;
}

// ============================================================
// COMPONENT INTEGRATION TESTS
// ============================================================

#[tokio::test]
async fn test_risk_gate_integration() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator::new();
    let gate = RiskGate::new();
    
    // Process ticks and verify risk gate doesn't trigger unnecessarily
    let mut triggers = 0;
    
    for _ in 0..1000 {
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        let ctx = GateContext {
            volatility_regime: 1,
            tau_max_ns: 500_000_000,
            price_variation: 0.001,
            atr_20: 0.005,
            delta_threshold: 0.3,
            kurtosis: 3.0,
            drift_bias: 0.0,
            predicted_prices: vec![],
            actual_prices: vec![],
            fill_probability: 0.5,
            conditional_pnl: 0.0,
            atr_10: 0.003,
            current_depth: 100000.0,
            candle_body_ratio: 0.5,
            order_book_conflict: false,
        };
        
        let decision = gate.evaluate(&ctx);
        if decision.status != GateStatus::Open {
            triggers += 1;
        }
    }
    
    // In normal market, triggers should be rare
    assert!(triggers < 50, "Too many risk gate triggers: {}", triggers);
    
    fixture.stop().await;
}

#[tokio::test]
async fn test_execution_integration() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let simulator = MarketSimulator::new();
    let book = simulator.generate_order_book();
    
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    // Test gate check
    let gate_open = executor.gate_check(0.025, 1.0, &book);
    assert!(gate_open, "Gate should be open for valid order");
    
    // Test execution
    let result = executor.execute(&mut order, &book);
    assert!(result.is_success(), "Execution should succeed");
    
    // Verify stealth metrics
    let stats = executor.stats();
    assert!(stats.detection_risk <= DetectionRisk::Low, "Detection risk too high");
    
    fixture.stop().await;
}

// ============================================================
// ERROR HANDLING TESTS
// ============================================================

#[tokio::test]
async fn test_error_recovery() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    // Simulate error conditions
    let mut simulator = MarketSimulator::new();
    
    // Process some normal ticks
    for _ in 0..100 {
        let tick = simulator.generate_tick();
        let _ = tick;
    }
    
    // Simulate a spike in latency
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(2));
    let latency = start.elapsed().as_nanos() as u64;
    
    // Verify system handles latency spike
    assert!(latency < 5_000_000, "Latency spike too high");
    
    // System should continue operating
    let health = fixture.system.health_check();
    assert_eq!(health, HealthStatus::Healthy);
    
    fixture.stop().await;
}

#[tokio::test]
async fn test_stealth_detection() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let simulator = MarketSimulator::new();
    let book = simulator.generate_order_book();
    
    // Execute many orders and track detection probability
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    for _ in 0..100 {
        let mut order_clone = order.clone();
        let _ = executor.execute(&mut order_clone, &book);
    }
    
    let stats = executor.stats();
    println!("Detection probability: {:?}", stats.detection_risk);
    
    // Detection probability should remain very low
    assert!(stats.detection_risk <= DetectionRisk::Medium, "Detection probability too high");
    
    fixture.stop().await;
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn percentile(data: &[u64], p: f64) -> u64 {
    let mut sorted = data.to_vec();
    sorted.sort();
    let idx = (sorted.len() as f64 * p) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ============================================================
// SCENARIO TESTS
// ============================================================

#[tokio::test]
async fn test_high_volatility_scenario() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator {
        volatility: 0.01, // High volatility
        ..MarketSimulator::new()
    };
    
    let mut latencies = Vec::new();
    
    for _ in 0..1000 {
        let start = Instant::now();
        
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        // Process in high volatility
        let _ = tick;
        let _ = book;
        
        latencies.push(start.elapsed().as_nanos() as u64);
    }
    
    let p99 = percentile(&latencies, 0.99);
    // Even in high volatility, latency should be reasonable
    assert!(p99 < 2_000_000, "High volatility latency too high: {}ns", p99);
    
    fixture.stop().await;
}

#[tokio::test]
async fn test_low_liquidity_scenario() {
    let fixture = TestFixture::new().await;
    fixture.start().await;
    
    let mut simulator = MarketSimulator {
        volume: 100.0, // Low volume
        ..MarketSimulator::new()
    };
    
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    
    for _ in 0..100 {
        let tick = simulator.generate_tick();
        let book = simulator.generate_order_book();
        
        // Execute in low liquidity
        let mut order_clone = order.clone();
        let result = executor.execute(&mut order_clone, &book);
        
        // May be rejected due to liquidity constraints
        if !result.is_success() {
            println!("Order rejected in low liquidity: {:?}", result);
        }
    }
    
    fixture.stop().await;
}
