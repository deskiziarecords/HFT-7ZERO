use super::DetectionStats;

pub struct DetectionTracker;
impl DetectionTracker {
    pub fn new() -> Self { Self }
    pub fn get_stats(&self) -> DetectionStats { DetectionStats::default() }
    pub fn record_event(&self, _e: DetectionEvent) {}
}

#[derive(Debug, Clone)]
pub struct DetectionEvent {
    pub risk_level: DetectionRiskLevel,
    pub event_type: String,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectionRiskLevel { Low, Medium, High, VeryHigh, Critical }
impl Default for DetectionRiskLevel { fn default() -> Self { DetectionRiskLevel::Low } }
