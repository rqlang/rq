use std::sync::OnceLock;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub struct Logger {
    debug: bool,
}

impl Logger {
    pub fn init(debug: bool) {
        let _ = LOGGER.get_or_init(|| Logger { debug });
    }

    fn get() -> &'static Logger {
        LOGGER
            .get()
            .expect("Logger not initialized. Call Logger::init() in main first.")
    }

    pub fn debug(message: &str) {
        let logger = Self::get();
        if logger.debug {
            eprintln!("{message}");
        }
    }

    #[allow(dead_code)]
    pub fn debug_fmt(args: std::fmt::Arguments) {
        let logger = Self::get();
        if logger.debug {
            eprintln!("{args}");
        }
    }
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::core::logger::Logger::debug_fmt(format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_debug_enabled() {
        Logger::init(true);
        Logger::debug("Test message");
        Logger::debug_fmt(format_args!("Formatted: {}", "test"));
    }

    #[test]
    fn test_logger_debug_disabled() {
        Logger::init(false);
        Logger::debug("Test message");
        Logger::debug_fmt(format_args!("Formatted: {}", "test"));
    }
}
