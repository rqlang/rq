use crate::syntax::error::SyntaxError;
use serde::Serialize;
use std::fmt;
use std::io;

#[derive(Serialize)]
struct JsonErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

#[derive(Serialize)]
struct JsonError {
    error: JsonErrorDetail,
}

pub fn error_to_json(error: &(dyn std::error::Error + 'static)) -> String {
    let detail = if let Some(rq_error) = error.downcast_ref::<RqError>() {
        match rq_error {
            RqError::Syntax(e) => JsonErrorDetail {
                error_type: "syntax".to_string(),
                message: e.message.clone(),
                file: e
                    .file_path
                    .as_deref()
                    .map(crate::core::paths::clean_path_str)
                    .map(str::to_string),
                line: if e.line > 0 { Some(e.line) } else { None },
                column: if e.column > 0 { Some(e.column) } else { None },
            },
            RqError::Auth(msg) => JsonErrorDetail {
                error_type: "auth".to_string(),
                message: msg.clone(),
                file: None,
                line: None,
                column: None,
            },
            RqError::Validation(msg) => JsonErrorDetail {
                error_type: "validation".to_string(),
                message: msg.clone(),
                file: None,
                line: None,
                column: None,
            },
            RqError::DirectoryNotFound(path) => JsonErrorDetail {
                error_type: "file".to_string(),
                message: format!("Directory not found: {path}"),
                file: None,
                line: None,
                column: None,
            },
            RqError::NotADirectory(path) => JsonErrorDetail {
                error_type: "file".to_string(),
                message: format!("Not a directory: {path}"),
                file: None,
                line: None,
                column: None,
            },
            RqError::RequestNotFound(name) => JsonErrorDetail {
                error_type: "not_found".to_string(),
                message: format!("Request not found: {name}"),
                file: None,
                line: None,
                column: None,
            },
            RqError::EnvironmentNotFound(name) => JsonErrorDetail {
                error_type: "not_found".to_string(),
                message: format!("Environment not found: {name}"),
                file: None,
                line: None,
                column: None,
            },
            RqError::Io(e) => JsonErrorDetail {
                error_type: "io".to_string(),
                message: e.to_string(),
                file: None,
                line: None,
                column: None,
            },
            RqError::Network(msg) => JsonErrorDetail {
                error_type: "network".to_string(),
                message: msg.clone(),
                file: None,
                line: None,
                column: None,
            },
            RqError::Generic(msg) => JsonErrorDetail {
                error_type: "generic".to_string(),
                message: msg.clone(),
                file: None,
                line: None,
                column: None,
            },
        }
    } else {
        JsonErrorDetail {
            error_type: "generic".to_string(),
            message: error.to_string(),
            file: None,
            line: None,
            column: None,
        }
    };

    serde_json::to_string(&JsonError { error: detail }).unwrap_or_else(|_| {
        format!(
            r#"{{"error":{{"type":"generic","message":{:?}}}}}"#,
            error.to_string()
        )
    })
}

#[derive(Serialize)]
struct JsonWarning {
    warning: JsonErrorDetail,
}

pub fn warning_to_json(message: &str) -> String {
    let detail = JsonErrorDetail {
        error_type: "parse".to_string(),
        message: message.to_string(),
        file: None,
        line: None,
        column: None,
    };
    serde_json::to_string(&JsonWarning { warning: detail })
        .unwrap_or_else(|_| format!(r#"{{"warning":{{"type":"parse","message":{message:?}}}}}"#))
}

#[derive(Debug)]
pub enum RqError {
    Io(io::Error),
    Syntax(SyntaxError),
    Auth(String),
    Validation(String),
    DirectoryNotFound(String),
    NotADirectory(String),
    RequestNotFound(String),
    EnvironmentNotFound(String),
    Network(String),
    Generic(String),
}

impl fmt::Display for RqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RqError::Io(err) => write!(f, "IO error: {err}"),
            RqError::Syntax(err) => write!(f, "{err}"),
            RqError::Auth(msg) => write!(f, "Auth error: {msg}"),
            RqError::Validation(msg) => write!(f, "Validation error: {msg}"),
            RqError::DirectoryNotFound(path) => write!(
                f,
                "Directory not found: {}",
                crate::core::paths::clean_path_str(path)
            ),
            RqError::NotADirectory(path) => write!(
                f,
                "Not a directory: {}",
                crate::core::paths::clean_path_str(path)
            ),
            RqError::RequestNotFound(name) => write!(f, "Request not found: {name}"),
            RqError::EnvironmentNotFound(name) => write!(f, "Environment not found: {name}"),
            RqError::Network(msg) => write!(f, "{msg}"),
            RqError::Generic(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RqError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RqError::Io(err) => Some(err),
            RqError::Syntax(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for RqError {
    fn from(err: io::Error) -> Self {
        RqError::Io(err)
    }
}

impl From<SyntaxError> for RqError {
    fn from(err: SyntaxError) -> Self {
        RqError::Syntax(err)
    }
}

impl From<String> for RqError {
    fn from(msg: String) -> Self {
        RqError::Generic(msg)
    }
}

impl From<&str> for RqError {
    fn from(msg: &str) -> Self {
        RqError::Generic(msg.to_string())
    }
}
