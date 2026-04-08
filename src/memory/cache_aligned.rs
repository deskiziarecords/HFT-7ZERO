// ============================================================
// CACHE ALIGNED STRUCTURES
// ============================================================

pub struct CacheAligned<T> {
    data: T,
}

impl<T> CacheAligned<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
    
    pub fn get(&self) -> &T {
        &self.data
    }
}
