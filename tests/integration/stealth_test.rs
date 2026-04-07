// ============================================================
// STEALTH EXECUTION INTEGRATION TEST
// ============================================================
// Detection probability validation
// Fragmentation strategy testing
// Jitter distribution verification
// ============================================================

use hft_stealth_system::*;
use rand::Rng;
use std::collections::HashMap;

// ============================================================
// STEALTH EXECUTION TESTS
// ============================================================

#[test]
fn test_stealth_executor_gate() {
    let config = StealthConfig::default();
    let executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    // Valid order
    let valid = executor.gate_check(0.025, 1.0, &book);
    assert!(valid, "Valid order should pass gate");
    
    // Invalid volume (too low)
    let too_low = executor.gate_check(0.005, 1.0, &book);
    assert!(!too_low, "Volume below minimum should fail");
    
    // Invalid volume (too high)
    let too_high = executor.gate_check(0.1, 1.0, &book);
    assert!(!too_high, "Volume above maximum should fail");
    
    // Invalid slippage (too low)
    let slippage_low = executor.gate_check(0.025, 0.1, &book);
    assert!(!slippage_low, "Slippage below minimum should fail");
    
    // Invalid slippage (too high)
    let slippage_high = executor.gate_check(0.025, 2.0, &book);
    assert!(!slippage_high, "Slippage above maximum should fail");
}

#[test]
fn test_stealth_execution_profiles() {
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    // Test each execution profile
    let profiles = [
        ExecutionProfile::Stealth,
        ExecutionProfile::Aggressive,
        ExecutionProfile::Adaptive,
        ExecutionProfile::Passive,
        ExecutionProfile::Iceberg,
    ];
    
    for profile in profiles {
        executor.set_profile(profile);
        let result = executor.execute(&mut order.clone(), &book);
        assert!(result.is_success(), "Profile {:?} should execute", profile);
    }
}

#[test]
fn test_detection_risk_tracking() {
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    // Execute many orders and track risk
    for _ in 0..1000 {
        let mut order_clone = order.clone();
        let _ = executor.execute(&mut order_clone, &book);
    }
    
    let stats = executor.stats();
    println!("Detection risk after 1000 orders: {:?}", stats.detection_risk);
    
    // Detection probability should remain very low (ℙ ≈ 0)
    match stats.detection_risk {
        DetectionRisk::None | DetectionRisk::Low => {}
        _ => panic!("Detection risk too high: {:?}", stats.detection_risk),
    }
}

// ============================================================
// FRAGMENTATION TESTS
// ============================================================

#[test]
fn test_fragmentation_strategies() {
    let total_volume = 0.025;
    let base_price = 100.00;
    
    let strategies = [
        FragmentStrategy::Uniform,
        FragmentStrategy::Geometric,
        FragmentStrategy::Random,
        FragmentStrategy::Adaptive,
    ];
    
    for strategy in strategies {
        let config = FragmentConfig {
            strategy,
            ..Default::default()
        };
        let mut fragmenter = Fragmenter::new(config);
        let fragments = fragmenter.fragment(total_volume, base_price);
        
        // Verify total volume sums correctly
        let sum: f64 = fragments.iter().map(|f| f.volume).sum();
        assert!((sum - total_volume).abs() < 0.0001, 
                "Strategy {:?} volume sum mismatch: {} vs {}", strategy, sum, total_volume);
        
        // Verify fragment count is within bounds
        assert!(fragments.len() >= 3 && fragments.len() <= 8,
                "Strategy {:?} fragment count out of bounds: {}", strategy, fragments.len());
        
        // Verify fragment sizes are positive
        for fragment in &fragments {
            assert!(fragment.volume > 0.0, "Strategy {:?} zero volume fragment", strategy);
        }
    }
}

#[test]
fn test_fragment_reassembly() {
    let config = FragmentConfig::default();
    let mut fragmenter = Fragmenter::new(config);
    let total_volume = 0.025;
    
    let fragments = fragmenter.fragment(total_volume, 100.00);
    let fragment_ids: Vec<u64> = fragments.iter().map(|f| f.fragment_id).collect();
    
    let reassembled = fragmenter.reassemble(&fragment_ids);
    assert!((reassembled - total_volume).abs() < 0.0001);
}

#[test]
fn test_fragment_statistics() {
    let config = FragmentConfig::default();
    let mut fragmenter = Fragmenter::new(config);
    
    // Generate many fragments
    for _ in 0..100 {
        let _ = fragmenter.fragment(0.025, 100.00);
    }
    
    let stats = fragmenter.stats();
    assert!(stats.total_fragments > 0);
    assert!(stats.avg_fragment_size > 0.0);
    assert!(stats.min_fragment_size <= stats.max_fragment_size);
}

// ============================================================
// JITTER TESTS
// ============================================================

#[test]
fn test_jitter_distribution() {
    let config = JitterConfig::default();
    let mut jitter_gen = TimingObfuscator::new(config);
    
    let mut jitters = Vec::new();
    for _ in 0..10000 {
        let jitter = jitter_gen.generate();
        jitters.push(jitter.as_micros() as u64);
    }
    
    // Verify uniform distribution bounds
    for &jitter in &jitters {
        assert!(jitter >= 50 && jitter <= 500, "Jitter out of bounds: {}", jitter);
    }
    
    // Calculate statistics
    let mean = jitters.iter().sum::<u64>() as f64 / jitters.len() as f64;
    println!("Mean jitter: {:.2} μs", mean);
    
    // Mean should be around 275 μs (midpoint of 50-500)
    assert!(mean > 200.0 && mean < 350.0, "Mean jitter unexpected: {}", mean);
}

#[test]
fn test_jitter_types() {
    let types = [
        JitterType::Uniform,
        JitterType::Gaussian,
        JitterType::Poisson,
        JitterType::Exponential,
    ];
    
    for jitter_type in types {
        let config = JitterConfig {
            jitter_type,
            ..Default::default()
        };
        let mut jitter_gen = TimingObfuscator::new(config);
        
        let mut jitters = Vec::new();
        for _ in 0..1000 {
            let jitter = jitter_gen.generate();
            jitters.push(jitter.as_micros() as u64);
        }
        
        // Verify bounds for all types
        for &jitter in &jitters {
            assert!(jitter >= 50 && jitter <= 500, 
                    "Type {:?} jitter out of bounds: {}", jitter_type, jitter);
        }
        
        let mean = jitters.iter().sum::<u64>() as f64 / jitters.len() as f64;
        println!("Type {:?} mean jitter: {:.2} μs", jitter_type, mean);
    }
}

#[test]
fn test_anti_pattern_detection() {
    let mut detector = AntiPatternDetector::new(0.7);
    
    // Random jitter (no pattern)
    let mut rng = rand::thread_rng();
    for _ in 0..100 {
        let jitter = rng.gen_range(50..500);
        let pattern = detector.record(jitter);
        assert!(!pattern, "Random jitter should not trigger pattern detection");
    }
    
    detector.reset();
    
    // Regular jitter (pattern)
    for i in 0..100 {
        let jitter = if i % 10 < 5 { 100 } else { 200 };
        let pattern = detector.record(jitter);
        if i > 50 {
            // Should detect pattern after enough samples
            println!("Pattern detection at iteration {}: {}", i, pattern);
        }
    }
}

// ============================================================
// VOLUME CONSTRAINTS TEST (V ∈ [0.01, 0.05])
// ============================================================

#[test]
fn test_volume_constraints() {
    let config = StealthConfig::default();
    let executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    // Test valid volumes
    let valid_volumes = [0.01, 0.02, 0.03, 0.04, 0.05];
    for &volume in &valid_volumes {
        assert!(executor.gate_check(volume, 1.0, &book), 
                "Volume {} should be valid", volume);
    }
    
    // Test invalid volumes
    let invalid_volumes = [0.005, 0.009, 0.051, 0.1, 0.5];
    for &volume in &invalid_volumes {
        assert!(!executor.gate_check(volume, 1.0, &book), 
                "Volume {} should be invalid", volume);
    }
}

// ============================================================
// SLIPPAGE CONSTRAINTS TEST (Δp_slip ≤ [0.5, 1.5] pips)
// ============================================================

#[test]
fn test_slippage_constraints() {
    let config = StealthConfig::default();
    let executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    // Test valid slippage
    let valid_slippage = [0.5, 0.75, 1.0, 1.25, 1.5];
    for &slippage in &valid_slippage {
        assert!(executor.gate_check(0.025, slippage, &book), 
                "Slippage {} should be valid", slippage);
    }
    
    // Test invalid slippage
    let invalid_slippage = [0.1, 0.4, 1.6, 2.0, 5.0];
    for &slippage in &invalid_slippage {
        assert!(!executor.gate_check(0.025, slippage, &book), 
                "Slippage {} should be invalid", slippage);
    }
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn generate_test_order_book() -> OrderBook {
    let mut book = OrderBook::new(1, 0.01);
    
    for i in 0..10 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0 * (10 - i) as f64, get_hardware_timestamp(), 1);
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0 * (10 - i) as f64, get_hardware_timestamp(), 1);
        book.update(&bid);
        book.update(&ask);
    }
    
    book
}

// ============================================================
// COMPREHENSIVE STEALTH TEST
// ============================================================

#[test]
fn test_comprehensive_stealth() {
    let config = StealthConfig::default();
    let mut executor = StealthExecutor::new(config);
    let book = generate_test_order_book();
    
    let mut rng = rand::thread_rng();
    let mut order = Order::buy(1, 0.025, 100.00);
    order.expected_slippage = 1.0;
    
    let mut results = HashMap::new();
    
    // Execute orders with various parameters
    for _ in 0..500 {
        let volume = rng.gen_range(0.01..0.05);
        let slippage = rng.gen_range(0.5..1.5);
        
        let mut order_clone = order.clone();
        order_clone.volume = volume;
        order_clone.expected_slippage = slippage;
        
        let result = executor.execute(&mut order_clone, &book);
        *results.entry(result.is_success()).or_insert(0) += 1;
    }
    
    let success_rate = *results.get(&true).unwrap_or(&0) as f64 / 500.0;
    println!("Stealth execution success rate: {:.1}%", success_rate * 100.0);
    
    // Should have high success rate for valid parameters
    assert!(success_rate > 0.95, "Success rate too low: {}", success_rate);
    
    let stats = executor.stats();
    println!("Final detection risk: {:?}", stats.detection_risk);
    assert!(stats.detection_risk <= DetectionRisk::Low, "Detection risk too high");
}
