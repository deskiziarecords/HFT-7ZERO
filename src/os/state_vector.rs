// ============================================================
// STATE VECTOR ENCODER
// ============================================================
// Encodes all 6 layer states into a single compact string
// Format: L6 L1 L2 L3 L4 L5
// Example: "N A C I T L" = Normal → Accumulation → Accumulating → Idle → Targeting → Linear
// ============================================================

use super::*;

/// Complete system state vector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateVector {
    pub privileged: PrivilegedState,
    pub compiler: CompilerState,
    pub memory: MemoryState,
    pub interrupt: InterruptState,
    pub collector: CollectorState,
    pub amplifier: AmplifierState,
}

impl StateVector {
    /// Create new state vector
    pub fn new(
        privileged: PrivilegedState,
        compiler: CompilerState,
        memory: MemoryState,
        interrupt: InterruptState,
        collector: CollectorState,
        amplifier: AmplifierState,
    ) -> Self {
        Self {
            privileged,
            compiler,
            memory,
            interrupt,
            collector,
            amplifier,
        }
    }
    
    /// Encode to 6-character string
    pub fn encode(&self) -> String {
        format!(
            "{}{}{}{}{}{}",
            self.privileged.as_char(),
            self.compiler.as_char(),
            self.memory.as_char(),
            self.interrupt.as_char(),
            self.collector.as_char(),
            self.amplifier.as_char(),
        )
    }
    
    /// Decode from 6-character string
    pub fn decode(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 6 {
            return None;
        }
        
        Some(StateVector {
            privileged: PrivilegedState::from_char(chars[0])?,
            compiler: CompilerState::from_char(chars[1])?,
            memory: MemoryState::from_char(chars[2])?,
            interrupt: InterruptState::from_char(chars[3])?,
            collector: CollectorState::from_char(chars[4])?,
            amplifier: AmplifierState::from_char(chars[5])?,
        })
    }
    
    /// Get human-readable description
    pub fn describe(&self) -> String {
        format!(
            "{} → {} → {} → {} → {} → {}",
            self.privileged.describe(),
            self.compiler.describe(),
            self.memory.describe(),
            self.interrupt.describe(),
            self.collector.describe(),
            self.amplifier.describe(),
        )
    }
    
    /// Check if system is in a tradable state
    pub fn can_trade(&self) -> bool {
        self.privileged.can_trade()
            && self.compiler.can_trade()
            && self.memory.can_trade()
            && !self.interrupt.is_overridden()
            && self.collector.is_active()
            && self.amplifier.can_trade()
    }
    
    /// Check if any layer is in override
    pub fn has_override(&self) -> bool {
        self.privileged.is_overridden()
            || self.compiler == CompilerState::Invalid
            || self.interrupt == InterruptState::Overridden
            || self.collector == CollectorState::Vetoed
            || self.amplifier == AmplifierState::Overridden
    }
}

// You'll need to add as_char/from_char to your existing state enums
// Add these to your existing compiler.rs, memory.rs, amplifier.rs

/// Compiler state (Layer 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerState {
    Accumulation,   // A
    Manipulation,   // M
    Distribution,   // D
    Invalid,        // I
}

impl CompilerState {
    pub fn as_char(&self) -> char {
        match self {
            CompilerState::Accumulation => 'A',
            CompilerState::Manipulation => 'M',
            CompilerState::Distribution => 'D',
            CompilerState::Invalid => 'I',
        }
    }
    
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' => Some(CompilerState::Accumulation),
            'M' => Some(CompilerState::Manipulation),
            'D' => Some(CompilerState::Distribution),
            'I' => Some(CompilerState::Invalid),
            _ => None,
        }
    }
    
    pub fn describe(&self) -> &'static str {
        match self {
            CompilerState::Accumulation => "ACCUMULATION",
            CompilerState::Manipulation => "MANIPULATION",
            CompilerState::Distribution => "DISTRIBUTION",
            CompilerState::Invalid => "INVALID",
        }
    }
    
    pub fn can_trade(&self) -> bool {
        matches!(self, CompilerState::Accumulation | CompilerState::Manipulation)
    }
}

/// Memory state (Layer 2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryState {
    Accumulating,   // C
    Stable,         // S
    Depleted,       // D
    Exposed,        // E
}

impl MemoryState {
    pub fn as_char(&self) -> char {
        match self {
            MemoryState::Accumulating => 'C',
            MemoryState::Stable => 'S',
            MemoryState::Depleted => 'D',
            MemoryState::Exposed => 'E',
        }
    }
    
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'C' => Some(MemoryState::Accumulating),
            'S' => Some(MemoryState::Stable),
            'D' => Some(MemoryState::Depleted),
            'E' => Some(MemoryState::Exposed),
            _ => None,
        }
    }
    
    pub fn describe(&self) -> &'static str {
        match self {
            MemoryState::Accumulating => "ACCUMULATING",
            MemoryState::Stable => "STABLE",
            MemoryState::Depleted => "DEPLETED",
            MemoryState::Exposed => "EXPOSED",
        }
    }
    
    pub fn can_trade(&self) -> bool {
        matches!(self, MemoryState::Accumulating | MemoryState::Stable)
    }
}

/// Amplifier state (Layer 5)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmplifierState {
    Linear,         // L
    Accumulating,   // A
    Squeeze,        // S
    Exhausted,      // E
    Overridden,     // O
}

impl AmplifierState {
    pub fn as_char(&self) -> char {
        match self {
            AmplifierState::Linear => 'L',
            AmplifierState::Accumulating => 'A',
            AmplifierState::Squeeze => 'S',
            AmplifierState::Exhausted => 'E',
            AmplifierState::Overridden => 'O',
        }
    }
    
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'L' => Some(AmplifierState::Linear),
            'A' => Some(AmplifierState::Accumulating),
            'S' => Some(AmplifierState::Squeeze),
            'E' => Some(AmplifierState::Exhausted),
            'O' => Some(AmplifierState::Overridden),
            _ => None,
        }
    }
    
    pub fn describe(&self) -> &'static str {
        match self {
            AmplifierState::Linear => "LINEAR",
            AmplifierState::Accumulating => "ACCUMULATING",
            AmplifierState::Squeeze => "SQUEEZE",
            AmplifierState::Exhausted => "EXHAUSTED",
            AmplifierState::Overridden => "OVERRIDDEN",
        }
    }
    
    pub fn can_trade(&self) -> bool {
        matches!(self, AmplifierState::Linear | AmplifierState::Accumulating)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_vector_encoding() {
        let vector = StateVector::new(
            PrivilegedState::Normal,
            CompilerState::Accumulation,
            MemoryState::Accumulating,
            InterruptState::Idle,
            CollectorState::Targeting,
            AmplifierState::Linear,
        );
        
        let encoded = vector.encode();
        assert_eq!(encoded, "NACITL");
        
        let decoded = StateVector::decode(&encoded).unwrap();
        assert_eq!(decoded.privileged, vector.privileged);
        assert_eq!(decoded.compiler, vector.compiler);
        assert_eq!(decoded.memory, vector.memory);
        assert_eq!(decoded.interrupt, vector.interrupt);
        assert_eq!(decoded.collector, vector.collector);
        assert_eq!(decoded.amplifier, vector.amplifier);
    }
    
    #[test]
    fn test_can_trade() {
        // Tradable state
        let tradable = StateVector::new(
            PrivilegedState::Normal,
            CompilerState::Accumulation,
            MemoryState::Accumulating,
            InterruptState::Idle,
            CollectorState::Targeting,
            AmplifierState::Linear,
        );
        assert!(tradable.can_trade());
        
        // Non-tradable: Privileged override
        let non_tradable = StateVector::new(
            PrivilegedState::Active,
            CompilerState::Accumulation,
            MemoryState::Accumulating,
            InterruptState::Idle,
            CollectorState::Targeting,
            AmplifierState::Linear,
        );
        assert!(!non_tradable.can_trade());
        
        // Non-tradable: Compiler invalid
        let non_tradable = StateVector::new(
            PrivilegedState::Normal,
            CompilerState::Invalid,
            MemoryState::Accumulating,
            InterruptState::Idle,
            CollectorState::Targeting,
            AmplifierState::Linear,
        );
        assert!(!non_tradable.can_trade());
    }
}
