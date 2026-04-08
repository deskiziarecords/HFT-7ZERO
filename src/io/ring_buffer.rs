// ============================================================
// LOCK-FREE RING BUFFERS
// ============================================================

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::memory::cache_aligned::CacheAligned;

pub struct MPSCRingBuffer {
    buffer: UnsafeCell<Vec<u8>>,
    capacity: usize,
    write_index: AtomicU64,
    read_index: AtomicU64,
    mask: usize,
}

unsafe impl Send for MPSCRingBuffer {}
unsafe impl Sync for MPSCRingBuffer {}

impl MPSCRingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two());
        Self {
            buffer: UnsafeCell::new(vec![0u8; capacity]),
            capacity,
            write_index: AtomicU64::new(0),
            read_index: AtomicU64::new(0),
            mask: capacity - 1,
        }
    }
    
    pub fn try_write(&self, data: &[u8]) -> bool {
        let data_len = data.len() as u64;
        let total_needed = data_len + 8;
        let write = self.write_index.load(Ordering::Acquire);
        let read = self.read_index.load(Ordering::Acquire);
        
        if self.capacity as u64 - (write - read) < total_needed {
            return false;
        }
        
        let start = (write as usize) & self.mask;
        unsafe {
            let buf = &mut *self.buffer.get();
            if start + total_needed as usize <= self.capacity {
                // Contiguous
                buf[start..start + 8].copy_from_slice(&data_len.to_le_bytes());
                buf[start + 8..start + total_needed as usize].copy_from_slice(data);
            } else {
                // Wrap around
                let mut temp = data_len.to_le_bytes().to_vec();
                temp.extend_from_slice(data);

                let first_part = self.capacity - start;
                buf[start..self.capacity].copy_from_slice(&temp[..first_part]);
                let remaining = temp.len() - first_part;
                buf[0..remaining].copy_from_slice(&temp[first_part..]);
            }
        }
        
        self.write_index.store(write + total_needed, Ordering::Release);
        true
    }
}

pub struct RingBufferReader {
    buffer: *const MPSCRingBuffer,
    local_read_index: u64,
}

unsafe impl Send for RingBufferReader {}
unsafe impl Sync for RingBufferReader {}

impl RingBufferReader {
    pub fn read(&mut self, output: &mut Vec<u8>) -> bool {
        let buffer = unsafe { &*self.buffer };
        let write = buffer.write_index.load(Ordering::Acquire);
        
        if self.local_read_index >= write {
            return false;
        }
        
        let start = (self.local_read_index as usize) & buffer.mask;
        let mut len_bytes = [0u8; 8];
        
        unsafe {
            let buf = &*buffer.buffer.get();
            if start + 8 <= buffer.capacity {
                len_bytes.copy_from_slice(&buf[start..start + 8]);
            } else {
                let first_part = buffer.capacity - start;
                len_bytes[..first_part].copy_from_slice(&buf[start..buffer.capacity]);
                let remaining = 8 - first_part;
                len_bytes[first_part..].copy_from_slice(&buf[0..remaining]);
            }
        }
        
        let data_len = u64::from_le_bytes(len_bytes) as usize;
        let total_len = data_len as u64 + 8;
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
                let remaining = data_len - first_part;
                output.extend_from_slice(&buf[0..remaining]);
            }
        }
        
        self.local_read_index += total_len;
        buffer.read_index.store(self.local_read_index, Ordering::Release);
        true
    }
}

#[allow(dead_code)]
pub struct SPSCRingBuffer<T, const N: usize> {
    _data: [T; N],
    head: CacheAligned<AtomicU64>,
    tail: CacheAligned<AtomicU64>,
}

impl<T: Default + Copy, const N: usize> SPSCRingBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            _data: [T::default(); N],
            head: CacheAligned::new(AtomicU64::new(0)),
            tail: CacheAligned::new(AtomicU64::new(0)),
        }
    }
}

pub struct RingBufferWriter;
