// ============================================================
// MEMORY MANAGEMENT MODULE
// ============================================================
// High-performance memory management with:
// - Custom allocator for HFT workloads
// - Cache-aligned structures to prevent false sharing
// - Zero-copy buffers for DMA-like access
// - NUMA-aware memory allocation
// ============================================================

pub mod allocator;
pub mod cache_aligned;
pub mod zero_copy;
pub mod numa;
pub mod pool;

pub use allocator::{HFTAllocator, ArenaAllocator, ObjectPool};
pub use cache_aligned::{CacheAligned, CachePadded, AlignedBuffer};
pub use zero_copy::{ZeroCopyBuffer, SharedMemoryRegion, RingBuffer};
pub use numa::{NumaBinding, NumaNode, MemoryPolicy};
pub use pool::{MemoryPool, PoolConfig, AllocationStats};

use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Memory alignment for optimal cache performance
pub const CACHE_LINE_SIZE: usize = 64;
pub const PAGE_SIZE: usize = 4096;
pub const HUGE_PAGE_SIZE: usize = 2 * 1024 * 1024; // 2MB

/// Memory access patterns for optimization hints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    Sequential,    // Read/write in order
    Random,        // Random access
    Streaming,     // Write once, read never
    Strided,       // Regular stride access
    Mixed,         // Mixed pattern
}

/// Memory region protection flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProtection {
    ReadOnly,
    ReadWrite,
    ExecuteOnly,
    ReadExecute,
    NoAccess,
}

/// Track memory allocation statistics
#[derive(Debug, Default, Clone)]
pub struct MemoryStats {
    pub total_allocated_bytes: AtomicUsize,
    pub peak_allocated_bytes: AtomicUsize,
    pub total_allocations: AtomicUsize,
    pub total_deallocations: AtomicUsize,
    pub page_faults: AtomicUsize,
    pub cache_misses: AtomicUsize,
}

impl MemoryStats {
    pub fn record_allocation(&self, size: usize) {
        let prev = self.total_allocated_bytes.fetch_add(size, Ordering::Relaxed);
        let new = prev + size;
        
        // Update peak
        let mut peak = self.peak_allocated_bytes.load(Ordering::Relaxed);
        while new > peak {
            match self.peak_allocated_bytes.compare_exchange_weak(
                peak, new, Ordering::Release, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
        
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_deallocation(&self, size: usize) {
        self.total_allocated_bytes.fetch_sub(size, Ordering::Relaxed);
        self.total_deallocations.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_page_fault(&self) {
        self.page_faults.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn snapshot(&self) -> MemoryStatsSnapshot {
        MemoryStatsSnapshot {
            total_allocated_bytes: self.total_allocated_bytes.load(Ordering::Relaxed),
            peak_allocated_bytes: self.peak_allocated_bytes.load(Ordering::Relaxed),
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.total_deallocations.load(Ordering::Relaxed),
            page_faults: self.page_faults.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryStatsSnapshot {
    pub total_allocated_bytes: usize,
    pub peak_allocated_bytes: usize,
    pub total_allocations: usize,
    pub total_deallocations: usize,
    pub page_faults: usize,
    pub cache_misses: usize,
}

/// Global memory statistics instance
pub static MEMORY_STATS: MemoryStats = MemoryStats {
    total_allocated_bytes: AtomicUsize::new(0),
    peak_allocated_bytes: AtomicUsize::new(0),
    total_allocations: AtomicUsize::new(0),
    total_deallocations: AtomicUsize::new(0),
    page_faults: AtomicUsize::new(0),
    cache_misses: AtomicUsize::new(0),
};
