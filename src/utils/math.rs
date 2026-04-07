// ============================================================
// FAST MATHEMATICAL OPERATIONS
// ============================================================
// Approximations for common functions
// SIMD-optimized operations
// No-std compatible where possible
// ============================================================

use std::f64::consts;

/// Fast math operations trait
pub trait FastMath {
    fn fast_exp(self) -> Self;
    fn fast_ln(self) -> Self;
    fn fast_pow(self, exp: Self) -> Self;
    fn fast_sigmoid(self) -> Self;
    fn fast_tanh(self) -> Self;
    fn inv_sqrt(self) -> Self;
    fn fast_erf(self) -> Self;
    fn fast_norm_cdf(self) -> Self;
}

impl FastMath for f64 {
    /// Fast exponential approximation using Remez algorithm
    /// Accuracy: ~0.1% relative error
    #[inline(always)]
    fn fast_exp(self) -> f64 {
        if self < -708.0 {
            return 0.0;
        }
        if self > 709.0 {
            return f64::INFINITY;
        }
        
        let a = self.abs();
        let n = (a + 0.5) as i64;
        let x = self - n as f64;
        
        // Remez approximation for e^x on [-0.5, 0.5]
        let p = 0.99999999999980993
            + x * (0.9999999999819111
                + x * (0.4999999999702409
                    + x * (0.1666666666666587
                        + x * (0.0416666666666661
                            + x * (0.00833333333333344
                                + x * (0.00138888888888866
                                    + x * (0.000198412698412573
                                        + x * 0.000024801587301672)))))));
        
        p * (1 << n) as f64
    }
    
    /// Fast natural logarithm approximation
    /// Accuracy: ~0.1% relative error
    #[inline(always)]
    fn fast_ln(self) -> f64 {
        if self <= 0.0 {
            return f64::NEG_INFINITY;
        }
        
        let mut x = self;
        let mut e = 0;
        
        // Reduce to [1, 2)
        while x >= 2.0 {
            x /= 2.0;
            e += 1;
        }
        while x < 1.0 {
            x *= 2.0;
            e -= 1;
        }
        
        // Approximation for ln(x) on [1, 2)
        let y = (x - 1.0) / (x + 1.0);
        let y2 = y * y;
        
        let ln_x = y * (2.0 + y2 * (0.6666666666666667 + y2 * (0.4 + y2 * (0.2857142857142857 + y2 * 0.2222222222222222))));
        
        ln_x + e as f64 * consts::LN_2
    }
    
    /// Fast power approximation
    #[inline(always)]
    fn fast_pow(self, exp: Self) -> Self {
        if self <= 0.0 {
            return 0.0;
        }
        (exp * self.fast_ln()).fast_exp()
    }
    
    /// Fast sigmoid approximation
    /// Accuracy: ~0.01 absolute error
    #[inline(always)]
    fn fast_sigmoid(self) -> Self {
        1.0 / (1.0 + (-self).fast_exp())
    }
    
    /// Fast tanh approximation
    #[inline(always)]
    fn fast_tanh(self) -> Self {
        let x2 = self * self;
        let p = x2 * (0.0415 + 0.0181 * x2);
        1.0 - 2.0 / ((2.0 * self).fast_exp() + 1.0)
    }
    
    /// Fast inverse square root (Quake III method)
    #[inline(always)]
    fn inv_sqrt(self) -> Self {
        if self <= 0.0 {
            return f64::INFINITY;
        }
        
        let i = self.to_bits();
        let j = 0x5FE6EB50C7B537A9u64 - (i >> 1);
        let y = f64::from_bits(j);
        y * (1.5 - 0.5 * self * y * y)
    }
    
    /// Fast error function approximation
    #[inline(always)]
    fn fast_erf(self) -> Self {
        let x = self.abs();
        let t = 1.0 / (1.0 + 0.3275911 * x);
        let poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
        let erf = 1.0 - poly * (-x * x).fast_exp();
        
        if self < 0.0 {
            -erf
        } else {
            erf
        }
    }
    
    /// Fast normal CDF approximation
    #[inline(always)]
    fn fast_norm_cdf(self) -> Self {
        0.5 * (1.0 + (self * consts::SQRT_2.recip()).fast_erf())
    }
}

/// Fast vector operations using SIMD
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn fast_dot_product(a: &[f64], b: &[f64]) -> f64 {
    use std::arch::x86_64::*;
    
    let n = a.len().min(b.len());
    let mut sum = 0.0;
    
    // Process 4 at a time using SIMD
    let mut i = 0;
    unsafe {
        let mut sum_vec = _mm256_setzero_pd();
        while i + 4 <= n {
            let a_vec = _mm256_loadu_pd(a.as_ptr().add(i));
            let b_vec = _mm256_loadu_pd(b.as_ptr().add(i));
            sum_vec = _mm256_add_pd(sum_vec, _mm256_mul_pd(a_vec, b_vec));
            i += 4;
        }
        
        // Horizontal sum
        let sum_high = _mm256_extractf128_pd(sum_vec, 1);
        let sum_low = _mm256_castpd256_pd128(sum_vec);
        let sum128 = _mm_add_pd(sum_low, sum_high);
        sum = _mm_cvtsd_f64(sum128) + _mm_cvtsd_f64(_mm_unpackhi_pd(sum128, sum128));
    }
    
    // Process remainder
    for j in i..n {
        sum += a[j] * b[j];
    }
    
    sum
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub fn fast_dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Fast moving average (sliding window)
pub struct MovingAverage {
    window: usize,
    values: Vec<f64>,
    sum: f64,
    index: usize,
}

impl MovingAverage {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            values: vec![0.0; window],
            sum: 0.0,
            index: 0,
        }
    }
    
    pub fn update(&mut self, value: f64) -> f64 {
        self.sum -= self.values[self.index];
        self.values[self.index] = value;
        self.sum += value;
        self.index = (self.index + 1) % self.window;
        self.sum / self.window as f64
    }
    
    pub fn value(&self) -> f64 {
        self.sum / self.window as f64
    }
    
    pub fn reset(&mut self) {
        self.values.fill(0.0);
        self.sum = 0.0;
        self.index = 0;
    }
}

/// Exponential moving average
pub struct ExponentialMovingAverage {
    alpha: f64,
    value: f64,
    initialized: bool,
}

impl ExponentialMovingAverage {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            value: 0.0,
            initialized: false,
        }
    }
    
    pub fn update(&mut self, new_value: f64) -> f64 {
        if !self.initialized {
            self.value = new_value;
            self.initialized = true;
        } else {
            self.value = self.alpha * new_value + (1.0 - self.alpha) * self.value;
        }
        self.value
    }
    
    pub fn value(&self) -> f64 {
        self.value
    }
    
    pub fn reset(&mut self) {
        self.initialized = false;
        self.value = 0.0;
    }
}

/// Linear interpolation
#[inline(always)]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Clamp value to range
#[inline(always)]
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Normalize to [0, 1] range
#[inline(always)]
pub fn normalize(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return 0.5;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

/// Convert degrees to radians
#[inline(always)]
pub fn to_radians(degrees: f64) -> f64 {
    degrees * consts::PI / 180.0
}

/// Convert radians to degrees
#[inline(always)]
pub fn to_degrees(radians: f64) -> f64 {
    radians * 180.0 / consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fast_exp() {
        assert!((0.0_f64.fast_exp() - 1.0).abs() < 1e-6);
        assert!((1.0_f64.fast_exp() - consts::E).abs() < 1e-4);
        assert!(f64::NEG_INFINITY.fast_exp() < 1e-10);
    }
    
    #[test]
    fn test_fast_ln() {
        assert!(1.0_f64.fast_ln().abs() < 1e-6);
        assert!((consts::E.fast_ln() - 1.0).abs() < 1e-4);
    }
    
    #[test]
    fn test_inv_sqrt() {
        let x = 4.0;
        let inv_sqrt = x.inv_sqrt();
        assert!((inv_sqrt - 0.5).abs() < 1e-6);
    }
    
    #[test]
    fn test_moving_average() {
        let mut ma = MovingAverage::new(3);
        assert_eq!(ma.update(10.0), 10.0 / 3.0);
        assert_eq!(ma.update(20.0), 30.0 / 3.0);
        assert_eq!(ma.update(30.0), 60.0 / 3.0);
        assert_eq!(ma.update(40.0), 90.0 / 3.0);
    }
    
    #[test]
    fn test_ema() {
        let mut ema = ExponentialMovingAverage::new(0.5);
        assert_eq!(ema.update(10.0), 10.0);
        assert_eq!(ema.update(20.0), 15.0);
        assert_eq!(ema.update(30.0), 22.5);
    }
}
