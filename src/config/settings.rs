// ============================================================
// SYSTEM SETTINGS
// ============================================================
// Main configuration structure
// Environment-specific settings
// Serialization/deserialization
// ============================================================

use super::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// System environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Development
    }
}

/// Main system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    // General
    pub environment: Environment,
    pub config_path: PathBuf,
    pub log_level: String,
    pub log_format: String,
    
    // Performance
    pub latency_budget_ns: u64,
    pub max_concurrent_tasks: usize,
    pub thread_pool_size: usize,
    pub numa_node: Option<usize>,
    pub cpu_affinity: Option<Vec<usize>>,
    
    // Risk limits
    pub max_position_lots: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub var_confidence: f64,
    pub risk_threshold: f64,
    
    // Trading
    pub instruments: Vec<Instrument>,
    pub default_venue: String,
    pub stealth_enabled: bool,
    pub dry_run: bool,
    pub backtest_mode: bool,
    pub backtest_data_file: Option<PathBuf>,
    
    // Network
    pub market_data_ports: Vec<u16>,
    pub order_entry_ports: Vec<u16>,
    pub multicast_groups: Vec<String>,
    pub interface_name: String,
    
    // Monitoring
    pub metrics_port: u16,
    pub enable_profiling: bool,
    pub alert_cooldown_ms: u64,
    pub prometheus_enabled: bool,
    
    // Security
    pub tls_enabled: bool,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub api_key_encrypted: Option<String>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Development,
            config_path: PathBuf::from("config/development.toml"),
            log_level: "info".to_string(),
            log_format: "pretty".to_string(),
            latency_budget_ns: 1_000_000,
            max_concurrent_tasks: 4,
            thread_pool_size: 8,
            numa_node: None,
            cpu_affinity: None,
            max_position_lots: 1000.0,
            max_daily_loss: 10000.0,
            max_drawdown: 0.05,
            var_confidence: 0.99,
            risk_threshold: 0.7,
            instruments: vec![],
            default_venue: "SIM".to_string(),
            stealth_enabled: true,
            dry_run: false,
            backtest_mode: false,
            backtest_data_file: None,
            market_data_ports: vec![10000, 10001],
            order_entry_ports: vec![20000, 20001],
            multicast_groups: vec![],
            interface_name: "eth0".to_string(),
            metrics_port: 9090,
            enable_profiling: false,
            alert_cooldown_ms: 5000,
            prometheus_enabled: true,
            tls_enabled: false,
            cert_path: None,
            key_path: None,
            api_key_encrypted: None,
        }
    }
}

impl SystemConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileError(e.to_string()))?;
        
        let mut config: SystemConfig = toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        config.config_path = path.to_path_buf();
        
        // Apply environment overrides
        config.apply_env_overrides();
        
        // Validate
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();
        
        // Environment
        if let Ok(env) = std::env::var("HFT_ENVIRONMENT") {
            config.environment = match env.to_lowercase().as_str() {
                "production" => Environment::Production,
                "staging" => Environment::Staging,
                _ => Environment::Development,
            };
        }
        
        // Latency budget
        if let Ok(val) = std::env::var("HFT_LATENCY_BUDGET_US") {
            if let Ok(us) = val.parse::<u64>() {
                config.latency_budget_ns = us * 1000;
            }
        }
        
        // Max position
        if let Ok(val) = std::env::var("HFT_MAX_POSITION_LOTS") {
            if let Ok(lots) = val.parse::<f64>() {
                config.max_position_lots = lots;
            }
        }
        
        // Stealth enabled
        if let Ok(val) = std::env::var("HFT_STEALTH_ENABLED") {
            config.stealth_enabled = val.to_lowercase() == "true";
        }
        
        // Dry run
        if let Ok(val) = std::env::var("HFT_DRY_RUN") {
            config.dry_run = val.to_lowercase() == "true";
        }
        
        // Interface
        if let Ok(val) = std::env::var("HFT_INTERFACE") {
            config.interface_name = val;
        }
        
        config.validate()?;
        Ok(config)
    }
    
    /// Apply environment variable overrides to existing config
    fn apply_env_overrides(&mut self) {
        // Override critical settings from env
        if let Ok(val) = std::env::var("HFT_LATENCY_BUDGET_US") {
            if let Ok(us) = val.parse::<u64>() {
                self.latency_budget_ns = us * 1000;
            }
        }
        
        if let Ok(val) = std::env::var("HFT_MAX_POSITION_LOTS") {
            if let Ok(lots) = val.parse::<f64>() {
                self.max_position_lots = lots;
            }
        }
        
        if let Ok(val) = std::env::var("HFT_DRY_RUN") {
            self.dry_run = val.to_lowercase() == "true";
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate latency budget
        if self.latency_budget_ns == 0 {
            return Err(ConfigError::InvalidValue("latency_budget_ns must be > 0".to_string()));
        }
        
        if self.latency_budget_ns > 10_000_000 {
            return Err(ConfigError::InvalidValue(
                "latency_budget_ns exceeds 10ms - too high for HFT".to_string()
            ));
        }
        
        // Validate position limits
        if self.max_position_lots <= 0.0 {
            return Err(ConfigError::InvalidValue("max_position_lots must be > 0".to_string()));
        }
        
        if self.max_daily_loss <= 0.0 {
            return Err(ConfigError::InvalidValue("max_daily_loss must be > 0".to_string()));
        }
        
        // Validate drawdown
        if !(0.0..1.0).contains(&self.max_drawdown) {
            return Err(ConfigError::InvalidValue("max_drawdown must be between 0 and 1".to_string()));
        }
        
        // Validate VaR confidence
        if !(0.9..1.0).contains(&self.var_confidence) {
            return Err(ConfigError::InvalidValue("var_confidence must be between 0.9 and 1.0".to_string()));
        }
        
        // Validate instruments
        if !self.backtest_mode && self.instruments.is_empty() {
            return Err(ConfigError::InvalidValue("at least one instrument required".to_string()));
        }
        
        // Validate network ports
        for &port in &self.market_data_ports {
            if port == 0 || port > 65535 {
                return Err(ConfigError::InvalidValue(format!("invalid port: {}", port)));
            }
        }
        
        Ok(())
    }
    
    /// Get config for specific environment
    pub fn for_environment(env: Environment) -> Self {
        let mut config = Self::default();
        config.environment = env;
        
        match env {
            Environment::Development => {
                config.log_level = "debug".to_string();
                config.stealth_enabled = false;
                config.dry_run = true;
            }
            Environment::Staging => {
                config.log_level = "info".to_string();
                config.stealth_enabled = true;
                config.dry_run = true;
                config.max_position_lots = 10.0;
            }
            Environment::Production => {
                config.log_level = "warn".to_string();
                config.stealth_enabled = true;
                config.dry_run = false;
                config.max_position_lots = 1000.0;
                config.enable_profiling = false;
            }
        }
        
        config
    }
    
    /// Export to TOML string
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializeError(e.to_string()))
    }
    
    /// Save to file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let toml_str = self.to_toml()?;
        std::fs::write(path, toml_str)
            .map_err(|e| ConfigError::FileError(e.to_string()))
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File error: {0}")]
    FileError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    
    #[error("Validation failed: {0}")]
    ValidationError(String),
    
    #[error("Serialize error: {0}")]
    SerializeError(String),
    
    #[error("Instrument not found: {0}")]
    InstrumentNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_config_default() {
        let config = SystemConfig::default();
        assert_eq!(config.latency_budget_ns, 1_000_000);
        assert!(config.stealth_enabled);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = SystemConfig::default();
        assert!(config.validate().is_ok());
        
        config.latency_budget_ns = 0;
        assert!(config.validate().is_err());
        
        config.latency_budget_ns = 1_000_000;
        config.max_drawdown = 1.5;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_from_env() {
        std::env::set_var("HFT_LATENCY_BUDGET_US", "500");
        std::env::set_var("HFT_STEALTH_ENABLED", "false");
        
        let config = SystemConfig::from_env().unwrap();
        assert_eq!(config.latency_budget_ns, 500_000);
        assert!(!config.stealth_enabled);
        
        std::env::remove_var("HFT_LATENCY_BUDGET_US");
        std::env::remove_var("HFT_STEALTH_ENABLED");
    }
    
    #[test]
    fn test_config_file_roundtrip() {
        let config = SystemConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        
        config.save_to_file(temp_file.path()).unwrap();
        let loaded = SystemConfig::from_file(temp_file.path()).unwrap();
        
        assert_eq!(config.latency_budget_ns, loaded.latency_budget_ns);
        assert_eq!(config.stealth_enabled, loaded.stealth_enabled);
    }
}
