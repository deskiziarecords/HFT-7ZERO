// ============================================================
// HIGH-PERFORMANCE I/O MODULE
// ============================================================
// Zero-copy I/O with io_uring
// Real-time packet capture
// Lock-free ring buffers
// ============================================================

pub mod io_uring;
pub mod packet_capture;
pub mod ring_buffer;
pub mod network;
pub mod timestamp;
pub mod zero_copy_io;

pub use io_uring::{IoUringDriver, IoUringConfig, SubmissionQueue, CompletionQueue};
pub use packet_capture::{PacketCapture, PacketFilter, CaptureStats};
pub use ring_buffer::{MPSCRingBuffer, SPSCRingBuffer, RingBufferReader, RingBufferWriter};
pub use network::{UDPReceiver, TCPReceiver, MulticastConfig};
pub use timestamp::{HardwareTimestamp, TimestampSource, ClockId};
pub use zero_copy_io::{ZeroCopyReader, ZeroCopyWriter, ScatterGatherIO};

use std::time::Duration;
use parking_lot::RwLock;

/// I/O performance metrics
#[derive(Debug, Default, Clone)]
pub struct IOMetrics {
    pub packets_received: u64,
    pub bytes_received: u64,
    pub packets_dropped: u64,
    pub avg_latency_ns: u64,
    pub p99_latency_ns: u64,
    pub io_uring_submissions: u64,
    pub io_uring_completions: u64,
}

/// I/O configuration
#[derive(Debug, Clone)]
pub struct IOConfig {
    pub io_uring_queue_depth: u32,
    pub ring_buffer_size: usize,
    pub use_huge_pages: bool,
    pub cpu_affinity: Option<usize>,
    pub numa_node: Option<usize>,
    pub receive_buffer_size: usize,
    pub timestamp_source: TimestampSource,
}

impl Default for IOConfig {
    fn default() -> Self {
        Self {
            io_uring_queue_depth: 4096,
            ring_buffer_size: 1024 * 1024 * 64, // 64MB
            use_huge_pages: true,
            cpu_affinity: None,
            numa_node: None,
            receive_buffer_size: 1024 * 1024 * 8, // 8MB
            timestamp_source: TimestampSource::Tsc,
        }
    }
}

/// Global I/O metrics collector
pub static IO_METRICS: RwLock<IOMetrics> = RwLock::new(IOMetrics::default());

/// Update I/O metrics
#[inline(always)]
pub fn record_io_metrics(packets: u64, bytes: u64, latency_ns: u64) {
    let mut metrics = IO_METRICS.write();
    metrics.packets_received += packets;
    metrics.bytes_received += bytes;
    
    if latency_ns > metrics.p99_latency_ns {
        metrics.p99_latency_ns = latency_ns;
    }
    
    // Exponential moving average for avg latency
    metrics.avg_latency_ns = metrics.avg_latency_ns * 99 / 100 + latency_ns / 100;
}
