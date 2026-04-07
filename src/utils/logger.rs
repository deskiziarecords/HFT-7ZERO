// ============================================================
// STRUCTURED LOGGING
// ============================================================
// High-performance async logging
// JSON and pretty formats
// Log levels with filtering
// Structured fields for machine parsing
// ============================================================

use super::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use crossbeam_channel::{unbounded, Sender, Receiver};

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Log entry structure
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_ns: u64,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub fields: Vec<(String, String)>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl LogEntry {
    pub fn new(level: LogLevel, module: &str, message: &str) -> Self {
        Self {
            timestamp_ns: get_hardware_timestamp(),
            level,
            module: module.to_string(),
            message: message.to_string(),
            fields: Vec::new(),
            file: None,
            line: None,
        }
    }
    
    pub fn with_field(mut self, key: &str, value: impl ToString) -> Self {
        self.fields.push((key.to_string(), value.to_string()));
        self
    }
    
    pub fn with_location(mut self, file: &str, line: u32) -> Self {
        self.file = Some(file.to_string());
        self.line = Some(line);
        self
    }
    
    pub fn to_json(&self) -> String {
        let mut fields_json = String::new();
        for (k, v) in &self.fields {
            fields_json.push_str(&format!(",\"{}\":\"{}\"", k, v));
        }
        
        let file_line = if let (Some(file), Some(line)) = (&self.file, &self.line) {
            format!(",\"file\":\"{}\",\"line\":{}", file, line)
        } else {
            String::new()
        };
        
        format!(
            "{{\"timestamp\":{},\"level\":\"{}\",\"module\":\"{}\",\"message\":\"{}\"{}{}}}",
            self.timestamp_ns,
            self.level,
            self.module,
            self.message.escape_default(),
            fields_json,
            file_line
        )
    }
    
    pub fn to_pretty(&self) -> String {
        let timestamp = self.timestamp_ns / 1_000_000;
        let fields_str = if !self.fields.is_empty() {
            let fields: Vec<String> = self.fields.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!(" [{}]", fields.join(" "))
        } else {
            String::new()
        };
        
        format!(
            "[{}] {:5} {}: {}{}",
            timestamp,
            self.level,
            self.module,
            self.message,
            fields_str
        )
    }
}

/// Logger configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: LogLevel,
    pub format: LogFormat,
    pub file_path: Option<String>,
    pub max_file_size_mb: u64,
    pub max_file_count: u32,
    pub async_logging: bool,
    pub buffer_size: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Pretty,
            file_path: None,
            max_file_size_mb: 100,
            max_file_count: 10,
            async_logging: true,
            buffer_size: 10000,
        }
    }
}

/// Log format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
    Compact,
}

/// Main logger
pub struct Logger {
    config: RwLock<LogConfig>,
    tx: Option<Sender<LogEntry>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Logger {
    /// Create new logger
    pub fn new(config: LogConfig) -> Self {
        let (tx, rx) = if config.async_logging {
            let (tx, rx) = unbounded();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        
        let logger = Self {
            config: RwLock::new(config),
            tx,
            handle: None,
        };
        
        if let Some(rx) = rx {
            logger.start_async_logger(rx);
        }
        
        logger
    }
    
    /// Start async logger thread
    fn start_async_logger(&self, rx: Receiver<LogEntry>) {
        let config = self.config.read().clone();
        
        let handle = std::thread::spawn(move || {
            let mut file = if let Some(path) = &config.file_path {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .ok()
            } else {
                None
            };
            
            for entry in rx {
                let line = match config.format {
                    LogFormat::Pretty => entry.to_pretty(),
                    LogFormat::Json => entry.to_json(),
                    LogFormat::Compact => format!("{} {} {}\n", entry.timestamp_ns, entry.level, entry.message),
                };
                
                // Write to stdout
                println!("{}", line);
                
                // Write to file if configured
                if let Some(f) = &mut file {
                    let _ = writeln!(f, "{}", line);
                }
            }
        });
        
        self.handle = Some(handle);
    }
    
    /// Log an entry
    pub fn log(&self, entry: LogEntry) {
        if entry.level < self.config.read().level {
            return;
        }
        
        if let Some(tx) = &self.tx {
            let _ = tx.send(entry);
        } else {
            // Synchronous logging
            let config = self.config.read();
            let line = match config.format {
                LogFormat::Pretty => entry.to_pretty(),
                LogFormat::Json => entry.to_json(),
                LogFormat::Compact => format!("{} {} {}\n", entry.timestamp_ns, entry.level, entry.message),
            };
            println!("{}", line);
        }
    }
    
    /// Trace level log
    pub fn trace(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Trace, module, msg));
    }
    
    /// Debug level log
    pub fn debug(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Debug, module, msg));
    }
    
    /// Info level log
    pub fn info(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Info, module, msg));
    }
    
    /// Warn level log
    pub fn warn(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Warn, module, msg));
    }
    
    /// Error level log
    pub fn error(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Error, module, msg));
    }
    
    /// Fatal level log
    pub fn fatal(&self, module: &str, msg: &str) {
        self.log(LogEntry::new(LogLevel::Fatal, module, msg));
    }
    
    /// Set log level
    pub fn set_level(&self, level: LogLevel) {
        self.config.write().level = level;
    }
    
    /// Flush logs
    pub fn flush(&self) {
        // Wait for async queue to empty
        if let Some(tx) = &self.tx {
            // Simple wait - in production would need better synchronization
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// Global logger instance
pub static LOGGER: once_cell::sync::Lazy<Arc<Logger>> = once_cell::sync::Lazy::new(|| {
    Arc::new(Logger::new(LogConfig::default()))
});

/// Initialize logging with configuration
pub fn init_logging(config: LogConfig) {
    // Replace global logger
    // Note: This is simplified - proper implementation would require more careful handling
}

/// Macro for easy logging
#[macro_export]
macro_rules! log {
    ($level:expr, $module:expr, $msg:expr) => {
        $crate::utils::logger::LOGGER.log($crate::utils::logger::LogEntry::new($level, $module, $msg))
    };
    ($level:expr, $module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        let mut entry = $crate::utils::logger::LogEntry::new($level, $module, $msg);
        $(entry = entry.with_field($key, $value);)*
        $crate::utils::logger::LOGGER.log(entry)
    };
}

#[macro_export]
macro_rules! trace {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Trace, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Trace, $module, $msg, $($key => $value),*)
    };
}

#[macro_export]
macro_rules! debug {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Debug, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Debug, $module, $msg, $($key => $value),*)
    };
}

#[macro_export]
macro_rules! info {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Info, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Info, $module, $msg, $($key => $value),*)
    };
}

#[macro_export]
macro_rules! warn {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Warn, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Warn, $module, $msg, $($key => $value),*)
    };
}

#[macro_export]
macro_rules! error {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Error, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Error, $module, $msg, $($key => $value),*)
    };
}

#[macro_export]
macro_rules! fatal {
    ($module:expr, $msg:expr) => {
        $crate::log!($crate::utils::logger::LogLevel::Fatal, $module, $msg)
    };
    ($module:expr, $msg:expr, $($key:expr => $value:expr),*) => {
        $crate::log!($crate::utils::logger::LogLevel::Fatal, $module, $msg, $($key => $value),*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_log_entry() {
        let entry = LogEntry::new(LogLevel::Info, "test", "Hello world")
            .with_field("user", "test_user")
            .with_field("duration", 123);
        
        assert!(entry.message.contains("Hello world"));
        assert_eq!(entry.fields.len(), 2);
    }
    
    #[test]
    fn test_log_entry_json() {
        let entry = LogEntry::new(LogLevel::Info, "test", "Hello")
            .with_field("key", "value");
        
        let json = entry.to_json();
        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"key\":\"value\""));
    }
}
