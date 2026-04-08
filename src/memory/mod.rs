// ============================================================
// MEMORY MANAGEMENT MODULE
// ============================================================

pub mod allocator;
pub mod cache_aligned;
pub mod zero_copy;
pub mod numa;

pub use allocator::{HFTAllocator, ArenaAllocator, ObjectPool};
pub use cache_aligned::CacheAligned;
pub use zero_copy::{ZeroCopyBuffer, SharedMemoryRegion, RingBuffer};

pub const CACHE_LINE_SIZE: usize = 64;
pub const PAGE_SIZE: usize = 4096;

pub struct MemoryStats;
pub static MEMORY_STATS: MemoryStats = MemoryStats;
