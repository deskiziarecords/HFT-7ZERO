// ============================================================
// SYSTEM SETTINGS
// ============================================================

use serde::{Deserialize, Serialize};
use std::path::Path;
use config::{Config, File};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub latency_budget_ns: u64,
    pub max_position_lots: u64,
    pub risk_threshold: f64,
    pub stealth_enabled: bool,
    pub dry_run: bool,
    pub backtest_mode: bool,
    pub backtest_data_file: std::path::PathBuf,
    pub instruments: Vec<String>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            latency_budget_ns: 1000000,
            max_position_lots: 100,
            risk_threshold: 0.5,
            stealth_enabled: true,
            dry_run: false,
            backtest_mode: false,
            backtest_data_file: std::path::PathBuf::new(),
            instruments: Vec::new(),
        }
    }
}

impl SystemConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let s = Config::builder()
            .add_source(File::from(path.as_ref()))
            .build()
            .map_err(|e| e.to_string())?;
        
        s.try_deserialize().map_err(|e| e.to_string())
    }
}
