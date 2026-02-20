use crate::core::error::RqError;

/// Exit codes for the RQ CLI
/// Following standard Unix/POSIX conventions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ExitCode {
    /// Success
    Success = 0,
    /// General/unspecified error
    GeneralError = 1,
    /// Syntax error in RQ file
    SyntaxError = 2,
    /// Configuration error (auth, environment)
    ConfigError = 3,
    /// File not found or IO error
    FileError = 4,
    /// Request or resource not found
    NotFoundError = 5,
    /// Network or HTTP error
    NetworkError = 6,
    /// Authentication error
    AuthError = 7,
    /// Variable resolution error
    VariableError = 8,
}

impl ExitCode {
    pub fn code(&self) -> i32 {
        *self as i32
    }
}

impl From<&Box<dyn std::error::Error>> for ExitCode {
    fn from(error: &Box<dyn std::error::Error>) -> Self {
        if let Some(rq_error) = error.downcast_ref::<RqError>() {
            match rq_error {
                RqError::Io(_) => ExitCode::FileError,
                RqError::Syntax(_) => ExitCode::SyntaxError,
                RqError::Auth(_) => ExitCode::AuthError,
                RqError::Validation(_) => ExitCode::ConfigError,
                RqError::DirectoryNotFound(_) => ExitCode::FileError,
                RqError::NotADirectory(_) => ExitCode::FileError,
                RqError::RequestNotFound(_) => ExitCode::NotFoundError,
                RqError::EnvironmentNotFound(_) => ExitCode::ConfigError,
                RqError::Generic(_) => ExitCode::GeneralError,
            }
        } else {
            let error_str = format!("{error:?}");
            let error_display = format!("{error}");

            // Check error type by string representation (both Debug and Display formats)
            if error_str.contains("SyntaxError") {
                ExitCode::SyntaxError
            } else if error_str.contains("AuthError") || error_display.contains("Auth error") {
                ExitCode::AuthError
            } else if error_display.contains("does not exist")
                || error_display.contains("not found")
                || error_display.contains("Not found")
                || error_display.contains("No such file")
                || error_display.contains("Not a directory")
            {
                if error_display.contains("Request") || error_display.contains("request") {
                    ExitCode::NotFoundError
                } else {
                    ExitCode::FileError
                }
            } else if error_display.contains("Environment")
                || error_display.contains("Configuration")
            {
                ExitCode::ConfigError
            } else if error_display.contains("Variable")
                || error_display.contains("Resolution")
                || error_display.contains("Circular reference")
            {
                ExitCode::VariableError
            } else if error_display.contains("HTTP")
                || error_display.contains("Network")
                || error_display.contains("Connection")
                || error_display.contains("request failed")
            {
                ExitCode::NetworkError
            } else {
                ExitCode::GeneralError
            }
        }
    }
}
