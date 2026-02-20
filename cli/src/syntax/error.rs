#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub span: std::ops::Range<usize>,
    pub file_path: Option<String>,
}
impl SyntaxError {
    pub fn new(message: String, line: usize, column: usize, span: std::ops::Range<usize>) -> Self {
        Self {
            message,
            line,
            column,
            span,
            file_path: None,
        }
    }
    pub fn with_file(
        message: String,
        line: usize,
        column: usize,
        span: std::ops::Range<usize>,
        file_path: String,
    ) -> Self {
        Self {
            message,
            line,
            column,
            span,
            file_path: Some(file_path),
        }
    }
}
impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(file) = &self.file_path {
            let path = std::path::Path::new(file);
            let display_path = if let Ok(cwd) = std::env::current_dir() {
                if let Ok(stripped) = path.strip_prefix(&cwd) {
                    stripped.display().to_string()
                } else {
                    crate::core::paths::clean_path(path)
                }
            } else {
                crate::core::paths::clean_path(path)
            };
            write!(
                f,
                "Syntax error in {} at line {}, column {}: {}",
                display_path, self.line, self.column, self.message
            )
        } else {
            write!(
                f,
                "Syntax error at line {}, column {}: {}",
                self.line, self.column, self.message
            )
        }
    }
}
impl std::error::Error for SyntaxError {}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthError {
    pub message: String,
}
impl AuthError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Auth error: {}", self.message)
    }
}
impl std::error::Error for AuthError {}
