// ============================================================
// VALUE AT RISK (VaR) CALCULATIONS
// ============================================================
// Historical VaR
// Parametric VaR
// Expected Shortfall
// Real-time risk metrics
// ============================================================

use super::*;
use crate::market::OrderBook;
use dashmap::DashMap;
use std::collections::VecDeque;

/// Value at Risk trait
pub trait ValueAtRisk: Send + Sync {
    fn calculate(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String>;
    fn expected_shortfall(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String>;
    fn update(&mut self, returns: &[f64]);
}

/// Historical VaR calculator
pub struct HistoricalVaR {
    confidence: f64,
    horizon_seconds: u64,
    returns_history: VecDeque<f64>,
    max_history: usize,
}

impl HistoricalVaR {
    pub fn new(confidence: f64, horizon_seconds: u64) -> Self {
        Self {
            confidence,
            horizon_seconds,
            returns_history: VecDeque::with_capacity(10000),
            max_history: 10000,
        }
    }
    
    fn compute_portfolio_returns(&self, positions: &DashMap<u32, Position>, book: &OrderBook) -> f64 {
        let mut total_return = 0.0;
        
        for entry in positions.iter() {
            let position = entry.value();
            // Simplified: would need instrument-specific pricing
            let position_value = position.quantity * book.mid_price();
            let return_contrib = position_value * 0.0001; // Placeholder
            total_return += return_contrib;
        }
        
        total_return
    }
}

impl ValueAtRisk for HistoricalVaR {
    fn calculate(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        if self.returns_history.is_empty() {
            return Ok(0.01); // Default 1% VaR
        }
        
        let mut sorted_returns: Vec<f64> = self.returns_history.iter().copied().collect();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = ((1.0 - confidence) * sorted_returns.len() as f64) as usize;
        let historical_var = -sorted_returns[index];
        
        // Scale by current portfolio value
        let portfolio_value = self.compute_portfolio_returns(positions, book);
        Ok(historical_var * portfolio_value.abs())
    }
    
    fn expected_shortfall(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        if self.returns_history.is_empty() {
            return Ok(0.02);
        }
        
        let mut sorted_returns: Vec<f64> = self.returns_history.iter().copied().collect();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let tail_size = ((1.0 - confidence) * sorted_returns.len() as f64) as usize;
        let tail_returns = &sorted_returns[..tail_size];
        let es = -tail_returns.iter().sum::<f64>() / tail_size as f64;
        
        let portfolio_value = self.compute_portfolio_returns(positions, book);
        Ok(es * portfolio_value.abs())
    }
    
    fn update(&mut self, returns: &[f64]) {
        for &ret in returns {
            self.returns_history.push_back(ret);
            while self.returns_history.len() > self.max_history {
                self.returns_history.pop_front();
            }
        }
    }
}

/// Parametric VaR (assuming normal distribution)
pub struct ParametricVaR {
    confidence: f64,
    horizon_seconds: u64,
    mean_return: f64,
    std_return: f64,
    return_history: VecDeque<f64>,
}

impl ParametricVaR {
    pub fn new(confidence: f64, horizon_seconds: u64) -> Self {
        Self {
            confidence,
            horizon_seconds,
            mean_return: 0.0,
            std_return: 0.01,
            return_history: VecDeque::with_capacity(1000),
        }
    }
    
    fn update_statistics(&mut self) {
        if self.return_history.is_empty() {
            return;
        }
        
        let n = self.return_history.len() as f64;
        self.mean_return = self.return_history.iter().sum::<f64>() / n;
        
        let variance = self.return_history.iter()
            .map(|r| (r - self.mean_return).powi(2))
            .sum::<f64>() / n;
        self.std_return = variance.sqrt();
    }
    
    fn z_score(&self, confidence: f64) -> f64 {
        // Approximate inverse CDF for normal distribution
        // In production, use proper implementation
        match confidence {
            c if c >= 0.99 => 2.326,
            c if c >= 0.95 => 1.645,
            c if c >= 0.90 => 1.282,
            _ => 1.0,
        }
    }
}

impl ValueAtRisk for ParametricVaR {
    fn calculate(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        let z = self.z_score(confidence);
        let portfolio_value = 1_000_000.0; // Placeholder
        
        let var = portfolio_value * (self.mean_return - z * self.std_return);
        Ok(var.abs())
    }
    
    fn expected_shortfall(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        let z = self.z_score(confidence);
        let pdf = (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let es_factor = pdf / (1.0 - confidence);
        
        let portfolio_value = 1_000_000.0;
        Ok(portfolio_value * es_factor * self.std_return)
    }
    
    fn update(&mut self, returns: &[f64]) {
        for &ret in returns {
            self.return_history.push_back(ret);
            while self.return_history.len() > 1000 {
                self.return_history.pop_front();
            }
        }
        // Would need to update statistics (mut self)
    }
}

/// Monte Carlo VaR (for complex portfolios)
pub struct MonteCarloVaR {
    confidence: f64,
    horizon_seconds: u64,
    num_simulations: usize,
    rng: fastrand::Rng,
}

impl MonteCarloVaR {
    pub fn new(confidence: f64, horizon_seconds: u64, num_simulations: usize) -> Self {
        Self {
            confidence,
            horizon_seconds,
            num_simulations,
            rng: fastrand::Rng::new(),
        }
    }
}

impl ValueAtRisk for MonteCarloVaR {
    fn calculate(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        let mut simulated_pnl = Vec::with_capacity(self.num_simulations);
        
        for _ in 0..self.num_simulations {
            // Simulate price paths using geometric Brownian motion
            let simulated_return = self.rng.f64() * 0.02 - 0.01;
            simulated_pnl.push(simulated_return);
        }
        
        simulated_pnl.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = ((1.0 - confidence) * self.num_simulations as f64) as usize;
        
        Ok(-simulated_pnl[index])
    }
    
    fn expected_shortfall(&self, positions: &DashMap<u32, Position>, book: &OrderBook, confidence: f64) -> Result<f64, String> {
        let mut simulated_pnl = Vec::with_capacity(self.num_simulations);
        
        for _ in 0..self.num_simulations {
            let simulated_return = self.rng.f64() * 0.02 - 0.01;
            simulated_pnl.push(simulated_return);
        }
        
        simulated_pnl.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let tail_size = ((1.0 - confidence) * self.num_simulations as f64) as usize;
        let tail = &simulated_pnl[..tail_size];
        
        Ok(-tail.iter().sum::<f64>() / tail_size as f64)
    }
    
    fn update(&mut self, returns: &[f64]) {
        // Would update volatility estimates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_historical_var() {
        let mut var = HistoricalVaR::new(0.99, 1);
        
        // Add some historical returns
        let returns: Vec<f64> = vec![-0.02, -0.01, 0.0, 0.01, 0.02, -0.03, 0.03];
        var.update(&returns);
        
        let positions = DashMap::new();
        // Would need order book for full test
    }
}
