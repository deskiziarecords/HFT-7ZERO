// ============================================================
// CONFIGURATION MODULE
// ============================================================
// Centralized configuration management
// Hot-reload support for runtime updates
// Environment variable overrides
// Multi-environment support (dev/staging/prod)
// ============================================================

pub mod settings;
pub mod constants;
pub mod instruments;
pub mod dynamic_config;
pub mod validation;
pub mod secrets;

pub use settings::{SystemConfig, ConfigError, Environment, load_config};
pub use constants::*;
pub use instruments::{InstrumentConfig, Instrument, InstrumentManager};
pub use dynamic_config::{DynamicConfig, ConfigReloader, ConfigWatcher};
pub use validation::{ConfigValidator, ValidationResult, ValidationError};
pub use secrets::{SecretStore, SecretProvider, EncryptedSecret};

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use once_cell::sync::Lazy;

/// Global configuration instance
pub static GLOBAL_CONFIG: Lazy<Arc<RwLock<SystemConfig>>> = Lazy::new(|| {
    Arc::new(RwLock::new(SystemConfig::default()))
});

/// Configuration manager
pub struct ConfigManager {
    config: Arc<RwLock<SystemConfig>>,
    dynamic_config: Arc<DynamicConfig>,
    instrument_manager: Arc<InstrumentManager>,
    secret_store: Arc<SecretStore>,
    watchers: DashMap<String, Box<dyn Fn(&SystemConfig) + Send + Sync>>,
}

impl ConfigManager {
    /// Create new configuration manager
    pub fn new() -> Self {
        Self {
            config: GLOBAL_CONFIG.clone(),
            dynamic_config: Arc::new(DynamicConfig::new()),
            instrument_manager: Arc::new(InstrumentManager::new()),
            secret_store: Arc::new(SecretStore::new()),
            watchers: DashMap::new(),
        }
    }
    
    /// Initialize configuration from file
    pub fn init_from_file(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let config = SystemConfig::from_file(path)?;
        *self.config.write() = config;
        
        // Trigger watchers
        self.notify_watchers();
        
        Ok(())
    }
    
    /// Initialize from environment
    pub fn init_from_env(&self) -> Result<(), ConfigError> {
        let config = SystemConfig::from_env()?;
        *self.config.write() = config;
        
        self.notify_watchers();
        Ok(())
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> SystemConfig {
        self.config.read().clone()
    }
    
    /// Update configuration (with validation)
    pub fn update_config(&self, new_config: SystemConfig) -> Result<(), ConfigError> {
        // Validate new config
        new_config.validate()?;
        
        *self.config.write() = new_config;
        self.notify_watchers();
        
        Ok(())
    }
    
    /// Reload configuration from file
    pub fn reload(&self) -> Result<(), ConfigError> {
        let current = self.config.read();
        let path = current.config_path.clone();
        drop(current);
        
        self.init_from_file(&path)
    }
    
    /// Watch configuration changes
    pub fn watch<F>(&self, name: String, callback: F) 
    where 
        F: Fn(&SystemConfig) + Send + Sync + 'static
    {
        self.watchers.insert(name, Box::new(callback));
    }
    
    /// Notify all watchers of config change
    fn notify_watchers(&self) {
        let config = self.config.read();
        for entry in self.watchers.iter() {
            entry.value()(&config);
        }
    }
    
    /// Get instrument configuration
    pub fn get_instrument(&self, symbol: &str) -> Option<Instrument> {
        self.instrument_manager.get_instrument(symbol)
    }
    
    /// Get secret value
    pub fn get_secret(&self, key: &str) -> Option<String> {
        self.secret_store.get(key)
    }
    
    /// Get dynamic config value
    pub fn get_dynamic(&self, key: &str) -> Option<serde_json::Value> {
        self.dynamic_config.get(key)
    }
    
    /// Update dynamic config
    pub fn set_dynamic(&self, key: String, value: serde_json::Value) {
        self.dynamic_config.set(key, value);
    }
}

/// Configuration builder for fluent API
pub struct ConfigBuilder {
    config: SystemConfig,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: SystemConfig::default(),
        }
    }
    
    pub fn environment(mut self, env: Environment) -> Self {
        self.config.environment = env;
        self
    }
    
    pub fn latency_budget_ns(mut self, ns: u64) -> Self {
        self.config.latency_budget_ns = ns;
        self
    }
    
    pub fn max_position_lots(mut self, lots: f64) -> Self {
        self.config.max_position_lots = lots;
        self
    }
    
    pub fn add_instrument(mut self, instrument: Instrument) -> Self {
        self.config.instruments.push(instrument);
        self
    }
    
    pub fn build(self) -> SystemConfig {
        self.config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_manager() {
        let manager = ConfigManager::new();
        let config = manager.get_config();
        assert_eq!(config.environment, Environment::Development);
    }
}
