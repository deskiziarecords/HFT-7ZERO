// ============================================================
// JITTER GENERATOR (Δt_jitter ~ 𝒰(50, 500) μs)
// ============================================================
// Random timing obfuscation
// Poisson process for natural-looking arrivals
// Adaptive jitter based on market activity
// ============================================================

use super::*;
use rand::Rng;
use std::time::{Duration, Instant};

/// Jitter type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterType {
    Uniform,        // Uniform distribution
    Gaussian,       // Normal distribution
    Poisson,        // Poisson process
    Exponential,    // Exponential distribution
    Adaptive,       // Based on market activity
}

/// Jitter configuration
#[derive(Debug, Clone)]
pub struct JitterConfig {
    pub jitter_type: JitterType,
    pub min_us: u64,
    pub max_us: u64,
    pub mean_us: u64,
    pub std_us: u64,
    pub lambda: f64,        // For Poisson
    pub adaptive_factor: f64,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            jitter_type: JitterType::Uniform,
            min_us: 50,
            max_us: 500,
            mean_us: 200,
            std_us: 100,
            lambda: 0.005,   // 5ms average interval
            adaptive_factor: 1.0,
        }
    }
}

/// Timing obfuscator for stealth execution
pub struct TimingObfuscator {
    config: JitterConfig,
    rng: fastrand::Rng,
    last_jitter: u64,
    last_time: Instant,
    jitter_history: VecDeque<u64>,
}

impl TimingObfuscator {
    /// Create new jitter generator
    pub fn new(config: JitterConfig) -> Self {
        Self {
            config,
            rng: fastrand::Rng::new(),
            last_jitter: 0,
            last_time: Instant::now(),
            jitter_history: VecDeque::with_capacity(1000),
        }
    }
    
    /// Generate jitter delay in microseconds
    pub fn generate(&mut self) -> Duration {
        let jitter_us = match self.config.jitter_type {
            JitterType::Uniform => self.uniform_jitter(),
            JitterType::Gaussian => self.gaussian_jitter(),
            JitterType::Poisson => self.poisson_jitter(),
            JitterType::Exponential => self.exponential_jitter(),
            JitterType::Adaptive => self.adaptive_jitter(),
        };
        
        self.last_jitter = jitter_us;
        self.jitter_history.push_back(jitter_us);
        
        while self.jitter_history.len() > 1000 {
            self.jitter_history.pop_front();
        }
        
        Duration::from_micros(jitter_us)
    }
    
    /// Uniform distribution: 𝒰(min, max)
    fn uniform_jitter(&mut self) -> u64 {
        self.rng.u64(self.config.min_us..=self.config.max_us)
    }
    
    /// Gaussian (normal) distribution
    fn gaussian_jitter(&mut self) -> u64 {
        let mut value = self.rng.f64() * 2.0 - 1.0;
        let mut result = 0.0;
        
        // Box-Muller transform
        for _ in 0..2 {
            let u1 = self.rng.f64();
            let u2 = self.rng.f64();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            result = z;
        }
        
        let jitter = (self.config.mean_us as f64 + result * self.config.std_us as f64) as u64;
        jitter.clamp(self.config.min_us, self.config.max_us)
    }
    
    /// Poisson process (arrival times)
    fn poisson_jitter(&mut self) -> u64 {
        // Time until next event in Poisson process
        let u = self.rng.f64();
        let interval = -u.ln() / self.config.lambda;
        (interval * 1000.0) as u64  // Convert to microseconds
    }
    
    /// Exponential distribution
    fn exponential_jitter(&mut self) -> u64 {
        let u = self.rng.f64();
        let rate = 1.0 / self.config.mean_us as f64;
        let jitter = -u.ln() / rate;
        (jitter as u64).clamp(self.config.min_us, self.config.max_us)
    }
    
    /// Adaptive jitter based on market activity
    fn adaptive_jitter(&mut self) -> u64 {
        // Measure time since last jitter
        let elapsed = self.last_time.elapsed().as_micros() as u64;
        self.last_time = Instant::now();
        
        // Increase jitter if we're being too regular
        let regularity = self.compute_regularity();
        
        let base_jitter = self.uniform_jitter();
        let adaptive_jitter = (base_jitter as f64 * (1.0 + regularity * self.config.adaptive_factor)) as u64;
        
        adaptive_jitter.clamp(self.config.min_us, self.config.max_us)
    }
    
    /// Compute how regular our jitter pattern is (0 = random, 1 = very regular)
    fn compute_regularity(&self) -> f64 {
        if self.jitter_history.len() < 10 {
            return 0.0;
        }
        
        let recent: Vec<u64> = self.jitter_history.iter().rev().take(20).copied().collect();
        let mean = recent.iter().sum::<u64>() as f64 / recent.len() as f64;
        let variance = recent.iter()
            .map(|&j| (j as f64 - mean).powi(2))
            .sum::<f64>() / recent.len() as f64;
        let std_dev = variance.sqrt();
        
        // Low standard deviation = high regularity
        let regularity = 1.0 / (1.0 + std_dev / mean);
        regularity.clamp(0.0, 1.0)
    }
    
    /// Generate jitter for a batch of orders
    pub fn batch_jitter(&mut self, count: usize) -> Vec<Duration> {
        let mut jitters = Vec::with_capacity(count);
        for _ in 0..count {
            jitters.push(self.generate());
        }
        jitters
    }
    
    /// Get jitter statistics
    pub fn stats(&self) -> JitterStats {
        let mean = if !self.jitter_history.is_empty() {
            self.jitter_history.iter().sum::<u64>() as f64 / self.jitter_history.len() as f64
        } else {
            0.0
        };
        
        let variance = if !self.jitter_history.is_empty() {
            self.jitter_history.iter()
                .map(|&j| (j as f64 - mean).powi(2))
                .sum::<f64>() / self.jitter_history.len() as f64
        } else {
            0.0
        };
        
        JitterStats {
            count: self.jitter_history.len(),
            mean_us: mean as u64,
            std_us: variance.sqrt() as u64,
            min_us: self.jitter_history.iter().min().copied().unwrap_or(0),
            max_us: self.jitter_history.iter().max().copied().unwrap_or(0),
            last_jitter_us: self.last_jitter,
        }
    }
    
    /// Reset jitter state
    pub fn reset(&mut self) {
        self.jitter_history.clear();
        self.last_time = Instant::now();
        self.last_jitter = 0;
    }
}

/// Jitter statistics
#[derive(Debug, Clone)]
pub struct JitterStats {
    pub count: usize,
    pub mean_us: u64,
    pub std_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub last_jitter_us: u64,
}

/// Anti-pattern detection for jitter
pub struct AntiPatternDetector {
    history: VecDeque<u64>,
    threshold: f64,
}

impl AntiPatternDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            history: VecDeque::with_capacity(100),
            threshold,
        }
    }
    
    pub fn record(&mut self, jitter_us: u64) -> bool {
        self.history.push_back(jitter_us);
        while self.history.len() > 50 {
            self.history.pop_front();
        }
        
        self.detect_pattern()
    }
    
    fn detect_pattern(&self) -> bool {
        if self.history.len() < 10 {
            return false;
        }
        
        // Check for periodic patterns using autocorrelation
        let values: Vec<f64> = self.history.iter().map(|&v| v as f64).collect();
        
        for lag in 1..10 {
            let mut corr = 0.0;
            for i in lag..values.len() {
                corr += values[i] * values[i - lag];
            }
            corr /= (values.len() - lag) as f64;
            
            if corr > self.threshold {
                return true;  // Periodic pattern detected
            }
        }
        
        false
    }
    
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jitter_generation() {
        let config = JitterConfig::default();
        let mut jitter_gen = TimingObfuscator::new(config);
        
        // Uniform jitter
        for _ in 0..100 {
            let jitter = jitter_gen.generate();
            let us = jitter.as_micros() as u64;
            assert!(us >= 50 && us <= 500);
        }
        
        let stats = jitter_gen.stats();
        println!("Jitter stats: {:?}", stats);
        assert!(stats.mean_us >= 50 && stats.mean_us <= 500);
    }
    
    #[test]
    fn test_gaussian_jitter() {
        let config = JitterConfig {
            jitter_type: JitterType::Gaussian,
            mean_us: 250,
            std_us: 50,
            ..Default::default()
        };
        
        let mut jitter_gen = TimingObfuscator::new(config);
        
        for _ in 0..1000 {
            let jitter = jitter_gen.generate();
            let us = jitter.as_micros() as u64;
            assert!(us >= 50 && us <= 500);
        }
        
        let stats = jitter_gen.stats();
        assert!((stats.mean_us as i64 - 250).abs() < 100);
    }
}
