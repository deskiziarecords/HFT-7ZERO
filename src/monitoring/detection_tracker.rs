// ============================================================
// DETECTION TRACKER
// ============================================================
// Tracks detection probability ℙ(detect | strategy)
// Multi-factor risk scoring
// Adaptive stealth adjustment
// ============================================================

use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicF64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;

/// Detection risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum DetectionRiskLevel {
    None = 0,       // ℙ ≈ 0
    VeryLow = 1,    // ℙ < 0.001
    Low = 2,        // ℙ < 0.005
    Medium = 3,     // ℙ < 0.01
    High = 4,       // ℙ < 0.05
    VeryHigh = 5,   // ℙ < 0.10
    Critical = 6,   // ℙ ≥ 0.10
}

impl DetectionRiskLevel {
    pub fn from_score(score: f64) -> Self {
        if score < 0.0001 {
            DetectionRiskLevel::None
        } else if score < 0.001 {
            DetectionRiskLevel::VeryLow
        } else if score < 0.005 {
            DetectionRiskLevel::Low
        } else if score < 0.01 {
            DetectionRiskLevel::Medium
        } else if score < 0.05 {
            DetectionRiskLevel::High
        } else if score < 0.10 {
            DetectionRiskLevel::VeryHigh
        } else {
            DetectionRiskLevel::Critical
        }
    }
}

/// Detection event
#[derive(Debug, Clone)]
pub struct DetectionEvent {
    pub event_type: DetectionEventType,
    pub risk_score: f64,
    pub risk_level: DetectionRiskLevel,
    pub timestamp_ns: u64,
    pub details: String,
}

/// Detection event types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionEventType {
    PatternDetected,      // Repeating pattern found
    VolumeAnomaly,        // Unusual volume concentration
    TimingAnomaly,        // Regular timing detected
    OrderBookInference,   // Order book reconstruction detected
    VenueFlag,            // Exchange flag/suspicion
    CorrelationDetected,  // Correlation with other traders
}

/// Detection tracker for stealth monitoring
pub struct DetectionTracker {
    events: RwLock<VecDeque<DetectionEvent>>,
    risk_score: AtomicF64,
    last_update: AtomicU64,
    max_history: usize,

    // Risk factors
    pattern_regularity: AtomicF64,
    volume_concentration: AtomicF64,
    timing_variance: AtomicF64,
    venue_alerts: AtomicU64,

    // Adaptive parameters
    stealth_multiplier: AtomicF64,
}

impl DetectionTracker {
    /// Create new detection tracker
    pub fn new() -> Self {
        Self {
            events: RwLock::new(VecDeque::with_capacity(10000)),
            risk_score: AtomicF64::new(0.0),
            last_update: AtomicU64::new(0),
            max_history: 10000,
            pattern_regularity: AtomicF64::new(0.0),
            volume_concentration: AtomicF64::new(0.0),
            timing_variance: AtomicF64::new(1.0),
            venue_alerts: AtomicU64::new(0),
            stealth_multiplier: AtomicF64::new(1.0),
        }
    }

    /// Record detection event
    pub fn record_event(&self, event: DetectionEvent) {
        let risk_level = DetectionRiskLevel::from_score(event.risk_score);
        let mut event = event;
        event.risk_level = risk_level;

        {
            let mut events = self.events.write();
            events.push_back(event.clone());

            while events.len() > self.max_history {
                events.pop_front();
            }
        }

        // Update running risk score (exponential moving average)
        let current_risk = self.risk_score.load(Ordering::Relaxed);
        let new_risk = current_risk * 0.9 + event.risk_score * 0.1;
        self.risk_score.store(new_risk, Ordering::Relaxed);

        // Update stealth multiplier inversely proportional to risk
        let stealth = (1.0 - new_risk).max(0.5).min(1.0);
        self.stealth_multiplier.store(stealth, Ordering::Relaxed);

        self.last_update.store(event.timestamp_ns, Ordering::Relaxed);

        // Log significant events
        if risk_level >= DetectionRiskLevel::Medium {
            tracing::warn!(
                "Detection event: {:?} (risk={:.4}, level={:?})",
                event.event_type, event.risk_score, risk_level
            );
        }
    }

    /// Update pattern regularity (0 = random, 1 = perfectly regular)
    pub fn update_pattern_regularity(&self, regularity: f64) {
        self.pattern_regularity.store(regularity, Ordering::Relaxed);

        if regularity > 0.7 {
            let event = DetectionEvent {
                event_type: DetectionEventType::PatternDetected,
                risk_score: regularity * 0.5,
                risk_level: DetectionRiskLevel::from_score(regularity * 0.5),
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                details: format!("Pattern regularity: {:.3}", regularity),
            };
            self.record_event(event);
        }
    }

    /// Update volume concentration (0 = dispersed, 1 = highly concentrated)
    pub fn update_volume_concentration(&self, concentration: f64) {
        self.volume_concentration.store(concentration, Ordering::Relaxed);

        if concentration > 0.5 {
            let event = DetectionEvent {
                event_type: DetectionEventType::VolumeAnomaly,
                risk_score: concentration * 0.3,
                risk_level: DetectionRiskLevel::from_score(concentration * 0.3),
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                details: format!("Volume concentration: {:.3}", concentration),
            };
            self.record_event(event);
        }
    }

    /// Update timing variance (higher = more random, lower = more regular)
    pub fn update_timing_variance(&self, variance: f64) {
        self.timing_variance.store(variance, Ordering::Relaxed);

        // Low variance means regular timing (suspicious)
        if variance < 0.1 {
            let risk = (0.1 - variance) * 5.0;
            let event = DetectionEvent {
                event_type: DetectionEventType::TimingAnomaly,
                risk_score: risk.min(0.8),
                risk_level: DetectionRiskLevel::from_score(risk),
                timestamp_ns: crate::utils::time::get_hardware_timestamp(),
                details: format!("Timing variance: {:.4}", variance),
            };
            self.record_event(event);
        }
    }

    /// Record venue alert
    pub fn record_venue_alert(&self, venue: &str, reason: &str) {
        self.venue_alerts.fetch_add(1, Ordering::Relaxed);

        let alert_count = self.venue_alerts.load(Ordering::Relaxed);
        let risk = (alert_count as f64 * 0.05).min(0.9);

        let event = DetectionEvent {
            event_type: DetectionEventType::VenueFlag,
            risk_score: risk,
            risk_level: DetectionRiskLevel::from_score(risk),
            timestamp_ns: crate::utils::time::get_hardware_timestamp(),
            details: format!("Venue {}: {}", venue, reason),
        };
        self.record_event(event);
    }

    /// Get current detection probability ℙ(detect | strategy)
    pub fn detection_probability(&self) -> f64 {
        self.risk_score.load(Ordering::Relaxed)
    }

    /// Get current risk level
    pub fn current_risk_level(&self) -> DetectionRiskLevel {
        DetectionRiskLevel::from_score(self.detection_probability())
    }

    /// Get stealth multiplier (reduce aggression when risk is high)
    pub fn stealth_multiplier(&self) -> f64 {
        self.stealth_multiplier.load(Ordering::Relaxed)
    }

    /// Get recent events
    pub fn recent_events(&self, count: usize) -> Vec<DetectionEvent> {
        self.events.read()
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> DetectionStats {
        let events = self.events.read();
        let high_risk = events.iter()
            .filter(|e| e.risk_level >= DetectionRiskLevel::High)
            .count();
        let critical = events.iter()
            .filter(|e| e.risk_level >= DetectionRiskLevel::Critical)
            .count();

        DetectionStats {
            total_events: events.len() as u64,
            high_risk_events: high_risk as u64,
            critical_risk_events: critical as u64,
            current_risk_level: self.current_risk_level(),
            last_event_time_ns: self.last_update.load(Ordering::Relaxed),
            avg_risk_score: self.risk_score.load(Ordering::Relaxed),
        }
    }

    /// Check if trading should be paused due to detection risk
    pub fn should_pause(&self) -> bool {
        self.current_risk_level() >= DetectionRiskLevel::High
    }

    /// Check if emergency shutdown needed
    pub fn emergency_needed(&self) -> bool {
        self.current_risk_level() >= DetectionRiskLevel::Critical
    }

    /// Reset tracker
    pub fn reset(&self) {
        self.events.write().clear();
        self.risk_score.store(0.0, Ordering::Relaxed);
        self.stealth_multiplier.store(1.0, Ordering::Relaxed);
        self.pattern_regularity.store(0.0, Ordering::Relaxed);
        self.volume_concentration.store(0.0, Ordering::Relaxed);
        self.timing_variance.store(1.0, Ordering::Relaxed);
        self.venue_alerts.store(0, Ordering::Relaxed);
        self.last_update.store(0, Ordering::Relaxed);
    }
}

/// Risk factor analyzer
pub struct RiskFactorAnalyzer {
    history: VecDeque<RiskFactors>,
    window_size: usize,
}

#[derive(Debug, Clone)]
struct RiskFactors {
    pattern_regularity: f64,
    volume_concentration: f64,
    timing_variance: f64,
    timestamp_ns: u64,
}

impl RiskFactorAnalyzer {
    pub fn new(window_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    pub fn update(&mut self, factors: RiskFactors) {
        self.history.push_back(factors);
        while self.history.len() > self.window_size {
            self.history.pop_front();
        }
    }

    pub fn trend(&self) -> f64 {
        if self.history.len() < 10 {
            return 0.0;
        }

        let recent: Vec<&RiskFactors> = self.history.iter().rev().take(10).collect();
        let older: Vec<&RiskFactors> = self.history.iter().take(10).collect();

        let recent_risk: f64 = recent.iter()
            .map(|f| f.pattern_regularity + f.volume_concentration + (1.0 - f.timing_variance))
            .sum::<f64>() / recent.len() as f64;

        let older_risk: f64 = older.iter()
            .map(|f| f.pattern_regularity + f.volume_concentration + (1.0 - f.timing_variance))
            .sum::<f64>() / older.len() as f64;

        (recent_risk - older_risk) / older_risk.max(0.01)
    }

    pub fn volatility(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let risks: Vec<f64> = self.history.iter()
            .map(|f| f.pattern_regularity + f.volume_concentration + (1.0 - f.timing_variance))
            .collect();

        let mean = risks.iter().sum::<f64>() / risks.len() as f64;
        let variance = risks.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / risks.len() as f64;
        variance.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_tracker() {
        let tracker = DetectionTracker::new();

        // Record some events
        tracker.update_pattern_regularity(0.8);
        tracker.update_volume_concentration(0.6);
        tracker.update_timing_variance(0.05);

        let stats = tracker.get_stats();
        println!("Detection stats: {:?}", stats);

        assert!(tracker.detection_probability() > 0.0);
        assert!(!tracker.should_pause()); // Risk should still be low
    }

    #[test]
    fn test_risk_levels() {
        assert_eq!(DetectionRiskLevel::from_score(0.00005), DetectionRiskLevel::None);
        assert_eq!(DetectionRiskLevel::from_score(0.0005), DetectionRiskLevel::VeryLow);
        assert_eq!(DetectionRiskLevel::from_score(0.003), DetectionRiskLevel::Low);
        assert_eq!(DetectionRiskLevel::from_score(0.008), DetectionRiskLevel::Medium);
        assert_eq!(DetectionRiskLevel::from_score(0.03), DetectionRiskLevel::High);
        assert_eq!(DetectionRiskLevel::from_score(0.08), DetectionRiskLevel::VeryHigh);
        assert_eq!(DetectionRiskLevel::from_score(0.15), DetectionRiskLevel::Critical);
    }
}
