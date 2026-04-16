#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdateFrequency {
    VeryLow,      // Daily (ranges)
    Low,          // Hourly (memory accumulation)
    Medium,       // Minute (sweep targets)
    High,         // Second (gamma)
    Tick,         // Every tick (100us)
    Scheduled,    // Event-driven (news)
    Aperiodic,    // Rare (overrides)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorType {
    L1, L2, L3, L4, L5, L6, L7
}
