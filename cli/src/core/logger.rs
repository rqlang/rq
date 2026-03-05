use std::sync::OnceLock;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub struct Logger {
    debug: bool,
}

fn sanitize_message(message: &str) -> String {
    // Simple best-effort redaction of obviously sensitive key-value pairs.
    // This avoids logging secrets like passwords or tokens in cleartext.
    let sensitive_keys = ["password", "passwd", "token", "secret", "apikey", "api_key", "auth"];

    let mut redacted = String::with_capacity(message.len());

    for (i, part) in message.split_whitespace().enumerate() {
        let mut replaced = false;

        for key in &sensitive_keys {
            if let Some(rest) = part.strip_prefix(&format!("{key}=")) {
                let _ = rest; // suppress unused warning if not in debug assertions
                redacted.push_str(key);
                redacted.push('=');
                redacted.push_str("***");
                replaced = true;
                break;
            } else if let Some(rest) = part.strip_prefix(&format!("{key}:")) {
                let _ = rest;
                redacted.push_str(key);
                redacted.push(':');
                redacted.push_str("***");
                replaced = true;
                break;
            }
        }

        if !replaced {
            redacted.push_str(part);
        }

        if i != message.split_whitespace().count().saturating_sub(1) {
            redacted.push(' ');
        }
    }

    if redacted.is_empty() {
        message.to_owned()
    } else {
        redacted
    }
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
            let sanitized = sanitize_message(message);
            eprintln!("{sanitized}");
        }
    }

    #[allow(dead_code)]
    pub fn debug_fmt(args: std::fmt::Arguments) {
        let logger = Self::get();
        if logger.debug {
            let formatted = format!("{args}");
            let sanitized = sanitize_message(&formatted);
            eprintln!("{sanitized}");
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
