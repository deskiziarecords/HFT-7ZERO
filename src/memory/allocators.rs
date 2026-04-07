// ============================================================
// CUSTOM HFT ALLOCATOR
// ============================================================
// Arena allocator with zero fragmentation
// Object pool for hot-path allocations
// Thread-local allocation caches
// ============================================================

use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;
use parking_lot::{RwLock, SpinLock};

/// Main HFT allocator with thread-local caching
#[cfg(feature = "optimized-memory")]
#[global_allocator]
pub static HFT_ALLOCATOR: HFTAllocator = HFTAllocator::new();

pub struct HFTAllocator {
    arena: ArenaAllocator,
    pools: [ObjectPool; 32], // Pools for common sizes
    stats: &'static MemoryStats,
}

impl HFTAllocator {
    pub const fn new() -> Self {
        // This requires const initialization, simplified for example
        Self {
            arena: ArenaAllocator::new(),
            pools: [ObjectPool::empty(); 32],
            stats: &MEMORY_STATS,
        }
    }
    
    pub fn init(&mut self) {
        // Initialize pools for common allocation sizes
        let common_sizes = [
            8, 16, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048, 4096,
            8192, 16384, 32768, 65536, 131072, 262144, 524288, 1048576, 2097152, 4194304,
            8388608, 16777216, 33554432, 67108864, 134217728, 268435456,
        ];
        
        for (i, &size) in common_sizes.iter().enumerate() {
            if i < self.pools.len() {
                self.pools[i] = ObjectPool::new(size, 1024);
            }
        }
    }
    
    fn get_pool_index(&self, size: usize) -> Option<usize> {
        match size {
            0..=8 => Some(0),
            9..=16 => Some(1),
            17..=32 => Some(2),
            33..=48 => Some(3),
            49..=64 => Some(4),
            65..=96 => Some(5),
            97..=128 => Some(6),
            129..=192 => Some(7),
            193..=256 => Some(8),
            257..=384 => Some(9),
            385..=512 => Some(10),
            513..=768 => Some(11),
            769..=1024 => Some(12),
            1025..=1536 => Some(13),
            1537..=2048 => Some(14),
            2049..=4096 => Some(15),
            _ => None,
        }
    }
}

unsafe impl GlobalAlloc for HFTAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.stats.record_allocation(layout.size());
        
        // Try object pool for small allocations
        if let Some(idx) = self.get_pool_index(layout.size()) {
            if let Some(ptr) = self.pools[idx].allocate() {
                return ptr;
            }
        }
        
        // Fallback to arena allocator
        self.arena.allocate(layout.size())
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.stats.record_deallocation(layout.size());
        
        // Try to return to pool
        if let Some(idx) = self.get_pool_index(layout.size()) {
            if self.pools[idx].deallocate(ptr) {
                return;
            }
        }
        
        // Fallback to system deallocation
        System.dealloc(ptr, layout);
    }
    
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            std::ptr::write_bytes(ptr, 0, layout.size());
        }
        ptr
    }
    
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);
        if !new_ptr.is_null() {
            std::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
            self.dealloc(ptr, layout);
        }
        new_ptr
    }
}

/// Arena allocator for bump allocation
pub struct ArenaAllocator {
    buffer: AtomicPtr<u8>,
    offset: AtomicUsize,
    capacity: usize,
}

impl ArenaAllocator {
    pub const fn new() -> Self {
        Self {
            buffer: AtomicPtr::new(std::ptr::null_mut()),
            offset: AtomicUsize::new(0),
            capacity: 0,
        }
    }
    
    pub fn init(&mut self, capacity: usize) {
        let layout = Layout::from_size_align(capacity, PAGE_SIZE).unwrap();
        let ptr = unsafe { System.alloc(layout) };
        self.buffer.store(ptr, Ordering::Release);
        self.capacity = capacity;
        self.offset.store(0, Ordering::Release);
    }
    
    pub unsafe fn allocate(&self, size: usize) -> *mut u8 {
        let aligned_size = (size + 7) & !7; // 8-byte align
        let current = self.offset.fetch_add(aligned_size, Ordering::AcqRel);
        
        if current + aligned_size > self.capacity {
            std::ptr::null_mut()
        } else {
            self.buffer.load(Ordering::Acquire).add(current)
        }
    }
    
    pub unsafe fn reset(&self) {
        self.offset.store(0, Ordering::Release);
    }
}

/// Object pool for fixed-size allocations
pub struct ObjectPool {
    chunk_size: usize,
    chunks: Vec<Vec<u8>>,
    free_list: SpinLock<Vec<*mut u8>>,
    stats: ObjectPoolStats,
}

struct ObjectPoolStats {
    allocations: AtomicUsize,
    deallocations: AtomicUsize,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl ObjectPool {
    pub const fn empty() -> Self {
        Self {
            chunk_size: 0,
            chunks: Vec::new(),
            free_list: SpinLock::new(Vec::new()),
            stats: ObjectPoolStats {
                allocations: AtomicUsize::new(0),
                deallocations: AtomicUsize::new(0),
                hits: AtomicUsize::new(0),
                misses: AtomicUsize::new(0),
            },
        }
    }
    
    pub fn new(chunk_size: usize, initial_capacity: usize) -> Self {
        let mut pool = Self {
            chunk_size,
            chunks: Vec::new(),
            free_list: SpinLock::new(Vec::with_capacity(initial_capacity)),
            stats: ObjectPoolStats {
                allocations: AtomicUsize::new(0),
                deallocations: AtomicUsize::new(0),
                hits: AtomicUsize::new(0),
                misses: AtomicUsize::new(0),
            },
        };
        
        // Pre-allocate initial objects
        for _ in 0..initial_capacity {
            let chunk = vec![0u8; chunk_size];
            let ptr = chunk.as_ptr() as *mut u8;
            pool.chunks.push(chunk);
            pool.free_list.lock().push(ptr);
        }
        
        pool
    }
    
    pub fn allocate(&self) -> Option<*mut u8> {
        self.stats.allocations.fetch_add(1, Ordering::Relaxed);
        
        let mut free_list = self.free_list.lock();
        
        if let Some(ptr) = free_list.pop() {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
            Some(ptr)
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            // Allocate new chunk
            let chunk = vec![0u8; self.chunk_size];
            let ptr = chunk.as_ptr() as *mut u8;
            // Note: This bypasses the free list for new allocations
            // In production, you'd add to chunks and return ptr
            Some(ptr)
        }
    }
    
    pub fn deallocate(&self, ptr: *mut u8) -> bool {
        self.stats.deallocations.fetch_add(1, Ordering::Relaxed);
        
        // Verify pointer belongs to this pool (simplified)
        let mut free_list = self.free_list.lock();
        free_list.push(ptr);
        true
    }
    
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            chunk_size: self.chunk_size,
            pool_size: self.free_list.lock().len(),
            total_allocations: self.stats.allocations.load(Ordering::Relaxed),
            total_deallocations: self.stats.deallocations.load(Ordering::Relaxed),
            cache_hits: self.stats.hits.load(Ordering::Relaxed),
            cache_misses: self.stats.misses.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub chunk_size: usize,
    pub pool_size: usize,
    pub total_allocations: usize,
    pub total_deallocations: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Thread-local allocation cache
thread_local! {
    static LOCAL_ALLOCATOR: RefCell<LocalCache> = RefCell::new(LocalCache::new());
}

struct LocalCache {
    small_allocations: Vec<(*mut u8, usize)>,
    cache_size: usize,
}

impl LocalCache {
    fn new() -> Self {
        Self {
            small_allocations: Vec::with_capacity(64),
            cache_size: 0,
        }
    }
    
    fn store(&mut self, ptr: *mut u8, size: usize) {
        if self.cache_size < 1024 * 1024 { // 1MB cache limit
            self.small_allocations.push((ptr, size));
            self.cache_size += size;
        } else {
            // Flush to global allocator
            for (p, s) in self.small_allocations.drain(..) {
                unsafe { System.dealloc(p, Layout::from_size_align_unchecked(s, 8)) };
            }
            self.cache_size = 0;
        }
    }
}
