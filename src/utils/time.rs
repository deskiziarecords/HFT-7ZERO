// ============================================================
// TIME UTILITIES
// ============================================================
// High-precision timekeeping and TSC handling
// ============================================================

use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct PreciseTime {
    start: Instant,
}

impl PreciseTime {
    pub fn now() -> Self {
        Self {
            start: Instant::now(),
        }
    }
    
    pub fn elapsed_nanos(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }
}

pub fn get_hardware_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
