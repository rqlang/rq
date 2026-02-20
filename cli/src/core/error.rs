use crate::syntax::error::{AuthError, SyntaxError};
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum RqError {
    Io(io::Error),
    Syntax(SyntaxError),
    Auth(AuthError),
    Validation(String),
    DirectoryNotFound(String),
    NotADirectory(String),
    RequestNotFound(String),
    EnvironmentNotFound(String),
    Generic(String),
}

impl fmt::Display for RqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RqError::Io(err) => write!(f, "IO error: {err}"),
            RqError::Syntax(err) => write!(f, "{err}"),
            RqError::Auth(err) => write!(f, "{err}"),
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
            RqError::Generic(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RqError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RqError::Io(err) => Some(err),
            RqError::Syntax(err) => Some(err),
            RqError::Auth(err) => Some(err),
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

impl From<AuthError> for RqError {
    fn from(err: AuthError) -> Self {
        RqError::Auth(err)
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
