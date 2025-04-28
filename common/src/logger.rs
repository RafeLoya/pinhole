use chrono::{DateTime, Local};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Log levels in order of increasing severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warning => write!(f, "WARNING"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Logger configuration
pub struct LoggerConfig {
    /// Path to log file
    pub log_file: String,
    /// Minimum log level to record
    pub min_level: LogLevel,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        LoggerConfig {
            log_file: "logs/debug.log".to_string(),
            min_level: LogLevel::Debug,
        }
    }
}

/// Logger to write log messages to a file
///
/// # Examples
///
/// ```
/// use common::logger::{Logger, LoggerConfig, LogLevel};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create logger with default configuration
/// let default_logger = Logger::new()?;
/// default_logger.info("This is an info message using the default log file")?;
///
/// // Create logger with custom file name
/// let custom_logger = Logger::with_file_name("application.log")?;
/// custom_logger.info("This is an info message using a custom log file")?;
///
/// // Create logger with full custom configuration
/// let custom_config = LoggerConfig {
///     log_file: "error_only.log".to_string(),
///     min_level: LogLevel::Error,
/// };
/// let error_logger = Logger::with_config(custom_config)?;
/// error_logger.info("This info won't be logged")?;
/// error_logger.error("This error will be logged")?;
/// # Ok(())
/// # }
/// ```
pub struct Logger {
    config: LoggerConfig,
    file: Arc<Mutex<File>>,
}

impl Logger {
    /// Create a new logger with the default configuration
    ///
    /// Uses "debug.log" as the default log file and Debug as the minimum log level.
    ///
    /// # Example
    ///
    /// ```
    /// use common::logger::Logger;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let logger = Logger::new()?;
    /// logger.info("This is an info message using the default log file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self, std::io::Error> {
        Self::with_config(LoggerConfig::default())
    }

    /// Create a new logger with a custom file name
    ///
    /// Uses the specified file name but keeps the default minimum log level (Debug).
    ///
    /// # Example
    ///
    /// ```
    /// use common::logger::Logger;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let logger = Logger::with_file_name("application.log")?;
    /// logger.info("This is an info message using a custom log file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_file_name(file_name: &str) -> Result<Self, std::io::Error> {
        let config = LoggerConfig {
            log_file: file_name.to_string(),
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create a new logger with a custom configuration
    ///
    /// Allows specifying both the log file name and minimum log level.
    ///
    /// # Example
    ///
    /// ```
    /// use common::logger::{Logger, LoggerConfig, LogLevel};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let custom_config = LoggerConfig {
    ///     log_file: "error_only.log".to_string(),
    ///     min_level: LogLevel::Error,
    /// };
    /// let logger = Logger::with_config(custom_config)?;
    /// logger.info("This info won't be logged")?;
    /// logger.error("This error will be logged")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config(config: LoggerConfig) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file)?;

        Ok(Logger {
            config,
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Log a message at the specified level
    ///
    /// Only logs the message if the specified level is greater than or equal to
    /// the logger's minimum log level.
    ///
    /// # Example
    ///
    /// ```
    /// use common::logger::{Logger, LogLevel};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let logger = Logger::new()?;
    /// // Log a message at INFO level
    /// logger.log(LogLevel::Info, "This is an info message")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn log(&self, level: LogLevel, message: &str) -> Result<(), std::io::Error> {
        if level < self.config.min_level {
            return Ok(());
        }

        let timestamp: DateTime<Local> = Local::now();
        // Include milliseconds in the timestamp format
        let formatted_timestamp = timestamp.format("%Y-%m-%d %H:%M:%S%.6f %:z").to_string();

        let log_entry = format!("{} [{}] {}\n", formatted_timestamp, level, message);

        let mut file = self.file.lock().unwrap();
        file.write_all(log_entry.as_bytes())?;
        file.flush()?;

        Ok(())
    }

    /// Log a trace message
    pub fn trace(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Trace, message)
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Debug, message)
    }

    /// Log an info message
    pub fn info(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Info, message)
    }

    /// Log a warning message
    pub fn warn(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Warning, message)
    }

    /// Log an error message
    pub fn error(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Error, message)
    }

    /// Log a fatal message
    pub fn fatal(&self, message: &str) -> Result<(), std::io::Error> {
        self.log(LogLevel::Fatal, message)
    }
}
