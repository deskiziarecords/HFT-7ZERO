// ============================================================
// UTILITIES MODULE
// ============================================================
// High-performance utility functions
// Hardware timestamping
// Fast math operations
// Statistical computations
// Structured logging
// ============================================================

pub mod time;
pub mod math;
pub mod stats;
pub mod logger;
pub mod thread_pool;
pub mod profiler;
pub mod ring_buffer;
pub mod cache;

pub use time::{PreciseTime, get_hardware_timestamp, Timestamp, sleep_precise};
pub use math::{FastMath, exp_approx, log_approx, pow_approx, sigmoid, inv_sqrt};
pub use stats::{Statistics, EWMA, RunningStats, Percentile, Correlation};
pub use logger::{init_logging, LogConfig, Logger, LogEntry, LogLevel};
pub use thread_pool::{AffinityThreadPool, ThreadPool, ThreadAffinity};
pub use profiler::{Profiler, ProfileScope, ProfileData, ProfilerConfig};
pub use ring_buffer::{RingBuffer, SPSCRingBuffer, MPSCRingBuffer};
pub use cache::{Cache, LRUCache, TimedCache, CacheStats};

use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Global utility state
pub struct UtilsState {
    pub start_time: Instant,
    pub uptime_ns: RwLock<u64>,
}

impl UtilsState {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            uptime_ns: RwLock::new(0),
        }
    }
    
    pub fn update_uptime(&self) {
        let mut uptime = self.uptime_ns.write();
        *uptime = self.start_time.elapsed().as_nanos() as u64;
    }
    
    pub fn uptime_ns(&self) -> u64 {
        *self.uptime_ns.read()
    }
}

lazy_static::lazy_static! {
    pub static ref UTILS_STATE: UtilsState = UtilsState::new();
}

/// Initialize utils module
pub fn init() {
    UTILS_STATE.update_uptime();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_utils_init() {
        init();
        assert!(UTILS_STATE.uptime_ns() >= 0);
    }
}
