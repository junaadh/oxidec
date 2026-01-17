//! A minimal, zero-dependency logging crate for the `OxideX` project.
//!
//! This crate provides thread-safe logging with automatic module path detection,
//! colored terminal output, and configurable log levels.
//!
//! # Example
//!
//! ```
//! use oxidex_log::{error, warn, info, debug, Level};
//!
//! // Set the minimum log level
//! oxidex_log::set_level(Level::Debug);
//!
//! let status = "running";
//! info!("Application is {}", status);
//! debug!("Debug information: {:?}", vec![1, 2, 3]);
//! warn!("This is a warning");
//! error!("This is an error message");
//! ```

use std::fmt::Arguments;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// Log levels representing the severity/priority of log messages.
///
/// `Levels` are ordered from most severe (Error) to least severe (Trace).
/// Lower numeric values indicate higher severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Error level - critical failures and errors
    Error = 0,
    /// Warning level - potentially harmful situations
    Warn = 1,
    /// Info level - informational messages
    Info = 2,
    /// Debug level - detailed diagnostic information
    Debug = 3,
    /// Trace level - most detailed tracing information
    Trace = 4,
}

impl Level {
    /// Returns the ANSI color code for this log level.
    const fn color_code(&self) -> &'static str {
        match self {
            Level::Error => "\x1b[31m", // Red
            Level::Warn => "\x1b[33m",  // Yellow
            Level::Info => "\x1b[32m",  // Green
            Level::Debug => "\x1b[36m", // Cyan
            Level::Trace => "\x1b[35m", // Magenta
        }
    }

    /// Returns the string representation of this log level.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    /// Parses a string into a Level.
    ///
    /// # Example
    ///
    /// ```
    /// use oxidex_log::Level;
    ///
    /// assert_eq!(Level::from_str("error"), Ok(Level::Error));
    /// assert_eq!(Level::from_str("INFO"), Ok(Level::Info));
    /// assert!(Level::from_str("invalid").is_err());
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "ERROR" => Ok(Level::Error),
            "WARN" => Ok(Level::Warn),
            "INFO" => Ok(Level::Info),
            "DEBUG" => Ok(Level::Debug),
            "TRACE" => Ok(Level::Trace),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// The global logger instance.
///
/// This struct uses atomic operations for thread-safe level management.
/// It is intended to be used as a singleton via `get_logger()`.
pub struct Logger {
    level: AtomicU8,
}

impl Logger {
    /// Creates a new logger with the specified minimum level.
    const fn new(level: Level) -> Self {
        Logger {
            level: AtomicU8::new(level as u8),
        }
    }

    /// Sets the minimum log level.
    ///
    /// Messages below this level will not be logged.
    pub fn set_level(&self, level: Level) {
        self.level.store(level as u8, Ordering::SeqCst);
    }

    /// Returns the current minimum log level.
    pub fn level(&self) -> Level {
        match self.level.load(Ordering::Relaxed) {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            _ => Level::Info, // Default fallback
        }
    }

    /// Checks if a message at the given level would be logged.
    pub fn enabled(&self, level: Level) -> bool {
        level as u8 <= self.level.load(Ordering::Relaxed)
    }
}

/// Global logger singleton.
static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Returns a reference to the global logger instance.
///
/// This initializes the logger on first call with `Level::Info` as the default level.
///
/// # Example
///
/// ```
/// use oxidex_log::get_logger;
///
/// let logger = get_logger();
/// logger.set_level(oxidex_log::Level::Debug);
/// ```
pub fn get_logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger::new(Level::Info))
}

/// Sets the minimum log level for the global logger.
///
/// # Example
///
/// ```
/// use oxidex_log::{set_level, Level};
///
/// set_level(Level::Debug);
/// ```
pub fn set_level(level: Level) {
    get_logger().set_level(level);
}

/// Sets the minimum log level from a string.
///
/// # Example
///
/// ```
/// use oxidex_log::set_level_from_str;
///
/// set_level_from_str("debug").unwrap();
/// ```
pub fn set_level_from_str(s: &str) -> Result<(), String> {
    let level = Level::from_str(s)?;
    set_level(level);
    Ok(())
}

/// Internal function that performs the actual logging.
///
/// This function is called by the log macros after checking if the level is enabled.
#[doc(hidden)]
pub fn __log_with_target(level: Level, target: &str, args: Arguments) {
    static RESET: &str = "\x1b[0m";

    if !get_logger().enabled(level) {
        return;
    }

    let color = level.color_code();
    let level_str = level.as_str();

    println!("{color}[{level_str}]{RESET} {target}: {args}");
}

/// The primary logging macro.
///
/// Logs a message at the specified level. The macro automatically captures
/// the module path where it was called.
///
/// # Example
///
/// ```
/// use oxidex_log::{log, Level};
///
/// # oxidex_log::set_level(Level::Info);
/// log!(level: Level::Info, "This is an info message: {}", 42);
/// ```
#[macro_export]
macro_rules! log {
    (level: $level:expr, $($arg:tt)*) => {
        {
            if $crate::get_logger().enabled($level) {
                $crate::__log_with_target(
                    $level,
                    module_path!(),
                    format_args!($($arg)*)
                );
            }
        }
    };
}

/// Logs a message at the Error level.
///
/// # Example
///
/// ```
/// use oxidex_log::error;
///
/// # let path = "/tmp/test.txt";
/// error!("Failed to open file: {}", path);
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!(level: $crate::Level::Error, $($arg)*)
    };
}

/// Logs a message at the Warn level.
///
/// # Example
///
/// ```
/// use oxidex_log::warn;
///
/// # oxidex_log::set_level(oxidex_log::Level::Warn);
/// warn!("Deprecated feature used");
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log!(level: $crate::Level::Warn, $($arg)*)
    };
}

/// Logs a message at the Info level.
///
/// # Example
///
/// ```
/// use oxidex_log::info;
///
/// # oxidex_log::set_level(oxidex_log::Level::Info);
/// info!("Application started successfully");
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!(level: $crate::Level::Info, $($arg)*)
    };
}

/// Logs a message at the Debug level.
///
/// # Example
///
/// ```
/// use oxidex_log::debug;
///
/// # let request = vec![1, 2, 3];
/// # oxidex_log::set_level(oxidex_log::Level::Debug);
/// debug!("Processing request: {:?}", request);
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log!(level: $crate::Level::Debug, $($arg)*)
    };
}

/// Logs a message at the Trace level.
///
/// # Example
///
/// ```
/// use oxidex_log::trace;
///
/// # let function_name = "process_data";
/// # oxidex_log::set_level(oxidex_log::Level::Trace);
/// trace!("Entering function: {}", function_name);
/// ```
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::log!(level: $crate::Level::Trace, $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_ordering() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Info);
        assert!(Level::Info < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }

    #[test]
    fn test_level_from_str() {
        assert_eq!(Level::from_str("error"), Ok(Level::Error));
        assert_eq!(Level::from_str("WARN"), Ok(Level::Warn));
        assert_eq!(Level::from_str("Info"), Ok(Level::Info));
        assert_eq!(Level::from_str("DEBUG"), Ok(Level::Debug));
        assert_eq!(Level::from_str("trace"), Ok(Level::Trace));
        assert!(Level::from_str("invalid").is_err());
    }

    #[test]
    fn test_level_as_str() {
        assert_eq!(Level::Error.as_str(), "ERROR");
        assert_eq!(Level::Warn.as_str(), "WARN");
        assert_eq!(Level::Info.as_str(), "INFO");
        assert_eq!(Level::Debug.as_str(), "DEBUG");
        assert_eq!(Level::Trace.as_str(), "TRACE");
    }

    #[test]
    fn test_logger_level_filtering() {
        let logger = Logger::new(Level::Info);

        assert!(logger.enabled(Level::Error));
        assert!(logger.enabled(Level::Warn));
        assert!(logger.enabled(Level::Info));
        assert!(!logger.enabled(Level::Debug));
        assert!(!logger.enabled(Level::Trace));

        logger.set_level(Level::Debug);

        assert!(logger.enabled(Level::Debug));
        assert!(!logger.enabled(Level::Trace));

        logger.set_level(Level::Trace);

        assert!(logger.enabled(Level::Trace));
    }

    #[test]
    fn test_set_level_from_str() {
        set_level_from_str("debug").unwrap();
        assert_eq!(get_logger().level(), Level::Debug);

        set_level_from_str("ERROR").unwrap();
        assert_eq!(get_logger().level(), Level::Error);

        assert!(set_level_from_str("invalid").is_err());
    }

    #[test]
    fn test_global_logger_singleton() {
        // Reset to Info level
        set_level(Level::Info);
        assert_eq!(get_logger().level(), Level::Info);

        // Change level
        set_level(Level::Debug);
        assert_eq!(get_logger().level(), Level::Debug);

        // Verify it's the same instance
        let logger1 = get_logger();
        let logger2 = get_logger();
        logger1.set_level(Level::Warn);
        assert_eq!(logger2.level(), Level::Warn);
    }

    #[test]
    fn test_macros_basic() {
        set_level(Level::Info);

        info!("This is an info message");
        debug!("This debug message should not appear");

        set_level(Level::Debug);
        debug!("Now debug messages should appear");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        set_level(Level::Info);

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    info!("Thread {} message", i);
                    debug!("Thread {} debug (should not show)", i);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_module_path_capture() {
        set_level(Level::Info);

        // This will show the full module path
        info!("Testing module path capture");
    }
}
