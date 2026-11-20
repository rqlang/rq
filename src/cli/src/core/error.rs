pub use rq_lib::error::*;

#[derive(Debug)]
pub struct CheckFailed {
    pub error_count: usize,
    pub warning_count: usize,
}

impl std::fmt::Display for CheckFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Check found {} error(s) and {} warning(s)",
            self.error_count, self.warning_count
        )
    }
}

impl std::error::Error for CheckFailed {}

#[derive(Debug)]
pub struct BodyNotWritten {
    pub path: String,
    pub reason: String,
}

impl std::fmt::Display for BodyNotWritten {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Body not written to {}: {}", self.path, self.reason)
    }
}

impl std::error::Error for BodyNotWritten {}
