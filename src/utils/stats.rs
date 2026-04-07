// ============================================================
// STATISTICAL COMPUTATIONS
// ============================================================
// Real-time statistics tracking
// Percentile estimation (P50, P95, P99, P999)
// Correlation and covariance
// Distribution fitting
// ============================================================

use super::math::FastMath;
use std::collections::VecDeque;

/// Running statistics (mean, variance, min, max)
pub struct RunningStats {
    n: u64,
    mean: f64,
    m2: f64,  // Sum of squared differences
    min: f64,
    max: f64,
}

impl RunningStats {
    pub fn new() -> Self {
        Self {
            n: 0,
            mean: 0.0,
            m2: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }
    
    pub fn update(&mut self, value: f64) {
        self.n += 1;
        
        // Welford's algorithm for variance
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }
    
    pub fn update_batch(&mut self, values: &[f64]) {
        for &v in values {
            self.update(v);
        }
    }
    
    pub fn count(&self) -> u64 {
        self.n
    }
    
    pub fn mean(&self) -> f64 {
        self.mean
    }
    
    pub fn variance(&self) -> f64 {
        if self.n < 2 {
            0.0
        } else {
            self.m2 / (self.n - 1) as f64
        }
    }
    
    pub fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }
    
    pub fn min(&self) -> f64 {
        self.min
    }
    
    pub fn max(&self) -> f64 {
        self.max
    }
    
    pub fn range(&self) -> f64 {
        self.max - self.min
    }
    
    pub fn reset(&mut self) {
        self.n = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
        self.min = f64::INFINITY;
        self.max = f64::NEG_INFINITY;
    }
}

/// Exponential weighted moving average statistics
pub struct EWMA {
    alpha: f64,
    value: f64,
    variance: f64,
    initialized: bool,
}

impl EWMA {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            value: 0.0,
            variance: 0.0,
            initialized: false,
        }
    }
    
    pub fn update(&mut self, new_value: f64) -> f64 {
        if !self.initialized {
            self.value = new_value;
            self.variance = 0.0;
            self.initialized = true;
        } else {
            let delta = new_value - self.value;
            self.value += self.alpha * delta;
            self.variance = (1.0 - self.alpha) * (self.variance + self.alpha * delta * delta);
        }
        self.value
    }
    
    pub fn value(&self) -> f64 {
        self.value
    }
    
    pub fn variance(&self) -> f64 {
        self.variance
    }
    
    pub fn stddev(&self) -> f64 {
        self.variance.sqrt()
    }
    
    pub fn reset(&mut self) {
        self.initialized = false;
        self.value = 0.0;
        self.variance = 0.0;
    }
}

/// Percentile estimator using P² algorithm
/// (constant memory, single-pass)
pub struct Percentile {
    p: f64,           // Target percentile (e.g., 0.99)
    n: usize,         // Number of observations
    markers: Vec<Marker>,
}

struct Marker {
    position: f64,    // Desired position
    height: f64,      // Current value
    n: usize,         // Number of observations for this marker
}

impl Percentile {
    pub fn new(percentile: f64) -> Self {
        let p = percentile.clamp(0.0, 1.0);
        let mut markers = Vec::with_capacity(5);
        
        // Initialize 5 markers (min, p/2, p, (1+p)/2, max)
        markers.push(Marker { position: 1.0, height: 0.0, n: 0 });
        markers.push(Marker { position: 1.0 + 2.0 * p, height: 0.0, n: 0 });
        markers.push(Marker { position: 1.0 + 4.0 * p, height: 0.0, n: 0 });
        markers.push(Marker { position: 3.0 + 2.0 * p, height: 0.0, n: 0 });
        markers.push(Marker { position: 5.0, height: 0.0, n: 0 });
        
        Self { p, n: 0, markers }
    }
    
    pub fn update(&mut self, value: f64) {
        self.n += 1;
        
        // First 5 observations go directly to markers
        if self.n <= 5 {
            self.markers[self.n - 1].height = value;
            if self.n == 5 {
                // Sort markers
                self.markers.sort_by(|a, b| a.height.partial_cmp(&b.height).unwrap());
            }
            return;
        }
        
        // Find which marker to update
        let mut k = 0;
        for i in 0..4 {
            if value < self.markers[i + 1].height {
                k = i;
                break;
            }
            if i == 3 {
                k = 4;
            }
        }
        
        // Update marker positions
        for i in k..5 {
            self.markers[i].n += 1;
        }
        
        // Update marker heights
        for i in 1..4 {
            let d = self.markers[i].position - self.markers[i].height;
            let inc = d / (self.markers[i + 1].n - self.markers[i - 1].n) as f64;
            self.markers[i].height += inc;
        }
        
        // Ensure monotonicity
        for i in 1..5 {
            if self.markers[i].height < self.markers[i - 1].height {
                self.markers[i].height = self.markers[i - 1].height;
            }
        }
    }
    
    pub fn value(&self) -> f64 {
        if self.n < 5 {
            return 0.0;
        }
        self.markers[2].height
    }
    
    pub fn reset(&mut self) {
        self.n = 0;
        for marker in &mut self.markers {
            marker.height = 0.0;
            marker.n = 0;
        }
    }
}

/// Correlation calculator (Pearson)
pub struct Correlation {
    n: u64,
    sum_x: f64,
    sum_y: f64,
    sum_xy: f64,
    sum_x2: f64,
    sum_y2: f64,
}

impl Correlation {
    pub fn new() -> Self {
        Self {
            n: 0,
            sum_x: 0.0,
            sum_y: 0.0,
            sum_xy: 0.0,
            sum_x2: 0.0,
            sum_y2: 0.0,
        }
    }
    
    pub fn update(&mut self, x: f64, y: f64) {
        self.n += 1;
        self.sum_x += x;
        self.sum_y += y;
        self.sum_xy += x * y;
        self.sum_x2 += x * x;
        self.sum_y2 += y * y;
    }
    
    pub fn update_batch(&mut self, x: &[f64], y: &[f64]) {
        for i in 0..x.len().min(y.len()) {
            self.update(x[i], y[i]);
        }
    }
    
    pub fn value(&self) -> f64 {
        if self.n == 0 {
            return 0.0;
        }
        
        let numerator = self.sum_xy - (self.sum_x * self.sum_y) / self.n as f64;
        let denom_x = self.sum_x2 - (self.sum_x * self.sum_x) / self.n as f64;
        let denom_y = self.sum_y2 - (self.sum_y * self.sum_y) / self.n as f64;
        
        numerator / (denom_x * denom_y).sqrt()
    }
    
    pub fn count(&self) -> u64 {
        self.n
    }
    
    pub fn reset(&mut self) {
        self.n = 0;
        self.sum_x = 0.0;
        self.sum_y = 0.0;
        self.sum_xy = 0.0;
        self.sum_x2 = 0.0;
        self.sum_y2 = 0.0;
    }
}

/// Histogram for distribution tracking
pub struct Histogram {
    bins: Vec<f64>,
    counts: Vec<u64>,
    min: f64,
    max: f64,
    total: u64,
}

impl Histogram {
    pub fn new(num_bins: usize) -> Self {
        Self {
            bins: vec![0.0; num_bins],
            counts: vec![0; num_bins],
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            total: 0,
        }
    }
    
    pub fn update(&mut self, value: f64) {
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
        
        self.total += 1;
        
        // Dynamic binning - update bin boundaries periodically
        if self.total % 100 == 0 {
            self.rebin();
        }
        
        let bin_idx = self.get_bin_index(value);
        self.counts[bin_idx] += 1;
    }
    
    fn rebin(&mut self) {
        let range = self.max - self.min;
        let bin_width = range / self.bins.len() as f64;
        
        for i in 0..self.bins.len() {
            self.bins[i] = self.min + (i as f64 + 0.5) * bin_width;
        }
    }
    
    fn get_bin_index(&self, value: f64) -> usize {
        if value <= self.min {
            return 0;
        }
        if value >= self.max {
            return self.bins.len() - 1;
        }
        
        let t = (value - self.min) / (self.max - self.min);
        ((t * self.bins.len() as f64) as usize).min(self.bins.len() - 1)
    }
    
    pub fn probability(&self, value: f64) -> f64 {
        let idx = self.get_bin_index(value);
        self.counts[idx] as f64 / self.total as f64
    }
    
    pub fn entropy(&self) -> f64 {
        let mut entropy = 0.0;
        for &count in &self.counts {
            let p = count as f64 / self.total as f64;
            if p > 0.0 {
                entropy -= p * p.ln();
            }
        }
        entropy
    }
    
    pub fn reset(&mut self) {
        self.counts.fill(0);
        self.total = 0;
        self.min = f64::INFINITY;
        self.max = f64::NEG_INFINITY;
    }
}

/// Z-score calculator
#[inline(always)]
pub fn z_score(value: f64, mean: f64, stddev: f64) -> f64 {
    if stddev == 0.0 {
        0.0
    } else {
        (value - mean) / stddev
    }
}

/// Standard normal CDF
#[inline(always)]
pub fn norm_cdf(z: f64) -> f64 {
    z.fast_norm_cdf()
}

/// Standard normal PDF
#[inline(always)]
pub fn norm_pdf(z: f64) -> f64 {
    (-z * z / 2.0).fast_exp() * (1.0 / (2.0 * std::f64::consts::PI).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_running_stats() {
        let mut stats = RunningStats::new();
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        
        for &v in &values {
            stats.update(v);
        }
        
        assert_eq!(stats.count(), 5);
        assert!((stats.mean() - 3.0).abs() < 1e-6);
        assert!((stats.variance() - 2.5).abs() < 1e-6);
        assert_eq!(stats.min(), 1.0);
        assert_eq!(stats.max(), 5.0);
    }
    
    #[test]
    fn test_percentile() {
        let mut p99 = Percentile::new(0.99);
        
        // Generate normal-like data
        for i in 0..1000 {
            p99.update(i as f64);
        }
        
        let value = p99.value();
        assert!(value > 980.0 && value < 1000.0);
    }
    
    #[test]
    fn test_correlation() {
        let mut corr = Correlation::new();
        
        // Perfect positive correlation
        for i in 0..100 {
            corr.update(i as f64, i as f64);
        }
        assert!((corr.value() - 1.0).abs() < 1e-6);
        
        corr.reset();
        
        // Perfect negative correlation
        for i in 0..100 {
            corr.update(i as f64, 100.0 - i as f64);
        }
        assert!((corr.value() + 1.0).abs() < 1e-6);
    }
}
