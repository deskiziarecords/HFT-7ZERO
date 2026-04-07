// ============================================================
// ZERO-COPY MEMORY MANAGEMENT
// ============================================================
// DMA-style buffer access without copying
// Shared memory for inter-process communication
// Memory-mapped files for persistence
// ============================================================

use super::*;
use std::fs::File;
use std::io::{self, Read, Write};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::Arc;
use memmap2::{Mmap, MmapMut, MmapOptions};
use parking_lot::RwLock;

/// Zero-copy buffer for DMA access
#[repr(C, align(64))]
pub struct ZeroCopyBuffer {
    ptr: NonNull<u8>,
    len: usize,
    capacity: usize,
    mmap: Option<MmapMut>,
}

unsafe impl Send for ZeroCopyBuffer {}
unsafe impl Sync for ZeroCopyBuffer {}

impl ZeroCopyBuffer {
    /// Create new zero-copy buffer with specified capacity
    pub fn new(capacity: usize) -> io::Result<Self> {
        // Use huge pages for large allocations
        let aligned_capacity = (capacity + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        
        // Memory map anonymous memory for zero-copy access
        let mmap = MmapOptions::new(aligned_capacity)
            .populate() // Pre-fault pages
            .map_anon()?;
        
        let mut mmap_mut = mmap.make_mut()?;
        let ptr = NonNull::new(mmap_mut.as_mut_ptr()).unwrap();
        
        Ok(Self {
            ptr,
            len: 0,
            capacity: aligned_capacity,
            mmap: Some(mmap_mut),
        })
    }
    
    /// Create from existing memory region
    pub unsafe fn from_raw_parts(ptr: *mut u8, len: usize, capacity: usize) -> Self {
        Self {
            ptr: NonNull::new_unchecked(ptr),
            len,
            capacity,
            mmap: None,
        }
    }
    
    /// Create from file with memory mapping
    pub fn from_file(path: &std::path::Path, size: usize) -> io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new(size).map(&file)? };
        let mmap_mut = mmap.make_mut()?;
        let ptr = NonNull::new(mmap_mut.as_mut_ptr()).unwrap();
        
        Ok(Self {
            ptr,
            len: size,
            capacity: size,
            mmap: Some(mmap_mut),
        })
    }
    
    /// Write data without copying (direct DMA)
    pub fn write_direct<F>(&mut self, f: F) -> usize
    where
        F: FnOnce(&mut [u8]) -> usize,
    {
        let slice = unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) };
        let written = f(slice);
        self.len = written;
        written
    }
    
    /// Read data without copying
    pub fn read_direct<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let slice = unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) };
        f(slice)
    }
    
    /// Get raw pointer
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }
    
    /// Get mutable raw pointer
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }
    
    /// Get current length
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Clear buffer
    pub fn clear(&mut self) {
        self.len = 0;
    }
    
    /// Advance write position
    pub fn advance(&mut self, n: usize) {
        self.len = (self.len + n).min(self.capacity);
    }
}

impl Deref for ZeroCopyBuffer {
    type Target = [u8];
    
    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl DerefMut for ZeroCopyBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for ZeroCopyBuffer {
    fn drop(&mut self) {
        // Mmap handles cleanup automatically
    }
}

/// Shared memory region for IPC
pub struct SharedMemoryRegion {
    name: String,
    fd: Option<File>,
    mmap: Option<MmapMut>,
    size: usize,
    created: bool,
}

impl SharedMemoryRegion {
    /// Create or open shared memory region
    pub fn new(name: &str, size: usize, create: bool) -> io::Result<Self> {
        use nix::sys::mman::{shm_open, shm_unlink};
        use nix::sys::stat::Mode;
        use std::os::unix::io::FromRawFd;
        
        let flags = if create {
            nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_RDWR
        } else {
            nix::fcntl::OFlag::O_RDWR
        };
        
        let fd = shm_open(name, flags, Mode::S_IRUSR | Mode::S_IWUSR)?;
        let file = unsafe { File::from_raw_fd(fd) };
        
        // Set size
        if create {
            file.set_len(size as u64)?;
        }
        
        let mmap = unsafe { MmapOptions::new(size).map(&file)? };
        let mmap_mut = mmap.make_mut()?;
        
        Ok(Self {
            name: name.to_string(),
            fd: Some(file),
            mmap: Some(mmap_mut),
            size,
            created: create,
        })
    }
    
    /// Get memory as slice
    pub fn as_slice(&self) -> &[u8] {
        if let Some(ref mmap) = self.mmap {
            unsafe { std::slice::from_raw_parts(mmap.as_ptr(), self.size) }
        } else {
            &[]
        }
    }
    
    /// Get mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if let Some(ref mut mmap) = self.mmap {
            unsafe { std::slice::from_raw_parts_mut(mmap.as_mut_ptr(), self.size) }
        } else {
            &mut []
        }
    }
}

impl Drop for SharedMemoryRegion {
    fn drop(&mut self) {
        use nix::sys::mman::shm_unlink;
        if self.created {
            let _ = shm_unlink(&self.name);
        }
    }
}

/// Lock-free ring buffer with zero-copy semantics
#[repr(C, align(64))]
pub struct RingBuffer<T, const N: usize> {
    buffer: [CacheAligned<T>; N],
    head: CacheAligned<AtomicU64>,
    tail: CacheAligned<AtomicU64>,
    mask: u64,
}

impl<T: Default + Copy, const N: usize> RingBuffer<T, N> {
    const CAPACITY: u64 = N as u64;
    
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Size must be power of two");
        
        let buffer = [(); N].map(|_| CacheAligned::new(T::default()));
        
        Self {
            buffer,
            head: CacheAligned::new(AtomicU64::new(0)),
            tail: CacheAligned::new(AtomicU64::new(0)),
            mask: (N - 1) as u64,
        }
    }
    
    /// Push value (zero-copy by reference)
    pub fn push(&self, value: T) -> bool {
        let head = self.head.get().load(Ordering::Acquire);
        let tail = self.tail.get().load(Ordering::Acquire);
        
        if head - tail >= Self::CAPACITY {
            return false;
        }
        
        let idx = (head & self.mask) as usize;
        unsafe {
            std::ptr::write(&mut *(&self.buffer[idx] as *const _ as *mut T), value);
        }
        
        self.head.get().store(head + 1, Ordering::Release);
        true
    }
    
    /// Pop value (zero-copy by reference)
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.get().load(Ordering::Acquire);
        let head = self.head.get().load(Ordering::Acquire);
        
        if tail >= head {
            return None;
        }
        
        let idx = (tail & self.mask) as usize;
        let value = unsafe { std::ptr::read(&self.buffer[idx] as *const _ as *const T) };
        
        self.tail.get().store(tail + 1, Ordering::Release);
        Some(value)
    }
    
    /// Peek without consuming
    pub fn peek(&self) -> Option<&T> {
        let tail = self.tail.get().load(Ordering::Acquire);
        let head = self.head.get().load(Ordering::Acquire);
        
        if tail >= head {
            return None;
        }
        
        let idx = (tail & self.mask) as usize;
        unsafe { Some(&*(&self.buffer[idx] as *const _ as *const T)) }
    }
    
    /// Get current size
    pub fn size(&self) -> u64 {
        let head = self.head.get().load(Ordering::Acquire);
        let tail = self.tail.get().load(Ordering::Acquire);
        head - tail
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }
    
    /// Check if full
    pub fn is_full(&self) -> bool {
        self.size() == Self::CAPACITY
    }
}

/// Scatter-gather list for zero-copy I/O
#[repr(C, align(64))]
pub struct ScatterGatherList {
    iovs: Vec<libc::iovec>,
    buffers: Vec<ZeroCopyBuffer>,
}

impl ScatterGatherList {
    pub fn new() -> Self {
        Self {
            iovs: Vec::new(),
            buffers: Vec::new(),
        }
    }
    
    pub fn add_buffer(&mut self
