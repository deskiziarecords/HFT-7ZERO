// ============================================================
// ORDER FRAGMENTATION
// ============================================================
// Splits large orders into smaller fragments
// Randomization to avoid pattern detection
// Adaptive fragment sizing based on liquidity
// ============================================================

use super::*;
use rand::Rng;
use std::collections::VecDeque;

/// Fragment strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentStrategy {
    Uniform,        // Equal-sized fragments
    Geometric,      // Geometrically decreasing sizes
    Random,         // Random sizes
    Adaptive,       // Based on market liquidity
    Poisson,        // Poisson-distributed sizes
}

/// Fragment configuration
#[derive(Debug, Clone)]
pub struct FragmentConfig {
    pub strategy: FragmentStrategy,
    pub min_fragment_size: f64,
    pub max_fragment_size: f64,
    pub num_fragments_min: usize,
    pub num_fragments_max: usize,
    pub random_seed: Option<u64>,
    pub inter_fragment_delay_us: (u64, u64),  // min, max
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self {
            strategy: FragmentStrategy::Random,
            min_fragment_size: 0.001,
            max_fragment_size: 0.01,
            num_fragments_min: 3,
            num_fragments_max: 8,
            random_seed: None,
            inter_fragment_delay_us: (50, 500),
        }
    }
}

/// Order fragment
#[derive(Debug, Clone)]
pub struct OrderFragment {
    pub fragment_id: u64,
    pub volume: f64,
    pub price: f64,
    pub timestamp_ns: u64,
    pub delay_us: u64,
    pub venue: String,
}

/// Fragmenter for order splitting
pub struct Fragmenter {
    config: FragmentConfig,
    rng: fastrand::Rng,
    fragment_history: VecDeque<OrderFragment>,
}

impl Fragmenter {
    /// Create new fragmenter
    pub fn new(config: FragmentConfig) -> Self {
        let rng = if let Some(seed) = config.random_seed {
            fastrand::Rng::with_seed(seed)
        } else {
            fastrand::Rng::new()
        };
        
        Self {
            config,
            rng,
            fragment_history: VecDeque::with_capacity(1000),
        }
    }
    
    /// Fragment an order into multiple pieces
    pub fn fragment(&mut self, total_volume: f64, base_price: f64) -> Vec<OrderFragment> {
        let num_fragments = self.rng.usize(self.config.num_fragments_min..=self.config.num_fragments_max);
        
        let volumes = match self.config.strategy {
            FragmentStrategy::Uniform => self.uniform_fragments(total_volume, num_fragments),
            FragmentStrategy::Geometric => self.geometric_fragments(total_volume, num_fragments),
            FragmentStrategy::Random => self.random_fragments(total_volume, num_fragments),
            FragmentStrategy::Adaptive => self.adaptive_fragments(total_volume, num_fragments),
            FragmentStrategy::Poisson => self.poisson_fragments(total_volume, num_fragments),
        };
        
        let mut fragments = Vec::with_capacity(num_fragments);
        let base_time = crate::utils::time::get_hardware_timestamp();
        
        for (i, volume) in volumes.into_iter().enumerate() {
            // Add price randomization
            let price_noise = (self.rng.f64() - 0.5) * 0.01;
            let price = (base_price + price_noise).max(0.01);
            
            // Random delay between fragments
            let delay_us = self.rng.u64(
                self.config.inter_fragment_delay_us.0..=self.config.inter_fragment_delay_us.1
            );
            
            let fragment = OrderFragment {
                fragment_id: self.rng.u64(..),
                volume,
                price,
                timestamp_ns: base_time + (i as u64 * delay_us * 1000),
                delay_us,
                venue: self.select_random_venue(),
            };
            
            fragments.push(fragment);
            self.fragment_history.push_back(fragment.clone());
        }
        
        while self.fragment_history.len() > 1000 {
            self.fragment_history.pop_front();
        }
        
        fragments
    }
    
    /// Uniform fragment sizes
    fn uniform_fragments(&self, total: f64, n: usize) -> Vec<f64> {
        let size = total / n as f64;
        vec![size; n]
    }
    
    /// Geometrically decreasing fragment sizes
    fn geometric_fragments(&self, total: f64, n: usize) -> Vec<f64> {
        let ratio = 0.7;  // Each fragment is 70% of previous
        let mut sizes = Vec::with_capacity(n);
        
        let mut sum = 0.0;
        let mut current = total * (1.0 - ratio) / (1.0 - ratio.powi(n as i32));
        
        for _ in 0..n {
            sizes.push(current);
            sum += current;
            current *= ratio;
        }
        
        // Normalize to exact total
        let scale = total / sum;
        sizes.iter().map(|&s| s * scale).collect()
    }
    
    /// Random fragment sizes (Dirichlet distribution)
    fn random_fragments(&self, total: f64, n: usize) -> Vec<f64> {
        let mut weights: Vec<f64> = (0..n).map(|_| self.rng.f64()).collect();
        let sum: f64 = weights.iter().sum();
        
        weights.iter()
            .map(|&w| total * w / sum)
            .collect()
    }
    
    /// Adaptive fragment sizes based on market conditions
    fn adaptive_fragments(&self, total: f64, n: usize) -> Vec<f64> {
        // In production, would use market liquidity data
        // For now, use random with bias toward smaller fragments
        let mut sizes = Vec::with_capacity(n);
        
        for i in 0..n {
            let bias = 1.0 - (i as f64 / n as f64) * 0.5;
            let size = total / n as f64 * bias;
            sizes.push(size);
        }
        
        // Normalize
        let sum: f64 = sizes.iter().sum();
        let scale = total / sum;
        sizes.iter().map(|&s| s * scale).collect()
    }
    
    /// Poisson-distributed fragment sizes
    fn poisson_fragments(&self, total: f64, n: usize) -> Vec<f64> {
        let lambda = total / n as f64;
        let mut sizes = Vec::with_capacity(n);
        
        for _ in 0..n {
            let size = self.poisson_sample(lambda);
            sizes.push(size.min(total));
        }
        
        // Normalize
        let sum: f64 = sizes.iter().sum();
        let scale = total / sum;
        sizes.iter().map(|&s| s * scale).collect()
    }
    
    /// Poisson sampling approximation
    fn poisson_sample(&self, lambda: f64) -> f64 {
        let l = (-lambda).exp();
        let mut k = 0.0;
        let mut p = 1.0;
        
        while p > l {
            p *= self.rng.f64();
            k += 1.0;
        }
        
        k - 1.0
    }
    
    /// Select random venue for fragment
    fn select_random_venue(&self) -> String {
        let venues = ["NYSE", "NASDAQ", "CME", "ICE", "LSE", "BATS", "DIRECT_EDGE"];
        let idx = self.rng.usize(0..venues.len());
        venues[idx].to_string()
    }
    
    /// Reassemble fragments back to original order (for tracking)
    pub fn reassemble(&self, fragment_ids: &[u64]) -> f64 {
        let mut total = 0.0;
        
        for fragment in &self.fragment_history {
            if fragment_ids.contains(&fragment.fragment_id) {
                total += fragment.volume;
            }
        }
        
        total
    }
    
    /// Get fragment statistics
    pub fn stats(&self) -> FragmentStats {
        let volumes: Vec<f64> = self.fragment_history.iter().map(|f| f.volume).collect();
        
        FragmentStats {
            total_fragments: self.fragment_history.len(),
            avg_fragment_size: if !volumes.is_empty() {
                volumes.iter().sum::<f64>() / volumes.len() as f64
            } else {
                0.0
            },
            min_fragment_size: volumes.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            max_fragment_size: volumes.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
        }
    }
}

/// Fragment statistics
#[derive(Debug, Clone)]
pub struct FragmentStats {
    pub total_fragments: usize,
    pub avg_fragment_size: f64,
    pub min_fragment_size: f64,
    pub max_fragment_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fragmentation() {
        let config = FragmentConfig::default();
        let mut fragmenter = Fragmenter::new(config);
        
        let total = 0.025;
        let fragments = fragmenter.fragment(total, 100.0);
        
        assert!(fragments.len() >= 3 && fragments.len() <= 8);
        
        let sum: f64 = fragments.iter().map(|f| f.volume).sum();
        assert!((sum - total).abs() < 0.0001);
    }
    
    #[test]
    fn test_different_strategies() {
        let total = 0.025;
        
        for strategy in [FragmentStrategy::Uniform, FragmentStrategy::Geometric, 
                         FragmentStrategy::Random, FragmentStrategy::Adaptive] {
            let config = FragmentConfig {
                strategy,
                ..Default::default()
            };
            let mut fragmenter = Fragmenter::new(config);
            let fragments = fragmenter.fragment(total, 100.0);
            
            let sum: f64 = fragments.iter().map(|f| f.volume).sum();
            assert!((sum - total).abs() < 0.0001, "Failed for {:?}", strategy);
        }
    }
}
