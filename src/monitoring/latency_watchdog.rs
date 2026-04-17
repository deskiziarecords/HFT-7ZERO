// ============================================================
// LATENCY WATCHDOG
// ============================================================
// Real-time latency monitoring and breach detection
// P99 tracking with sliding windows
// Automatic alerting on violations
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Watchdog configuration
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    pub latency_threshold_ns: u64,
    pub window_size_secs: u64,
    pub breach_threshold: u32,      // Number of breaches before alert
    pub check_interval_ms: u64,
    pub enable_auto_remediation: bool,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            latency_threshold_ns: 1_000_000,  // 1ms
            window_size_secs: 60,              // 60 second window
            breach_threshold: 3,               // 3 breaches triggers alert
            check_interval_ms: 100,            // Check every 100ms
            enable_auto_remediation: true,
        }
    }
}

/// Latency breach event
#[derive(Debug, Clone)]
pub struct LatencyBreach {
    pub operation: String,
    pub latency_ns: u64,
    pub threshold_ns: u64,
    pub timestamp_ns: u64,
    pub severity: BreachSeverity,
}

/// Breach severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreachSeverity {
    Warning,
    Critical,
    Emergency,
}

/// Latency watchdog
pub struct LatencyWatchdog {
    config: WatchdogConfig,
    latency_history: RwLock<VecDeque<(String, u64, u64)>>, // (operation, latency, timestamp)
    breach_count: RwLock<VecDeque<LatencyBreach>>,
    is_healthy: AtomicBool,
    last_check: AtomicU64,
    remediation_triggered: AtomicBool,
}

impl LatencyWatchdog {
    /// Create new latency watchdog
    pub fn new(config: WatchdogConfig) -> Self {
        Self {
            config,
            latency_history: RwLock::new(VecDeque::with_capacity(10000)),
            breach_count: RwLock::new(VecDeque::with_capacity(1000)),
            is_healthy: AtomicBool::new(true),
            last_check: AtomicU64::new(0),
            remediation_triggered: AtomicBool::new(false),
        }
    }

    /// Record latency measurement
    pub fn record_latency(&self, operation: &str, latency_ns: u64) {
        let now = crate::utils::time::get_hardware_timestamp();

        // Store in history
        {
            let mut history = self.latency_history.write();
            history.push_back((operation.to_string(), latency_ns, now));

            // Clean old entries
            let cutoff = now - self.config.window_size_secs * 1_000_000_000;
            while let Some(entry) = history.front() {
                if entry.2 < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }
        }

        // Check for breach
        if latency_ns > self.config.latency_threshold_ns {
            self.record_breach(operation, latency_ns);
        }
    }

    /// Record a latency breach
    fn record_breach(&self, operation: &str, latency_ns: u64) {
        let severity = if latency_ns > self.config.latency_threshold_ns * 2 {
            BreachSeverity::Emergency
        } else if latency_ns > self.config.latency_threshold_ns * 15 / 10 {
            BreachSeverity::Critical
        } else {
            BreachSeverity::Warning
        };

        let breach = LatencyBreach {
            operation: operation.to_string(),
            latency_ns,
            threshold_ns: self.config.latency_threshold_ns,
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            severity,
        };

        {
            let mut breaches = self.breach_count.write();
            breaches.push_back(breach.clone());

            // Keep last 1000 breaches
            while breaches.len() > 1000 {
                breaches.pop_front();
            }
        }

        tracing::warn!(
            "Latency breach: {} = {}ns (threshold: {}ns, severity: {:?})",
            operation, latency_ns, self.config.latency_threshold_ns, severity
        );

        // Check if we need to trigger remediation
        self.check_breach_threshold();
    }

    /// Check if breach threshold exceeded
    fn check_breach_threshold(&self) {
        let now = crate::utils::time::get_hardware_timestamp();
        let window_start = now - self.config.window_size_secs * 1_000_000_000;

        let recent_breaches: Vec<&LatencyBreach> = self.breach_count.read()
            .iter()
            .filter(|b| b.timestamp_ns >= window_start)
            .collect();

        if recent_breaches.len() >= self.config.breach_threshold as usize {
            self.is_healthy.store(false, Ordering::Release);

            if self.config.enable_auto_remediation && !self.remediation_triggered.load(Ordering::Acquire) {
                self.trigger_remediation();
            }
        }
    }

    /// Trigger automatic remediation
    fn trigger_remediation(&self) {
        self.remediation_triggered.store(true, Ordering::Release);

        tracing::error!("Auto-remediation triggered due to latency breaches");

        // Remediation actions:
        // 1. Reduce batch sizes
        // 2. Flush caches
        // 3. Reset connections
        // 4. Throttle trading

        // Simulate remediation
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            // Remediation complete
        });
    }

    /// Get P99 latency for operation
    pub fn p99_latency(&self, operation: &str) -> u64 {
        let history = self.latency_history.read();
        let mut latencies: Vec<u64> = history.iter()
            .filter(|(op, _, _)| op == operation)
            .map(|(_, lat, _)| *lat)
            .collect();

        if latencies.is_empty() {
            return 0;
        }

        latencies.sort();
        let idx = (latencies.len() as f64 * 0.99) as usize;
        latencies[idx.min(latencies.len() - 1)]
    }

    /// Get recent breaches
    pub fn recent_breaches(&self, count: usize) -> Vec<LatencyBreach> {
        self.breach_count.read()
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Acquire)
    }

    /// Reset watchdog
    pub fn reset(&self) {
        self.latency_history.write().clear();
        self.breach_count.write().clear();
        self.is_healthy.store(true, Ordering::Release);
        self.remediation_triggered.store(false, Ordering::Release);
        self.last_check.store(crate::utils::time::get_hardware_timestamp(), Ordering::Release);
    }

    /// Get watchdog statistics
    pub fn stats(&self) -> WatchdogStats {
        let history = self.latency_history.read();
        let breaches = self.breach_count.read();

        let avg_latency: u64 = if !history.is_empty() {
            history.iter().map(|(_, lat, _)| lat).sum::<u64>() / history.len() as u64
        } else {
            0
        };

        WatchdogStats {
            total_samples: history.len(),
            total_breaches: breaches.len(),
            avg_latency_ns: avg_latency,
            p99_latency_ns: self.p99_latency("pipeline"),
            is_healthy: self.is_healthy(),
            last_breach_time: breaches.back().map(|b| b.timestamp_ns).unwrap_or(0),
        }
    }
}

/// Watchdog statistics
#[derive(Debug, Clone)]
pub struct WatchdogStats {
    pub total_samples: usize,
    pub total_breaches: usize,
    pub avg_latency_ns: u64,
    pub p99_latency_ns: u64,
    pub is_healthy: bool,
    pub last_breach_time: u64,
}

/// Real-time latency monitor with sliding window
pub struct LatencyMonitor {
    windows: Vec<SlidingWindow>,
    config: WatchdogConfig,
}

struct SlidingWindow {
    operation: String,
    latencies: VecDeque<u64>,
    timestamps: VecDeque<u64>,
    window_ns: u64,
}

impl SlidingWindow {
    fn new(operation: String, window_ns: u64) -> Self {
        Self {
            operation,
            latencies: VecDeque::new(),
            timestamps: VecDeque::new(),
            window_ns,
        }
    }

    fn add(&mut self, latency_ns: u64, timestamp_ns: u64) {
        self.latencies.push_back(latency_ns);
        self.timestamps.push_back(timestamp_ns);

        self.cleanup(timestamp_ns);
    }

    fn cleanup(&mut self, now_ns: u64) {
        let cutoff = now_ns - self.window_ns;
        while let Some(&ts) = self.timestamps.front() {
            if ts < cutoff {
                self.timestamps.pop_front();
                self.latencies.pop_front();
            } else {
                break;
            }
        }
    }

    fn p99(&self) -> u64 {
        let mut sorted: Vec<u64> = self.latencies.iter().copied().collect();
        if sorted.is_empty() {
            return 0;
        }
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

impl LatencyMonitor {
    pub fn new(config: WatchdogConfig, operations: Vec<String>) -> Self {
        let windows = operations.into_iter()
            .map(|op| SlidingWindow::new(op, config.window_size_secs * 1_000_000_000))
            .collect();

        Self { windows, config }
    }

    pub fn record(&mut self, operation: &str, latency_ns: u64) {
        let now = crate::utils::time::get_hardware_timestamp();

        for window in &mut self.windows {
            if window.operation == operation {
                window.add(latency_ns, now);

                if latency_ns > self.config.latency_threshold_ns {
                    tracing::warn!("High latency in {}: {}ns", operation, latency_ns);
                }
                break;
            }
        }
    }

    pub fn get_p99(&self, operation: &str) -> u64 {
        self.windows.iter()
            .find(|w| w.operation == operation)
            .map(|w| w.p99())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_watchdog() {
        let config = WatchdogConfig {
            latency_threshold_ns: 1_000_000,
            window_size_secs: 10,
            breach_threshold: 3,
            ..Default::default()
        };

        let watchdog = LatencyWatchdog::new(config);

        // Record normal latencies
        for _ in 0..10 {
            watchdog.record_latency("pipeline", 500_000);
        }

        assert!(watchdog.is_healthy());

        // Record breaches
        for _ in 0..5 {
            watchdog.record_latency("pipeline", 2_000_000);
        }

        // Should be unhealthy now
        assert!(!watchdog.is_healthy());

        let stats = watchdog.stats();
        assert!(stats.total_breaches >= 5);
        assert!(stats.p99_latency_ns > 1_000_000);
    }
}
