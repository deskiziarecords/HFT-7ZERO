// ============================================================
// KL DIVERGENCE (ν_KL)
// ============================================================
// D_KL(P_PSD || Q_PSD) for distribution comparison
// Chatter suppression when ν_KL < ε
// Real-time distribution tracking
// ============================================================

use super::*;
use std::collections::VecDeque;

/// KL divergence result
#[derive(Debug, Clone)]
pub struct DivergenceResult {
    pub kl_divergence: f64,
    pub js_divergence: f64,  // Jensen-Shannon (symmetric)
    pub wasserstein: f64,    // Earth mover's distance
    pub is_significant: bool,
    pub epsilon: f64,
}

/// Distribution comparator for real-time monitoring
pub struct DistributionComparator {
    reference_distribution: Vec<f64>,
    current_distribution: VecDeque<f64>,
    window_size: usize,
    epsilon: f64,
    history: VecDeque<Diverg
