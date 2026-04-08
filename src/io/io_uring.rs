// ============================================================
// IO_URING DRIVER
// ============================================================

use io_uring::{IoUring};
use memmap2::MmapMut;
use std::io;

#[allow(dead_code)]
pub struct IoUringDriver {
    ring: IoUring,
    _buffer_pool: MmapMut,
}

pub struct IoUringConfig {
    pub queue_depth: u32,
    pub buffer_size: usize,
}

impl Default for IoUringConfig {
    fn default() -> Self {
        Self {
            queue_depth: 4096,
            buffer_size: 64 * 1024 * 1024,
        }
    }
}

impl IoUringDriver {
    pub fn new(config: IoUringConfig) -> io::Result<Self> {
        let ring = IoUring::new(config.queue_depth)?;
        let _buffer_pool = MmapMut::map_anon(config.buffer_size)?;
        Ok(Self { ring, _buffer_pool })
    }
}
