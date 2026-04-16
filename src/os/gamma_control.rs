use crate::market::OrderBook;

pub struct GammaController {
    pub eta: f64,
    pub kappa: f64,
    pub target: f64,
}

impl GammaController {
    pub fn new(eta: f64, kappa: f64, target: f64) -> Self {
        Self { eta, kappa, target }
    }
    pub fn calculate_feedback(&self, book: &OrderBook) -> f64 {
        book.spread() * 0.1
    }
    pub fn update(&self, current: f64, feedback: f64) -> f64 {
        current + self.eta * feedback
    }
}
pub struct GammaConfig;
pub struct HedgeSignal;
