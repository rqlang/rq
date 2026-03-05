use std::sync::OnceLock;

use lazy_static::lazy_static;
use regex::Regex;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub struct Logger {
    debug: bool,
}

lazy_static! {
    static ref SENSITIVE_RE: Regex = Regex::new(
        r#"(?xi)
        (?P<json_key>"(?:password|passwd|token|secret|api_?key|auth(?:orization)?)")
        \s*:\s*
        "(?P<json_val>[^"]*)"
        |
        (?P<auth_bearer_key>auth(?:orization)?)
        (?P<auth_bearer_sep>[=:]\s*)
        Bearer\s+\S+
        |
        (?P<kv_key>(?:password|passwd|token|secret|api_?key))
        (?P<kv_sep>[=:]\s*)
        (?P<kv_val>\S+)
        |
        (?P<bearer>Bearer\s+)
        (?P<bearer_val>\S+)
        "#
    )
    .expect("SENSITIVE_RE is a valid regex");
}

fn sanitize_message(message: &str) -> String {
    SENSITIVE_RE
        .replace_all(message, |caps: &regex::Captures| {
            if let Some(key) = caps.name("json_key") {
                format!("{}: \"***\"", key.as_str())
            } else if let Some(key) = caps.name("auth_bearer_key") {
                let sep = caps.name("auth_bearer_sep").map_or(": ", |m| m.as_str());
                format!("{}{}Bearer ***", key.as_str(), sep)
            } else if let Some(key) = caps.name("kv_key") {
                let sep = caps.name("kv_sep").map_or("=", |m| m.as_str());
                format!("{}{}{}", key.as_str(), sep, "***")
            } else if let Some(bearer) = caps.name("bearer") {
                format!("{}***", bearer.as_str())
            } else {
                caps[0].to_owned()
            }
        })
        .into_owned()
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
    fn redacts_key_equals_value() {
        assert_eq!(sanitize_message("token=abc123"), "token=***");
    }

    #[test]
    fn redacts_key_colon_value() {
        assert_eq!(sanitize_message("password: hunter2"), "password: ***");
    }

    #[test]
    fn redacts_json_style() {
        assert_eq!(
            sanitize_message(r#"{"token":"abc123"}"#),
            r#"{"token": "***"}"#
        );
        assert_eq!(
            sanitize_message(r#"{"password": "s3cr3t"}"#),
            r#"{"password": "***"}"#
        );
    }

    #[test]
    fn redacts_bearer_token() {
        assert_eq!(
            sanitize_message("Authorization: Bearer eyJhbGciOiJSUzI1NiJ9"),
            "Authorization: Bearer ***"
        );
    }

    #[test]
    fn redacts_case_insensitive() {
        assert_eq!(sanitize_message("TOKEN=abc"), "TOKEN=***");
        assert_eq!(sanitize_message("Password=abc"), "Password=***");
        assert_eq!(sanitize_message("API_KEY=abc"), "API_KEY=***");
        assert_eq!(sanitize_message("APIKEY=abc"), "APIKEY=***");
    }

    #[test]
    fn redacts_multiple_occurrences_in_one_message() {
        let input = "token=abc password=xyz user=alice";
        let output = sanitize_message(input);
        assert!(output.contains("token=***"), "token not redacted: {output}");
        assert!(
            output.contains("password=***"),
            "password not redacted: {output}"
        );
        assert!(
            output.contains("user=alice"),
            "non-sensitive field was redacted: {output}"
        );
    }

    #[test]
    fn leaves_non_sensitive_fields_unchanged() {
        assert_eq!(
            sanitize_message("status=200 method=GET"),
            "status=200 method=GET"
        );
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(sanitize_message(""), "");
    }

    #[test]
    fn handles_message_with_no_sensitive_data() {
        let msg = "GET /health 200 OK";
        assert_eq!(sanitize_message(msg), msg);
    }

    #[test]
    fn logger_debug_enabled_does_not_panic() {
        Logger::init(true);
        Logger::debug("Test message");
        Logger::debug_fmt(format_args!("Formatted: {}", "test"));
    }

    #[test]
    fn logger_debug_disabled_does_not_panic() {
        Logger::init(false);
        Logger::debug("Test message");
        Logger::debug_fmt(format_args!("Formatted: {}", "test"));
    }
}
