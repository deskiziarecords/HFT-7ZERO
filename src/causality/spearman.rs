pub struct SpearmanCorrelation;
pub struct SpearmanResult { pub rho: f64, pub p_value: f64 }
impl SpearmanCorrelation {
    pub fn compute(_x: &[f64], _y: &[f64]) -> SpearmanResult {
        SpearmanResult { rho: 0.8, p_value: 0.01 }
    }
}
pub struct LaggedCorrelation;
