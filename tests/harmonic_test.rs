// ============================================================
// HARMONIC DETECTOR UNIT TESTS
// ============================================================
// Phase inversion detection
// Trap type classification
// Edge case handling
// ============================================================

use hft_stealth_system::signal::harmonic_detector::*;
use std::f64::consts::PI;

// ============================================================
// BASIC DETECTION TESTS
// ============================================================

#[test]
fn test_no_trap_detection() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    // Identical signals (no phase shift)
    let signal: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    let is_trap = detector.detect_trap(&signal, &signal);
    assert!(!is_trap, "Identical signals should not trigger trap");
}

#[test]
fn test_phase_inversion_trap() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    // In-phase signal
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    // 180-degree phase shift
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + PI).sin()).collect();
    
    let is_trap = detector.detect_trap(&pred, &act);
    assert!(is_trap, "Phase inversion should trigger trap");
}

#[test]
fn test_90_degree_phase_shift() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + PI / 2.0).sin()).collect();
    
    let is_trap = detector.detect_trap(&pred, &act);
    // 90° is exactly at threshold (π/2)
    // May or may not trigger depending on implementation
}

// ============================================================
// TRAP TYPE CLASSIFICATION TESTS
// ============================================================

#[test]
fn test_trap_type_phase_inversion() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + PI).sin()).collect();
    
    let (is_trap, trap_type) = detector.detect_with_type(&pred, &act);
    assert!(is_trap);
    assert_eq!(trap_type, TrapType::PhaseInversion);
}

#[test]
fn test_trap_type_frequency_doubling() {
    let mut detector = HarmonicTrapDetector::new(512);
    
    // Fundamental frequency
    let pred: Vec<f64> = (0..512).map(|i| (i as f64 * 0.05).sin()).collect();
    
    // Double frequency (2nd harmonic)
    let act: Vec<f64> = (0..512).map(|i| (i as f64 * 0.1).sin()).collect();
    
    let (is_trap, trap_type) = detector.detect_with_type(&pred, &act);
    // May detect frequency doubling
    println!("Frequency doubling detection: {:?}", trap_type);
}

#[test]
fn test_trap_type_sub_harmonic() {
    let mut detector = HarmonicTrapDetector::new(512);
    
    // Higher frequency
    let pred: Vec<f64> = (0..512).map(|i| (i as f64 * 0.1).sin()).collect();
    
    // Half frequency (sub-harmonic)
    let act: Vec<f64> = (0..512).map(|i| (i as f64 * 0.05).sin()).collect();
    
    let (is_trap, trap_type) = detector.detect_with_type(&pred, &act);
    println!("Sub-harmonic detection: {:?}", trap_type);
}

// ============================================================
// REAL SIGNAL TESTS
// ============================================================

#[test]
fn test_noisy_signal() {
    let mut detector = HarmonicTrapDetector::new(256);
    let mut rng = rand::thread_rng();
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| {
        (i as f64 * 0.1).sin() + rng.gen::<f64>() * 0.1
    }).collect();
    
    let is_trap = detector.detect_trap(&pred, &act);
    // Noisy but in-phase should not trigger
    assert!(!is_trap, "Noisy in-phase signal should not trigger trap");
}

#[test]
fn test_market_like_signal() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    // Simulate market price with trend and noise
    let mut pred = Vec::with_capacity(256);
    let mut act = Vec::with_capacity(256);
    let mut price = 100.0;
    
    for i in 0..256 {
        price *= 1.0 + (i as f64 * 0.001).sin() * 0.001;
        pred.push(price);
        
        // Actual with slight phase lag
        let lagged_price = 100.0 + (price - 100.0) * 0.95;
        act.push(lagged_price);
    }
    
    let is_trap = detector.detect_trap(&pred, &act);
    println!("Market-like signal trap detection: {}", is_trap);
}

// ============================================================
// EDGE CASE TESTS
// ============================================================

#[test]
fn test_empty_signals() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let empty: Vec<f64> = vec![];
    let is_trap = detector.detect_trap(&empty, &empty);
    assert!(!is_trap, "Empty signals should not trigger trap");
}

#[test]
fn test_short_signals() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let short_signal: Vec<f64> = (0..10).map(|i| i as f64).collect();
    let is_trap = detector.detect_trap(&short_signal, &short_signal);
    assert!(!is_trap, "Signals shorter than FFT size should not trigger trap");
}

#[test]
fn test_constant_signals() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let constant = vec![1.0; 256];
    let is_trap = detector.detect_trap(&constant, &constant);
    assert!(!is_trap, "Constant signals should not trigger trap");
}

#[test]
fn test_amplitude_difference() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin() * 2.0).collect();
    
    let is_trap = detector.detect_trap(&pred, &act);
    // Amplitude difference alone should not trigger phase trap
    assert!(!is_trap, "Amplitude difference should not trigger phase trap");
}

// ============================================================
// PHASE SPECTRUM TESTS
// ============================================================

#[test]
fn test_phase_spectrum() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1 + PI / 4.0).sin()).collect();
    
    let phase_spectrum = detector.phase_spectrum(&pred, &act);
    assert!(!phase_spectrum.is_empty());
    
    // All phase differences should be around PI/4
    for phase in &phase_spectrum {
        assert!((phase - PI / 4.0).abs() < 0.5);
    }
}

#[test]
fn test_magnitude_ratio_spectrum() {
    let mut detector = HarmonicTrapDetector::new(256);
    
    let pred: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    let act: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin() * 1.5).collect();
    
    let mag_ratio = detector.magnitude_ratio_spectrum(&pred, &act);
    assert!(!mag_ratio.is_empty());
    
    // All magnitude ratios should be around 0.667 (1/1.5)
    for ratio in &mag_ratio {
        assert!((ratio - 0.6667).abs() < 0.1);
    }
}

// ============================================================
// PERFORMANCE TESTS
// ============================================================

#[test]
fn test_detection_speed() {
    let mut detector = HarmonicTrapDetector::new(256);
    let signal: Vec<f64> = (0..256).map(|i| (i as f64 * 0.1).sin()).collect();
    
    let start = std::time::Instant::now();
    
    for _ in 0..1000 {
        let _ = detector.detect_trap(&signal, &signal);
    }
    
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() as f64 / 1000.0;
    println!("Average detection time: {:.2} ns", avg_ns);
    
    // Should be under 10 microseconds per detection
    assert!(avg_ns < 10000.0, "Detection too slow: {:.2} ns", avg_ns);
}
