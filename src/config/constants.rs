// ============================================================
// SYSTEM CONSTANTS
// ============================================================
// Hardcoded system limits and thresholds
// Mathematical constants
// Market structure constants
// ============================================================



// ============================================================
// LATENCY BOUNDS (Section I)
// ============================================================

/// Maximum allowed latency from tick to signal (1ms)
pub const MAX_TICK_TO_SIGNAL_LATENCY_NS: u64 = 1_000_000;

/// Total pipeline stage latency budget (1.9ms)
pub const TOTAL_STAGE_BUDGET_NS: u64 = 1_900_000;

/// Individual stage latency budgets (μs)
pub const STAGE1_BUDGET_NS: u64 = 200_000;  // Decode
pub const STAGE2_BUDGET_NS: u64 = 300_000;  // Normalize
pub const STAGE3_BUDGET_NS: u64 = 300_000;  // Risk
pub const STAGE4_BUDGET_NS: u64 = 200_000;  // Signal

// ============================================================
// TRADING CONSTRAINTS (Section V)
// ============================================================

/// Minimum execution volume (lots or %ADV)
pub const MIN_VOLUME: f64 = 0.01;

/// Maximum execution volume
pub const MAX_VOLUME: f64 = 0.05;

/// Minimum acceptable slippage (pips)
pub const MIN_SLIPPAGE_PIPS: f64 = 0.5;

/// Maximum acceptable slippage (pips)
pub const MAX_SLIPPAGE_PIPS: f64 = 1.5;

/// Jitter range microseconds Δt_jitter ~ 𝒰(50, 500) μs
pub const JITTER_MIN_US: u64 = 50;
pub const JITTER_MAX_US: u64 = 500;

// ============================================================
// TIME WINDOWS (Section IV)
// ============================================================

/// Minimum trade horizon (seconds)
pub const MIN_TRADE_HORIZON_SEC: u64 = 15;

/// Maximum trade horizon (seconds)
pub const MAX_TRADE_HORIZON_SEC: u64 = 180;

/// London trading window (UTC)
pub const LONDON_WINDOW_START_SEC: u64 = 8 * 3600;      // 08:00 UTC
pub const LONDON_WINDOW_END_SEC: u64 = 10 * 3600;       // 10:00 UTC

/// New York trading window (UTC)
pub const NY_WINDOW_START_SEC: u64 = 13 * 3600 + 30 * 60; // 13:30 UTC
pub const NY_WINDOW_END_SEC: u64 = 15 * 3600 + 30 * 60;   // 15:30 UTC

// ============================================================
// RISK THRESHOLDS (Section III)
// ============================================================

/// Default δ threshold for λ₁
pub const DEFAULT_DELTA_THRESHOLD: f64 = 0.3;

/// Default γ threshold for λ₂
pub const DEFAULT_GAMMA_THRESHOLD: f64 = 0.2;

/// Default φ threshold for λ₄
pub const DEFAULT_PHI_THRESHOLD: f64 = 0.6;

/// Default τ_max for λ₁ (milliseconds)
pub const DEFAULT_TAU_MAX_MS: u64 = 500;

/// Default body ratio threshold for λ₆
pub const DEFAULT_BODY_RATIO_THRESHOLD: f64 = 0.7;

// ============================================================
// CAUSALITY PARAMETERS (Section IV)
// ============================================================

/// Decay rate for temporal weighting (e^{-0.08τ})
pub const CAUSAL_DECAY_RATE: f64 = 0.08;

/// Default fusion weight
pub const DEFAULT_FUSION_WEIGHT: f64 = 0.5;

/// Minimum adaptive weight
pub const MIN_ADAPTIVE_WEIGHT: f64 = 0.1;

/// Maximum adaptive weight
pub const MAX_ADAPTIVE_WEIGHT: f64 = 0.9;

// ============================================================
// SPECTRAL ANALYSIS (Section II)
// ============================================================

/// Phase threshold for harmonic trap (π/2)
pub const HARMONIC_PHASE_THRESHOLD: f64 = std::f64::consts::PI / 2.0;

/// Default FFT size
pub const DEFAULT_FFT_SIZE: usize = 256;

/// KL divergence epsilon for chatter suppression
pub const KL_EPSILON: f64 = 0.01;

/// Mandra gate energy threshold (ΔE ≥ 2)
pub const MANDRA_ENERGY_THRESHOLD: f64 = 2.0;

// ============================================================
// MARKET MICROSTRUCTURE
// ============================================================

/// Default tick size (varies by instrument)
pub const DEFAULT_TICK_SIZE: f64 = 0.01;

/// Maximum order book depth
pub const MAX_BOOK_DEPTH: usize = 100;

/// Default number of price levels for features
pub const DEFAULT_PRICE_LEVELS: usize = 10;

// ============================================================
// MEMORY & CACHE
// ============================================================

/// Cache line size (bytes)
pub const CACHE_LINE_SIZE: usize = 64;

/// Page size (bytes)
pub const PAGE_SIZE: usize = 4096;

/// Huge page size (bytes)
pub const HUGE_PAGE_SIZE: usize = 2 * 1024 * 1024;

/// Default ring buffer size
pub const DEFAULT_RING_BUFFER_SIZE: usize = 1024 * 1024 * 64; // 64MB

// ============================================================
// IO_URING
// ============================================================

/// Default io_uring queue depth
pub const DEFAULT_IO_URING_QUEUE_DEPTH: u32 = 4096;

/// Default socket receive buffer size
pub const DEFAULT_RECV_BUFFER_SIZE: usize = 1024 * 1024 * 8; // 8MB

// ============================================================
// MATHEMATICAL CONSTANTS
// ============================================================

/// Machine epsilon for floating point comparisons
pub const EPSILON: f64 = 1e-8;

/// Small value to avoid division by zero
pub const SMALL_VALUE: f64 = 1e-12;

/// Log of 2
pub const LN_2: f64 = std::f64::consts::LN_2;

/// Square root of 2π
pub const SQRT_2PI: f64 = 2.5066282746310002;

// ============================================================
// HELPER FUNCTIONS
// ============================================================

/// Convert microseconds to nanoseconds
#[inline(always)]
pub fn us_to_ns(us: u64) -> u64 {
    us * 1000
}

/// Convert nanoseconds to microseconds
#[inline(always)]
pub fn ns_to_us(ns: u64) -> u64 {
    ns / 1000
}

/// Convert milliseconds to nanoseconds
#[inline(always)]
pub fn ms_to_ns(ms: u64) -> u64 {
    ms * 1_000_000
}

/// Convert seconds to nanoseconds
#[inline(always)]
pub fn sec_to_ns(sec: u64) -> u64 {
    sec * 1_000_000_000
}

/// Check if within tolerance
#[inline(always)]
pub fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

/// Clamp value to range
#[inline(always)]
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_constants() {
        assert_eq!(MAX_TICK_TO_SIGNAL_LATENCY_NS, 1_000_000);
        assert_eq!(MIN_VOLUME, 0.01);
        assert_eq!(MAX_VOLUME, 0.05);
        assert_eq!(JITTER_MIN_US, 50);
        assert_eq!(JITTER_MAX_US, 500);
    }
    
    #[test]
    fn test_conversion_functions() {
        assert_eq!(us_to_ns(1000), 1_000_000);
        assert_eq!(ns_to_us(1_000_000), 1000);
        assert_eq!(ms_to_ns(1000), 1_000_000_000);
        assert_eq!(sec_to_ns(1), 1_000_000_000);
    }
}
