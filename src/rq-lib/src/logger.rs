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
        (?P<json_key>"(?:[^"]*(?:password|passwd|token|secret|credential|api[_-]?key|authorization)[^"]*|auth)")
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

    pub fn is_debug_enabled() -> bool {
        LOGGER.get().is_some_and(|logger| logger.debug)
    }

    pub fn debug(message: &str) {
        if Self::is_debug_enabled() {
            let sanitized = sanitize_message(message);
            eprintln!("{sanitized}");
        }
    }

    #[allow(dead_code)]
    pub fn debug_fmt(args: std::fmt::Arguments) {
        if Self::is_debug_enabled() {
            let formatted = format!("{args}");
            let sanitized = sanitize_message(&formatted);
            eprintln!("{sanitized}");
        }
    }

    pub fn mask_header_value(name: &str, value: &str) -> String {
        let name = name.to_ascii_lowercase();
        let sensitive = [
            "authorization",
            "cookie",
            "token",
            "secret",
            "key",
            "password",
        ]
        .iter()
        .any(|marker| name.contains(marker));
        if sensitive {
            "***".to_string()
        } else {
            value.to_string()
        }
    }
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::logger::Logger::debug_fmt(format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::Logger;

    #[test]
    fn sensitive_header_value_is_masked() {
        let target = Logger::mask_header_value("X-Api-Key", "abc123");
        assert_eq!(target, "***");
    }

    #[test]
    fn regular_header_value_is_kept() {
        let target = Logger::mask_header_value("Content-Type", "application/json");
        assert_eq!(target, "application/json");
    }

    #[test]
    fn json_key_containing_secret_is_masked() {
        let target = super::sanitize_message(r#"{"client_secret": "abc", "user": "ana"}"#);
        assert_eq!(target, r#"{"client_secret": "***", "user": "ana"}"#);
    }

    #[test]
    fn json_key_containing_token_is_masked() {
        let target = super::sanitize_message(r#"{"access_token":"abc"}"#);
        assert_eq!(target, r#"{"access_token": "***"}"#);
    }

    #[test]
    fn json_key_with_hyphenated_api_key_is_masked() {
        let target = super::sanitize_message(r#"{"x-api-key": "abc"}"#);
        assert_eq!(target, r#"{"x-api-key": "***"}"#);
    }

    #[test]
    fn json_key_that_only_starts_like_auth_is_kept() {
        let target = super::sanitize_message(r#"{"author": "ana"}"#);
        assert_eq!(target, r#"{"author": "ana"}"#);
    }

    #[test]
    fn debug_without_init_does_not_panic() {
        Logger::debug("message");
    }
}
