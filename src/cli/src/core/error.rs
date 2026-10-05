pub use rq_lib::error::*;

#[derive(Debug)]
pub struct CheckFailed {
    pub error_count: usize,
}

impl std::fmt::Display for CheckFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Check found {} error(s)", self.error_count)
    }
}

impl std::error::Error for CheckFailed {}
