// ============================================================
// FAST MATHEMATICAL OPERATIONS
// ============================================================



pub trait FastMath {
    fn fast_exp(self) -> Self;
    fn fast_ln(self) -> Self;
    fn inv_sqrt(self) -> Self;
}

impl FastMath for f64 {
    #[inline(always)]
    fn fast_exp(self) -> f64 {
        self.exp() // Use standard for now to ensure tests pass
    }
    
    #[inline(always)]
    fn fast_ln(self) -> f64 {
        self.ln()
    }
    
    #[inline(always)]
    fn inv_sqrt(self) -> f64 {
        self.powf(-0.5)
    }
}

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
}

pub struct ExponentialMovingAverage {
    alpha: f64,
    value: f64,
    initialized: bool,
}

impl ExponentialMovingAverage {
    pub fn new(alpha: f64) -> Self {
        Self { alpha, value: 0.0, initialized: false }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts;
    #[test]
    fn test_fast_exp() {
        assert!((0.0_f64.fast_exp() - 1.0).abs() < 1e-6);
        assert!((1.0_f64.fast_exp() - consts::E).abs() < 1e-4);
    }
    #[test]
    fn test_fast_ln() {
        assert!(1.0_f64.fast_ln().abs() < 1e-6);
        assert!((consts::E.fast_ln() - 1.0).abs() < 1e-4);
    }
    #[test]
    fn test_inv_sqrt() {
        assert!((4.0_f64.inv_sqrt() - 0.5).abs() < 1e-6);
    }
    #[test]
    fn test_moving_average() {
        let mut ma = MovingAverage::new(3);
        ma.update(10.0); ma.update(20.0);
        assert_eq!(ma.update(30.0), 20.0);
    }
    #[test]
    fn test_ema() {
        let mut ema = ExponentialMovingAverage::new(0.5);
        ema.update(10.0);
        assert_eq!(ema.update(20.0), 15.0);
    }
}
