// ============================================================
// LOCK-FREE RING BUFFERS
// ============================================================
// Multi-producer single-consumer (MPSC) ring buffer
// Single-producer single-consumer (SPSC) ring buffer
// Cache-aligned for optimal performance
// ============================================================

use super::*;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Multi-producer single-consumer ring buffer
pub struct MPSCRingBuffer {
    buffer: UnsafeCell<Vec<u8>>,
    capacity: usize,
    write_index: AtomicU64,
    read_index: AtomicU64,
    mask: u64,
}

unsafe impl Send for MPSCRingBuffer {}
unsafe impl Sync for MPSCRingBuffer {}

impl MPSCRingBuffer {
    /// Create new MPSC ring buffer with specified capacity (must be power of two)
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of two");
        let buffer = UnsafeCell::new(vec![0u8; capacity]);
        
        Self {
            buffer,
            capacity,
            write_index: AtomicU64::new(0),
            read_index: AtomicU64::new(0),
            mask: (capacity - 1) as u64,
        }
    }
    
    /// Try to write data (non-blocking)
    pub fn try_write(&self, data: &[u8]) -> bool {
        let data_len = data.len() as u64;
        let write = self.write_index.load(Ordering::Acquire);
        let read = self.read_index.load(Ordering::Acquire);
        
        let available = self.capacity as u64 - (write - read);
        if available < data_len + 8 {  // +8 for length prefix
            return false;
        }
        
        let start = (write & self.mask) as usize;
        let end = start + data_len as usize;
        
        unsafe {
            let buf = &mut *self.buffer.get();
            
            if end <= self.capacity {
                // Write length prefix
                buf[start..start + 8].copy_from_slice(&data_len.to_le_bytes());
                // Write data
                buf[start + 8..end + 8].copy_from_slice(data);
            } else {
                // Wrap around
                let first_part = self.capacity - start;
                buf[start..self.capacity].copy_from_slice(&data_len.to_le_bytes()[..first_part]);
                let remaining = data_len as usize + 8 - first_part;
                buf[0..remaining].copy_from_slice(&data[first_part - 8..]);
            }
        }
        
        self.write_index.store(write + data_len + 8, Ordering::Release);
        true
    }
    
    /// Try to read data (non-blocking)
    pub fn try_read(&self, output: &mut Vec<u8>) -> bool {
        let read = self.read_index.load(Ordering::Acquire);
        let write = self.write_index.load(Ordering::Acquire);
        
        if read >= write {
            return false;
        }
        
        let start = (read & self.mask) as usize;
        let mut len_bytes = [0u8; 8];
        
        unsafe {
            let buf = &*self.buffer.get();
            
            if start + 8 <= self.capacity {
                len_bytes.copy_from_slice(&buf[start..start + 8]);
            } else {
                let first_part = self.capacity - start;
                len_bytes[..first_part].copy_from_slice(&buf[start..self.capacity]);
                len_bytes[first_part..].copy_from_slice(&buf[..8 - first_part]);
            }
        }
        
        let data_len = u64::from_le_bytes(len_bytes) as usize;
        let total_len = data_len + 8;
        
        output.clear();
        output.reserve(data_len);
        
        let data_start = (start + 8) & self.mask;
        
        unsafe {
            let buf = &*self.buffer.get();
            
            if data_start + data_len <= self.capacity {
                output.extend_from_slice(&buf[data_start..data_start + data_len]);
            } else {
                let first_part = self.capacity - data_start;
                output.extend_from_slice(&buf[data_start..self.capacity]);
                output.extend_from_slice(&buf[..data_len - first_part]);
            }
        }
        
        self.read_index.store(read + total_len as u64, Ordering::Release);
        true
    }
    
    /// Create reader handle
    pub fn create_reader(&self) -> RingBufferReader {
        RingBufferReader {
            buffer: self as *const _,
            local_read_index: 0,
        }
    }
    
    /// Get approximate fill level
    pub fn fill_level(&self) -> f64 {
        let write = self.write_index.load(Ordering::Acquire);
        let read = self.read_index.load(Ordering::Acquire);
        (write - read) as f64 / self.capacity as f64
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        let write = self.write_index.load(Ordering::Acquire);
        let read = self.read_index.load(Ordering::Acquire);
        write == read
    }
}

/// Ring buffer reader (single consumer)
pub struct RingBufferReader {
    buffer: *const MPSCRingBuffer,
    local_read_index: u64,
}

unsafe impl Send for RingBufferReader {}
unsafe impl Sync for RingBufferReader {}

impl RingBufferReader {
    /// Read next packet
    pub fn read(&mut self, output: &mut Vec<u8>) -> bool {
        let buffer = unsafe { &*self.buffer };
        let write = buffer.write_index.load(Ordering::Acquire);
        
        if self.local_read_index >= write {
            return false;
        }
        
        let start = (self.local_read_index & buffer.mask) as usize;
        let mut len_bytes = [0u8; 8];
        
        unsafe {
            let buf = &*buffer.buffer.get();
            
            if start + 8 <= buffer.capacity {
                len_bytes.copy_from_slice(&buf[start..start + 8]);
            } else {
                let first_part = buffer.capacity - start;
                len_bytes[..first_part].copy_from_slice(&buf[start..buffer.capacity]);
                len_bytes[first_part..].copy_from_slice(&buf[..8 - first_part]);
            }
        }
        
        let data_len = u64::from_le_bytes(len_bytes) as usize;
        let total_len = data_len + 8;
        
        output.clear();
        output.reserve(data_len);
        
        let data_start = (start + 8) & buffer.mask;
        
        unsafe {
            let buf = &*buffer.buffer.get();
            
            if data_start + data_len <= buffer.capacity {
                output.extend_from_slice(&buf[data_start..data_start + data_len]);
            } else {
                let first_part = buffer.capacity - data_start;
                output.extend_from_slice(&buf[data_start..buffer.capacity]);
                output.extend_from_slice(&buf[..data_len - first_part]);
            }
        }
        
        self.local_read_index += total_len as u64;
        buffer.read_index.store(self.local_read_index, Ordering::Release);
        true
    }
}

/// Single-producer single-consumer ring buffer (faster than MPSC)
#[repr(C, align(64))]
pub struct SPSCRingBuffer<T, const N: usize> {
    buffer: [UnsafeCell<T>; N],
    head: CacheAligned<AtomicU64>,
    tail: CacheAligned<AtomicU64>,
    mask: u64,
}

impl<T: Default + Copy, const N: usize> SPSCRingBuffer<T, N> {
    const CAPACITY: u64 = N as u64;
    
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Size must be power of two");
        
        let buffer = [(); N].map(|_| UnsafeCell::new(T::default()));
        
        Self {
            buffer,
            head: CacheAligned::new(AtomicU64::new(0)),
            tail: CacheAligned::new(AtomicU64::new(0)),
            mask: (N - 1) as u64,
        }
    }
    
    /// Push by producer
    pub fn push(&self, value: T) -> bool {
        let head = self.head.get().load(Ordering::Acquire);
        let tail = self.tail.get().load(Ordering::Acquire);
        
        if head - tail >= Self::CAPACITY {
            return false;
        }
        
        let idx = (head & self.mask) as usize;
        unsafe {
            *self.buffer[idx].get() = value;
        }
        
        self.head.get().store(head + 1, Ordering::Release);
        true
    }
    
    /// Pop by consumer
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.get().load(Ordering::Acquire);
        let head = self.head.get().load(Ordering::Acquire);
        
        if tail >= head {
            return None;
        }
        
        let idx = (tail & self.mask) as usize;
        let value = unsafe { *self.buffer[idx].get() };
        
        self.tail.get().store(tail + 1, Ordering::Release);
        Some(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_mpsc_ring_buffer() {
        let buffer = MPSCRingBuffer::new(1024);
        let mut reader = buffer.create_reader();
        
        // Test write/read
        let test_data = b"Hello, HFT!";
        assert!(buffer.try_write(test_data));
        
        let mut output = Vec::new();
        assert!(reader.read(&mut output));
        assert_eq!(&output, test_data);
    }
    
    #[test]
    fn test_spsc_ring_buffer() {
        let buffer: SPSCRingBuffer<u64, 64> = SPSCRingBuffer::new();
        
        assert!(buffer.push(42));
        assert!(buffer.push(43));
        
        assert_eq!(buffer.pop(), Some(42));
        assert_eq!(buffer.pop(), Some(43));
        assert_eq!(buffer.pop(), None);
    }
    
    #[test]
    fn test_parallel_mpsc() {
        let buffer = Arc::new(MPSCRingBuffer::new(1024 * 1024));
        let mut handles = vec![];
        
        // Multiple producers
        for i in 0..4 {
            let buffer = buffer.clone();
            handles.push(thread::spawn(move || {
                let data = format!("Message from thread {}", i);
                for _ in 0..1000 {
                    while !buffer.try_write(data.as_bytes()) {
                        thread::yield_now();
                    }
                }
            }));
        }
        
        // Single consumer
        let mut reader = buffer.create_reader();
        let mut total_messages = 0;
        let mut output = Vec::new();
        
        while total_messages < 4000 {
            if reader.read(&mut output) {
                total_messages += 1;
            }
        }
        
        assert_eq!(total_messages, 4000);
        
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
