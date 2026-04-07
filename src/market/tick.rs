// ============================================================
// TICK DATA STRUCTURE
// ============================================================
// Hardware-timestamped market ticks
// Zero-copy deserialization
// Cache-aligned for performance
// ============================================================

use super::*;
use bytemuck::{Pod, Zeroable};

/// Market tick types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TickType {
    Bid = 0,
    Ask = 1,
    Trade = 2,
    Snapshot = 3,
    Cancel = 4,
    Modify = 5,
}

/// Tick flags for additional information
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum TickFlags {
    None = 0,
    Iceberg = 1 << 0,
    Implied = 1 << 1,
    BlockTrade = 1 << 2,
    OddLot = 1 << 3,
    CrossTrade = 1 << 4,
}

/// Main tick structure (cache-aligned, 64 bytes)
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Tick {
    pub price: f64,
    pub volume: f64,
    pub timestamp_ns: u64,
    pub exchange_id: u8,
    pub side: u8,        // 0=buy, 1=sell
    pub tick_type: u8,   // TickType as u8
    pub flags: u32,      // TickFlags bitmask
    pub sequence: u32,
    pub instrument_id: u32,
    pub trade_id: u64,
    _padding: [u8; 16],
}

unsafe impl Send for Tick {}
unsafe impl Sync for Tick {}

impl Default for Tick {
    fn default() -> Self {
        Self {
            price: 0.0,
            volume: 0.0,
            timestamp_ns: 0,
            exchange_id: 0,
            side: 0,
            tick_type: 0,
            flags: 0,
            sequence: 0,
            instrument_id: 0,
            trade_id: 0,
            _padding: [0; 16],
        }
    }
}

impl Tick {
    /// Create new bid tick
    pub fn bid(price: f64, volume: f64, timestamp_ns: u64, exchange_id: u8) -> Self {
        Self {
            price,
            volume,
            timestamp_ns,
            exchange_id,
            side: 0,
            tick_type: TickType::Bid as u8,
            flags: 0,
            sequence: 0,
            instrument_id: 0,
            trade_id: 0,
            _padding: [0; 16],
        }
    }
    
    /// Create new ask tick
    pub fn ask(price: f64, volume: f64, timestamp_ns: u64, exchange_id: u8) -> Self {
        Self {
            price,
            volume,
            timestamp_ns,
            exchange_id,
            side: 1,
            tick_type: TickType::Ask as u8,
            flags: 0,
            sequence: 0,
            instrument_id: 0,
            trade_id: 0,
            _padding: [0; 16],
        }
    }
    
    /// Create new trade tick
    pub fn trade(price: f64, volume: f64, timestamp_ns: u64, exchange_id: u8, side: u8) -> Self {
        Self {
            price,
            volume,
            timestamp_ns,
            exchange_id,
            side,
            tick_type: TickType::Trade as u8,
            flags: 0,
            sequence: 0,
            instrument_id: 0,
            trade_id: 0,
            _padding: [0; 16],
        }
    }
    
    /// Check if bid
    pub fn is_bid(&self) -> bool {
        self.tick_type == TickType::Bid as u8
    }
    
    /// Check if ask
    pub fn is_ask(&self) -> bool {
        self.tick_type == TickType::Ask as u8
    }
    
    /// Check if trade
    pub fn is_trade(&self) -> bool {
        self.tick_type == TickType::Trade as u8
    }
    
    /// Get tick type
    pub fn tick_type(&self) -> TickType {
        match self.tick_type {
            0 => TickType::Bid,
            1 => TickType::Ask,
            2 => TickType::Trade,
            3 => TickType::Snapshot,
            4 => TickType::Cancel,
            5 => TickType::Modify,
            _ => TickType::Bid,
        }
    }
    
    /// Check if flag is set
    pub fn has_flag(&self, flag: TickFlags) -> bool {
        (self.flags & flag as u32) != 0
    }
    
    /// Set flag
    pub fn set_flag(&mut self, flag: TickFlags) {
        self.flags |= flag as u32;
    }
    
    /// Clear flag
    pub fn clear_flag(&mut self, flag: TickFlags) {
        self.flags &= !(flag as u32);
    }
}

/// Tick batch for processing
#[repr(C, align(64))]
pub struct TickBatch {
    ticks: [Tick; 64],
    count: usize,
    timestamp_ns: u64,
}

impl TickBatch {
    pub fn new() -> Self {
        Self {
            ticks: [Tick::default(); 64],
            count: 0,
            timestamp_ns: 0,
        }
    }
    
    pub fn push(&mut self, tick: Tick) -> bool {
        if self.count < 64 {
            self.ticks[self.count] = tick;
            self.count += 1;
            true
        } else {
            false
        }
    }
    
    pub fn is_full(&self) -> bool {
        self.count == 64
    }
    
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    pub fn len(&self) -> usize {
        self.count
    }
    
    pub fn clear(&mut self) {
        self.count = 0;
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &Tick> {
        self.ticks[..self.count].iter()
    }
}

/// Tick statistics for monitoring
#[derive(Debug, Default)]
pub struct TickStats {
    pub total_ticks: u64,
    pub bid_ticks: u64,
    pub ask_ticks: u64,
    pub trade_ticks: u64,
    pub max_tick_rate_ps: f64,
    pub avg_tick_rate_ps: f64,
    pub last_timestamp_ns: u64,
}

impl TickStats {
    pub fn update(&mut self, tick: &Tick) {
        self.total_ticks += 1;
        
        match tick.tick_type() {
            TickType::Bid => self.bid_ticks += 1,
            TickType::Ask => self.ask_ticks += 1,
            TickType::Trade => self.trade_ticks += 1,
            _ => {}
        }
        
        // Update tick rate (simple exponential moving average)
        if self.last_timestamp_ns > 0 {
            let delta_ns = tick.timestamp_ns - self.last_timestamp_ns;
            if delta_ns > 0 {
                let rate = 1_000_000_000.0 / delta_ns as f64;
                self.avg_tick_rate_ps = self.avg_tick_rate_ps * 0.99 + rate * 0.01;
                self.max_tick_rate_ps = self.max_tick_rate_ps.max(rate);
            }
        }
        
        self.last_timestamp_ns = tick.timestamp_ns;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tick_creation() {
        let bid = Tick::bid(100.50, 1000.0, 1234567890, 1);
        assert!(bid.is_bid());
        assert!(!bid.is_ask());
        assert_eq!(bid.price, 100.50);
        
        let ask = Tick::ask(100.55, 500.0, 1234567891, 1);
        assert!(ask.is_ask());
        assert_eq!(ask.price, 100.55);
    }
    
    #[test]
    fn test_tick_flags() {
        let mut tick = Tick::bid(100.00, 1000.0, 1000, 1);
        tick.set_flag(TickFlags::Iceberg);
        assert!(tick.has_flag(TickFlags::Iceberg));
        
        tick.clear_flag(TickFlags::Iceberg);
        assert!(!tick.has_flag(TickFlags::Iceberg));
    }
    
    #[test]
    fn test_tick_batch() {
        let mut batch = TickBatch::new();
        
        for i in 0..64 {
            let tick = Tick::bid(100.00 + i as f64 * 0.01, 1000.0, 1000 + i, 1);
            assert!(batch.push(tick));
        }
        
        assert!(batch.is_full());
        assert!(!batch.push(Tick::default()));
        
        assert_eq!(batch.len(), 64);
    }
}
