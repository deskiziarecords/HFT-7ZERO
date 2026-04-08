// ============================================================
// RISK TRIGGERS
// ============================================================
// Definitions of risk trigger types and severities
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerType {
    Lambda1, // Volatility regime
    Lambda2, // Kurtosis/drift
    Lambda3, // Harmonic trap
    Lambda4, // Fill probability
    Lambda5, // Potential gradient
    Lambda6, // Candle body ratio
    Manual,  // Manual intervention
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

pub struct RiskTriggers;
