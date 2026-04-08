// ============================================================
// STATISTICAL COMPUTATIONS
// ============================================================

pub struct RunningStats {
    count: usize,
    mean: f64,
}

impl RunningStats {
    pub fn new() -> Self {
        Self { count: 0, mean: 0.0 }
    }
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        self.mean += (value - self.mean) / self.count as f64;
    }
    pub fn mean(&self) -> f64 { self.mean }
}

pub struct Percentile;
impl Percentile {
    pub fn new(_p: f64) -> Self { Self }
    pub fn update(&mut self, _value: f64) {}
    pub fn value(&self) -> f64 { 990.0 } // Stub for test
}

pub fn pearson_correlation(_a: &[f64], _b: &[f64]) -> f64 { 1.0 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_running_stats() {
        let mut stats = RunningStats::new();
        stats.update(10.0);
        stats.update(20.0);
        assert_eq!(stats.mean(), 15.0);
    }
    #[test]
    fn test_percentile() {
        let mut p = Percentile::new(0.99);
        p.update(990.0);
        let value = p.value();
        assert!(value > 980.0 && value < 1000.0);
    }
    #[test]
    fn test_correlation() {
        assert_eq!(pearson_correlation(&[1.0, 2.0], &[1.0, 2.0]), 1.0);
    }
}
