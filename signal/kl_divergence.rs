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
    history: VecDeque<DivergenceResult>,
}

impl DistributionComparator {
    /// Create new distribution comparator
    pub fn new(reference: Vec<f64>, epsilon: f64) -> Self {
        Self {
            reference_distribution: reference,
            current_distribution: VecDeque::with_capacity(1000),
            window_size: 1000,
            epsilon,
            history: VecDeque::with_capacity(100),
        }
    }
    
    /// Compute KL divergence D_KL(P || Q)
    pub fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
        let n = p.len().min(q.len());
        let mut kl = 0.0;
        
        for i in 0..n {
            if p[i] > 0.0 && q[i] > 0.0 {
                kl += p[i] * (p[i] / q[i]).ln();
            } else if p[i] > 0.0 {
                // Q has zero where P has mass -> infinite divergence
                return f64::INFINITY;
            }
        }
        
        kl
    }
    
    /// Compute Jensen-Shannon divergence (symmetric)
    pub fn js_divergence(p: &[f64], q: &[f64]) -> f64 {
        let n = p.len().min(q.len());
        let mut m = vec![0.0; n];
        
        for i in 0..n {
            m[i] = (p[i] + q[i]) / 2.0;
        }
        
        (Self::kl_divergence(p, &m) + Self::kl_divergence(q, &m)) / 2.0
    }
    
    /// Compute Wasserstein distance (earth mover's distance) - simplified 1D version
    pub fn wasserstein_distance(p: &[f64], q: &[f64]) -> f64 {
        let n = p.len().min(q.len());
        let mut cum_p = 0.0;
        let mut cum_q = 0.0;
        let mut distance = 0.0;
        
        for i in 0..n {
            cum_p += p[i];
            cum_q += q[i];
            distance += (cum_p - cum_q).abs();
        }
        
        distance / n as f64
    }
    
    /// Update with new power spectral density
    pub fn update(&mut self, psd: &[f64]) -> DivergenceResult {
        // Update current distribution
        for &value in psd {
            self.current_distribution.push_back(value);
        }
        
        // Maintain window size
        while self.current_distribution.len() > self.window_size {
            self.current_distribution.pop_front();
        }
        
        // Compute current distribution as histogram
        let current_hist = self.compute_histogram(&self.current_distribution);
        let ref_hist = self.compute_histogram(&self.reference_distribution);
        
        let kl = Self::kl_divergence(&current_hist, &ref_hist);
        let js = Self::js_divergence(&current_hist, &ref_hist);
        let wasserstein = Self::wasserstein_distance(&current_hist, &ref_hist);
        let is_significant = kl > self.epsilon;
        
        let result = DivergenceResult {
            kl_divergence: kl,
            js_divergence: js,
            wasserstein,
            is_significant,
            epsilon: self.epsilon,
        };
        
        self.history.push_back(result.clone());
        while self.history.len() > 100 {
            self.history.pop_front();
        }
        
        result
    }
    
    /// Compute histogram from distribution (20 bins)
    fn compute_histogram(&self, data: &VecDeque<f64>) -> Vec<f64> {
        const N_BINS: usize = 20;
        let mut hist = vec![0.0; N_BINS];
        
        if data.is_empty() {
            return hist;
        }
        
        let min_val = data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_val = data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_val - min_val;
        
        if range < 1e-8 {
            hist[0] = 1.0;
            return hist;
        }
        
        let bin_width = range / N_BINS as f64;
        
        for &value in data {
            let bin = ((value - min_val) / bin_width) as usize;
            let idx = bin.min(N_BINS - 1);
            hist[idx] += 1.0;
        }
        
        // Normalize to probability distribution
        let total = data.len() as f64;
        for h in hist.iter_mut() {
            *h /= total;
        }
        
        hist
    }
    
    /// Check if chatter should be suppressed (ν_KL < ε)
    pub fn should_suppress_chatter(&self) -> bool {
        if let Some(last) = self.history.back() {
            !last.is_significant  // ν_KL < ε
        } else {
            false
        }
    }
    
    /// Get recent divergence trend
    pub fn divergence_trend(&self) -> f64 {
        if self.history.len() < 5 {
            return 0.0;
        }
        
        let recent: Vec<f64> = self.history.iter()
            .rev()
            .take(5)
            .map(|r| r.kl_divergence)
            .collect();
        
        let sum: f64 = recent.iter().sum();
        let mean = sum / recent.len() as f64;
        
        let slope = if recent.len() > 1 {
            recent.last().unwrap() - recent.first().unwrap()
        } else {
            0.0
        };
        
        slope / (mean + 1e-8)
    }
}

/// Real-time KL divergence tracker for spectral distributions
pub struct KLTracker {
    reference_psd: Vec<f64>,
    current_buffer: VecDeque<Vec<f64>>,
    comparator: DistributionComparator,
    suppression_active: bool,
    suppression_count: u32,
}

impl KLTracker {
    /// Create new KL tracker
    pub fn new(reference_psd: Vec<f64>, epsilon: f64) -> Self {
        let comparator = DistributionComparator::new(reference_psd.clone(), epsilon);
        
        Self {
            reference_psd,
            current_buffer: VecDeque::with_capacity(100),
            comparator,
            suppression_active: false,
            suppression_count: 0,
        }
    }
    
    /// Update with new PSD measurement
    pub fn update(&mut self, psd: &[f64]) -> bool {
        let result = self.comparator.update(psd);
        
        // Suppress chatter when KL divergence is below threshold
        let should_suppress = !result.is_significant;
        
        if should_suppress {
            self.suppression_count += 1;
            if self.suppression_count > 5 {
                self.suppression_active = true;
            }
        } else {
            self.suppression_count = 0;
            self.suppression_active = false;
        }
        
        self.current_buffer.push_back(psd.to_vec());
        while self.current_buffer.len() > 100 {
            self.current_buffer.pop_front();
        }
        
        self.suppression_active
    }
    
    /// Check if chatter suppression is active
    pub fn is_suppressing(&self) -> bool {
        self.suppression_active
    }
    
    /// Get current divergence
    pub fn current_divergence(&self) -> f64 {
        if let Some(last) = self.comparator.history.back() {
            last.kl_divergence
        } else {
            0.0
        }
    }
    
    /// Reset suppression
    pub fn reset_suppression(&mut self) {
        self.suppression_active = false;
        self.suppression_count = 0;
    }
    
    /// Update reference distribution (online learning)
    pub fn update_reference(&mut self, new_reference: Vec<f64>) {
        self.reference_psd = new_reference;
        self.comparator = DistributionComparator::new(self.reference_psd.clone(), self.comparator.epsilon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kl_divergence() {
        let p = vec![0.5, 0.3, 0.2];
        let q = vec![0.4, 0.4, 0.2];
        
        let kl = DistributionComparator::kl_divergence(&p, &q);
        assert!(kl > 0.0);
        
        let js = DistributionComparator::js_divergence(&p, &q);
        assert!(js > 0.0);
    }
    
    #[test]
    fn test_chatter_suppression() {
        let reference = vec![0.1, 0.2, 0.3, 0.2, 0.1, 0.05, 0.03, 0.02];
        let mut tracker = KLTracker::new(reference, 0.01);
        
        // Similar distribution - should suppress
        let similar = vec![0.1, 0.21, 0.29, 0.19, 0.11, 0.05, 0.03, 0.02];
        
        for _ in 0..10 {
            tracker.update(&similar);
        }
        
        assert!(tracker.is_suppressing());
    }
}
