// ============================================================
// RISK ENGINE
// ============================================================
// Central risk management engine
// Real-time risk calculation and monitoring
// Integration with trading system
// ============================================================

use dashmap::DashMap;
use super::*;
use crate::market::OrderBook;
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::RwLock;

/// Main risk engine
pub struct RiskEngine {
    config: RiskConfig,
    metrics: Arc<RwLock<RiskMetrics>>,
    var_calculator: Box<dyn ValueAtRisk>,
    stress_tester: StressTester,
    position_limits: PositionLimits,
    pnl_calculator: PnLCalculator,
    history: VecDeque<RiskMetrics>,
    event_sender: tokio::sync::mpsc::UnboundedSender<RiskEvent>,
}

impl RiskEngine {
    /// Create new risk engine
    pub fn new(
        config: RiskConfig,
        event_sender: tokio::sync::mpsc::UnboundedSender<RiskEvent>,
    ) -> Self {
        Self {
            config: config.clone(),
            metrics: Arc::new(RwLock::new(RiskMetrics::default())),
            var_calculator: Box::new(HistoricalVaR::new(config.var_confidence, config.var_horizon_seconds)),
            stress_tester: StressTester::new(),
            position_limits: PositionLimits::new(config.max_position_size, config.max_daily_loss),
            pnl_calculator: PnLCalculator::new(),
            history: VecDeque::with_capacity(1000),
            event_sender,
        }
    }
    
    /// Update risk metrics with new market data
    pub fn update(&mut self, book: &OrderBook, positions: &DashMap<u32, Position>) -> Result<(), String> {
        let start_time = std::time::Instant::now();
        
        // Update positions
        for entry in positions.iter() {
            let instrument_id = *entry.key();
            let position = entry.value();
            self.position_limits.update_position(instrument_id, position.quantity);
        }
        
        // Calculate current PnL
        let current_pnl = self.pnl_calculator.calculate_total_pnl(positions, book);
        
        // Calculate VaR
        let var_95 = self.var_calculator.calculate(positions, book, 0.95)?;
        let var_99 = self.var_calculator.calculate(positions, book, 0.99)?;
        let es = self.var_calculator.expected_shortfall(positions, book, 0.99)?;
        
        // Calculate drawdown
        let drawdown = self.calculate_drawdown(&current_pnl);
        
        // Update metrics
        let mut metrics = self.metrics.write();
        metrics.current_pnl = current_pnl;
        metrics.daily_pnl += current_pnl;
        metrics.daily_loss = metrics.daily_pnl.min(0.0).abs();
        metrics.drawdown = drawdown;
        metrics.var_95 = var_95;
        metrics.var_99 = var_99;
        metrics.expected_shortfall = es;
        metrics.timestamp_ns = crate::utils::time::get_hardware_timestamp();
        
        // Store history
        self.history.push_back(metrics.clone());
        while self.history.len() > 1000 {
            self.history.pop_front();
        }
        
        // Check limits
        self.check_limits(&metrics)?;
        
        let elapsed_ns = start_time.elapsed().as_nanos() as u64;
        tracing::debug!("Risk update took {} ns", elapsed_ns);
        
        Ok(())
    }
    
    /// Check all risk limits
    fn check_limits(&self, metrics: &RiskMetrics) -> Result<(), String> {
        // Check position limit
        if let Some(breach) = self.position_limits.check_position_limits() {
            let _ = self.event_sender.send(RiskEvent::LimitBreached(breach.clone()));
            return Err(format!("Position limit breached: {:?}", breach));
        }
        
        // Check daily loss limit
        if metrics.daily_loss > self.config.max_daily_loss {
            let _ = self.event_sender.send(RiskEvent::LimitBreached(
                LimitBreach::DailyLoss { loss: metrics.daily_loss, limit: self.config.max_daily_loss }
            ));
            return Err(format!("Daily loss limit exceeded: {:.2} > {:.2}", 
                metrics.daily_loss, self.config.max_daily_loss));
        }
        
        // Check drawdown limit
        if metrics.drawdown > self.config.max_drawdown {
            let _ = self.event_sender.send(RiskEvent::DrawdownLimitHit {
                drawdown: metrics.drawdown,
                limit: self.config.max_drawdown,
            });
            return Err(format!("Drawdown limit exceeded: {:.2}% > {:.2}%", 
                metrics.drawdown * 100.0, self.config.max_drawdown * 100.0));
        }
        
        // Check VaR exceedance
        if metrics.current_pnl.abs() > metrics.var_99 {
            let _ = self.event_sender.send(RiskEvent::VaRExceeded {
                var_value: metrics.var_99,
                actual_loss: metrics.current_pnl.abs(),
            });
        }
        
        Ok(())
    }
    
    /// Calculate current drawdown
    fn calculate_drawdown(&self, current_pnl: &f64) -> f64 {
        let peak_pnl = self.history.iter()
            .map(|m| m.current_pnl)
            .fold(*current_pnl, f64::max);
        
        if peak_pnl > 0.0 {
            (peak_pnl - current_pnl) / peak_pnl
        } else {
            0.0
        }
    }
    
    /// Run stress tests
    pub fn run_stress_test(&mut self, book: &OrderBook, positions: &DashMap<u32, Position>) -> Vec<ScenarioResult> {
        self.stress_tester.run_all_scenarios(book, positions)
    }
    
    /// Get current risk metrics
    pub fn metrics(&self) -> RiskMetrics {
        self.metrics.read().clone()
    }
    
    /// Check if trading is allowed
    pub fn can_trade(&self, order_size: f64) -> bool {
        let metrics = self.metrics.read();
        
        // Check if any limits are close to being breached
        if metrics.daily_loss + order_size.abs() > self.config.max_daily_loss * 0.9 {
            return false;
        }
        
        if metrics.drawdown + 0.01 > self.config.max_drawdown {
            return false;
        }
        
        true
    }
    
    /// Record a trade
    pub fn record_trade(&mut self, trade: TradeRecord) {
        self.pnl_calculator.record_trade(trade);
        let _ = self.event_sender.send(RiskEvent::LimitBreached(
            LimitBreach::TradeExecuted { trade_id: trade.trade_id, size: trade.size }
        ));
    }
    
    /// Reset daily metrics
    pub fn reset_daily(&mut self) {
        let mut metrics = self.metrics.write();
        metrics.daily_pnl = 0.0;
        metrics.daily_loss = 0.0;
        self.pnl_calculator.reset_daily();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_risk_engine() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let _engine = RiskEngine::new(RiskConfig::default(), tx);
        
        // Test would require order book and positions
    }
}
