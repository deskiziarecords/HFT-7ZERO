// ============================================================
// CACHE-ALIGNED MEMORY STRUCTURES
// ============================================================
// Prevents false sharing between CPU cores
// Ensures optimal cache line utilization
// ============================================================

use super::CACHE_LINE_SIZE;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicPtr, Ordering};

/// Wrapper that ensures type is cache-line aligned
#[repr(C, align(64))]
#[derive(Debug)]
pub struct CacheAligned<T> {
    value: MaybeUninit<T>,
    _padding: [u8; CACHE_LINE_SIZE - core::mem::size_of::<T>() % CACHE_LINE_SIZE],
}

impl<T> CacheAligned<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: MaybeUninit::new(value),
            _padding: [0; CACHE_LINE_SIZE - core::mem::size_of::<T>() % CACHE_LINE_SIZE],
        }
    }
    
    pub fn get(&self) -> &T {
        unsafe { self.value.assume_init_ref() }
    }
    
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { self.value.assume_init_mut() }
    }
    
    pub fn into_inner(self) -> T {
        unsafe { self.value.assume_init() }
    }
}

impl<T> Deref for CacheAligned<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> DerefMut for CacheAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T> Default for CacheAligned<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Clone for CacheAligned<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.get().clone())
    }
}

/// Cache-padded version for hot variables
#[repr(C, align(64))]
#[derive(Debug)]
pub struct CachePadded<T> {
    value: T,
    _padding: [u8; CACHE_LINE_SIZE - core::mem::size_of::<T>() % CACHE_LINE_SIZE],
}

impl<T> CachePadded<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            _padding: [0; CACHE_LINE_SIZE - core::mem::size_of::<T>() % CACHE_LINE_SIZE],
        }
    }
    
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Atomic cache-padded integer for lock-free structures
pub type AtomicAlignedU64 = CachePadded<AtomicU64>;
pub type AtomicAlignedBool = CachePadded<AtomicBool>;
pub type AtomicAlignedPtr<T> = CachePadded<AtomicPtr<T>>;

/// Aligned buffer for DMA and SIMD operations
#[repr(C, align(64))]
pub struct AlignedBuffer {
    data: [u8; CACHE_LINE_SIZE],
    len: usize,
}

impl AlignedBuffer {
    pub fn new(size: usize) -> Self {
        let aligned_size = ((size + CACHE_LINE_SIZE - 1) / CACHE_LINE_SIZE) * CACHE_LINE_SIZE;
        let mut data = [0u8; CACHE_LINE_SIZE];
        
        // In production, this would allocate dynamically
        // Using stack array for simplicity here
        
        Self {
            data,
            len: aligned_size,
        }
    }
    
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }
    
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }
    
    pub fn len(&self) -> usize {
        self.len
    }
}

/// Prevents false sharing for counters accessed by multiple threads
pub struct ShardedCounter {
    shards: Vec<CacheAligned<AtomicU64>>,
    num_shards: usize,
}

impl ShardedCounter {
    pub fn new(num_shards: usize) -> Self {
        let num_shards = num_shards.next_power_of_two();
        let mut shards = Vec::with_capacity(num_shards);
        
        for _ in 0..num_shards {
            shards.push(CacheAligned::new(AtomicU64::new(0)));
        }
        
        Self { shards, num_shards }
    }
    
    fn shard_index(&self, key: u64) -> usize {
        (key as usize) & (self.num_shards - 1)
    }
    
    pub fn increment(&self, key: u64) -> u64 {
        let idx = self.shard_index(key);
        self.shards[idx].get().fetch_add(1, Ordering::Relaxed) + 1
    }
    
    pub fn add(&self, key: u64, value: u64) -> u64 {
        let idx = self.shard_index(key);
        self.shards[idx].get().fetch_add(value, Ordering::Relaxed) + value
    }
    
    pub fn get(&self, key: u64) -> u64 {
        let idx = self.shard_index(key);
        self.shards[idx].get().load(Ordering::Relaxed)
    }
    
    pub fn total(&self) -> u64 {
        self.shards.iter().map(|s| s.get().load(Ordering::Relaxed)).sum()
    }
    
    pub fn reset(&self) {
        for shard in &self.shards {
            shard.get().store(0, Ordering::Relaxed);
        }
    }
}

/// Ring buffer with cache-aligned slots
#[repr(C, align(64))]
pub struct AlignedRingBuffer<T, const N: usize> {
    slots: [CacheAligned<T>; N],
    read_idx: CacheAligned<AtomicU64>,
    write_idx: CacheAligned<AtomicU64>,
}

impl<T: Default + Copy, const N: usize> AlignedRingBuffer<T, N> {
    pub fn new() -> Self {
        let slots = [(); N].map(|_| CacheAligned::new(T::default()));
        
        Self {
            slots,
            read_idx: CacheAligned::new(AtomicU64::new(0)),
            write_idx: CacheAligned::new(AtomicU64::new(0)),
        }
    }
    
    pub fn push(&self, value: T) -> bool {
        let write = self.write_idx.get().load(Ordering::Acquire);
        let read = self.read_idx.get().load(Ordering::Acquire);
        
        if write - read >= N as u64 {
            return false; // Full
        }
        
        let idx = (write % N as u64) as usize;
        unsafe {
            std::ptr::write(&mut *(&self.slots[idx] as *const _ as *mut T), value);
        }
        
        self.write_idx.get().store(write + 1, Ordering::Release);
        true
    }
    
    pub fn pop(&self) -> Option<T> {
        let read = self.read_idx.get().load(Ordering::Acquire);
        let write = self.write_idx.get().load(Ordering::Acquire);
        
        if read >= write {
            return None; // Empty
        }
        
        let idx = (read % N as u64) as usize;
        let value = unsafe { std::ptr::read(&self.slots[idx] as *const _ as *const T) };
        
        self.read_idx.get().store(read + 1, Ordering::Release);
        Some(value)
    }
    
    pub fn is_empty(&self) -> bool {
        let read = self.read_idx.get().load(Ordering::Acquire);
        let write = self.write_idx.get().load(Ordering::Acquire);
        read >= write
    }
    
    pub fn is_full(&self) -> bool {
        let write = self.write_idx.get().load(Ordering::Acquire);
        let read = self.read_idx.get().load(Ordering::Acquire);
        write - read >= N as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_cache_aligned() {
        let aligned = CacheAligned::new(42);
        assert_eq!(*aligned, 42);
        assert_eq!(core::mem::align_of_val(&aligned), CACHE_LINE_SIZE);
    }
    
    #[test]
    fn test_sharded_counter() {
        let counter = ShardedCounter::new(8);
        
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let counter = &counter;
                thread::spawn(move || {
                    for i in 0..1000 {
                        counter.increment(i);
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(counter.total(), 4000);
    }
    
    #[test]
    fn test_aligned_ring_buffer() {
        let buffer: AlignedRingBuffer<u64, 64> = AlignedRingBuffer::new();
        
        assert!(buffer.push(42));
        assert!(buffer.push(43));
        
        assert_eq!(buffer.pop(), Some(42));
        assert_eq!(buffer.pop(), Some(43));
        assert!(buffer.pop().is_none());
    }
}
