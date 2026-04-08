// ============================================================
// ARENA ALLOCATOR
// ============================================================

use std::alloc::{Layout, GlobalAlloc, System};

pub struct HFTAllocator;

unsafe impl GlobalAlloc for HFTAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

pub struct ArenaAllocator;

impl ArenaAllocator {
    pub fn with_capacity(_capacity: usize) -> Self {
        Self
    }
}

pub struct ObjectPool<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ObjectPool<T> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}
