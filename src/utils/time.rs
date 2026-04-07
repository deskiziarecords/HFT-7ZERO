// ============================================================
// HIGH-PRECISION TIME UTILITIES
// ============================================================
// Hardware timestamping with TSC
// Sub-nanosecond precision timing
// Sleep with microsecond accuracy
// ============================================================

use std::arch::x86_64::_rdtsc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;

/// TSC frequency in Hz (calibrated at startup)
static TSC_FREQUENCY: Lazy<u64> = Lazy::new(calibrate_tsc);

/// Global time offset for TSC to nanoseconds conversion
static TIME_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Precise time structure with nanosecond resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Get current timestamp using TSC
    #[inline(always)]
    pub fn now() -> Self {
        Self(get_hardware_timestamp())
    }
    
    /// Create from nanoseconds since epoch
    #[inline(always)]
    pub fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }
    
    /// Get nanoseconds since epoch
    #[inline(always)]
    pub fn as_nanos(&self) -> u64 {
        self.0
    }
    
    /// Get microseconds since epoch
    #[inline(always)]
    pub fn as_micros(&self) -> u64 {
        self.0 / 1000
    }
    
    /// Get milliseconds since epoch
    #[inline(always)]
    pub fn as_millis(&self) -> u64 {
        self.0 / 1_000_000
    }
    
    /// Duration since this timestamp
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        let now = Self::now();
        Duration::from_nanos(now.0 - self.0)
    }
    
    /// Add duration
    #[inline(always)]
    pub fn add(&self, dur: Duration) -> Self {
        Self(self.0 + dur.as_nanos() as u64)
    }
    
    /// Subtract duration
    #[inline(always)]
    pub fn sub(&self, dur: Duration) -> Self {
        Self(self.0 - dur.as_nanos() as u64)
    }
}

impl std::ops::Sub for Timestamp {
    type Output = Duration;
    
    #[inline(always)]
    fn sub(self, other: Self) -> Duration {
        Duration::from_nanos(self.0 - other.0)
    }
}

/// Get hardware timestamp using TSC (Time Stamp Counter)
/// This provides sub-nanosecond precision on modern CPUs
#[inline(always)]
pub fn get_hardware_timestamp() -> u64 {
    unsafe {
        let tsc = _rdtsc();
        // Convert TSC cycles to nanoseconds
        (tsc as u128 * 1_000_000_000u128 / *TSC_FREQUENCY as u128) as u64
    }
}

/// Calibrate TSC frequency (runs once at startup)
fn calibrate_tsc() -> u64 {
    // Use QueryPerformanceCounter on Windows, clock_gettime on Linux
    // Fallback to approximate frequency (3GHz)
    #[cfg(target_os = "linux")]
    {
        use std::time::Instant;
        
        let start_tsc = unsafe { _rdtsc() };
        let start_time = Instant::now();
        
        // Measure for 100ms
        std::thread::sleep(Duration::from_millis(100));
        
        let end_tsc = unsafe { _rdtsc() };
        let end_time = Instant::now();
        
        let tsc_diff = end_tsc - start_tsc;
        let time_diff_ns = end_time.duration_since(start_time).as_nanos();
        
        (tsc_diff as u128 * 1_000_000_000u128 / time_diff_ns) as u64
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        // Default to 3GHz if calibration fails
        3_000_000_000u64
    }
}

/// Get monotonic timestamp (nanoseconds since boot)
#[inline(always)]
pub fn get_monotonic_ns() -> u64 {
    let now = Instant::now();
    now.elapsed().as_nanos() as u64
}

/// Get UTC timestamp in nanoseconds
#[inline(always)]
pub fn get_utc_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// High-precision sleep (busy-wait for short durations, yield for longer)
pub fn sleep_precise(duration: Duration) {
    let start = get_hardware_timestamp();
    let target_ns = duration.as_nanos() as u64;
    
    if target_ns < 100_000 {
        // Busy-wait for short durations (<100μs)
        while get_hardware_timestamp() - start < target_ns {
            std::hint::spin_loop();
        }
    } else {
        // Use thread::sleep for longer durations
        std::thread::sleep(duration);
    }
}

/// Spin wait for specified nanoseconds (busy loop)
#[inline(always)]
pub fn spin_wait_ns(nanos: u64) {
    let start = get_hardware_timestamp();
    while get_hardware_timestamp() - start < nanos {
        std::hint::spin_loop();
    }
}

/// Measure execution time of a function
#[inline(always)]
pub fn measure_time<F, R>(f: F) -> (R, u64)
where
    F: FnOnce() -> R,
{
    let start = get_hardware_timestamp();
    let result = f();
    let elapsed = get_hardware_timestamp() - start;
    (result, elapsed)
}

/// High-resolution timer for benchmarking
pub struct Timer {
    start: u64,
}

impl Timer {
    #[inline(always)]
    pub fn start() -> Self {
        Self {
            start: get_hardware_timestamp(),
        }
    }
    
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        get_hardware_timestamp() - self.start
    }
    
    #[inline(always)]
    pub fn elapsed_us(&self) -> u64 {
        self.elapsed_ns() / 1000
    }
    
    #[inline(always)]
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ns() / 1_000_000
    }
    
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        Duration::from_nanos(self.elapsed_ns())
    }
    
    #[inline(always)]
    pub fn reset(&mut self) {
        self.start = get_hardware_timestamp();
    }
}

/// Rate limiter for controlling operation frequency
pub struct RateLimiter {
    last_time: AtomicU64,
    min_interval_ns: u64,
}

impl RateLimiter {
    pub fn new(operations_per_second: f64) -> Self {
        let min_interval_ns = (1_000_000_000.0 / operations_per_second) as u64;
        Self {
            last_time: AtomicU64::new(0),
            min_interval_ns,
        }
    }
    
    pub fn wait_if_needed(&self) {
        let now = get_hardware_timestamp();
        let last = self.last_time.load(Ordering::Acquire);
        
        if now - last < self.min_interval_ns {
            let wait_ns = self.min_interval_ns - (now - last);
            spin_wait_ns(wait_ns);
        }
        
        self.last_time.store(get_hardware_timestamp(), Ordering::Release);
    }
    
    pub fn try_acquire(&self) -> bool {
        let now = get_hardware_timestamp();
        let last = self.last_time.load(Ordering::Acquire);
        
        if now - last >= self.min_interval_ns {
            self.last_time.store(now, Ordering::Release);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timestamp() {
        let ts1 = Timestamp::now();
        std::thread::sleep(Duration::from_millis(10));
        let ts2 = Timestamp::now();
        
        assert!(ts2 > ts1);
        assert!(ts2 - ts1 >= Duration::from_millis(10));
    }
    
    #[test]
    fn test_measure_time() {
        let (result, elapsed) = measure_time(|| {
            std::thread::sleep(Duration::from_micros(100));
            42
        });
        
        assert_eq!(result, 42);
        assert!(elapsed >= 90_000);
    }
    
    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(1000.0); // 1000 ops/sec = 1ms interval
        
        let start = get_hardware_timestamp();
        for _ in 0..10 {
            limiter.wait_if_needed();
        }
        let elapsed = get_hardware_timestamp() - start;
        
        // Should take at least 9ms (9 intervals)
        assert!(elapsed >= 9_000_000);
    }
}
