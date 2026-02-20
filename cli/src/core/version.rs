pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_version_not_empty() {
        let version = app_version();
        assert!(!version.is_empty(), "Version should not be empty");
    }

    #[test]
    fn test_app_version_format() {
        let version = app_version();
        assert!(
            version.contains('.'),
            "Version should be in semantic format"
        );
    }
}
