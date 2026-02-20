use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref NAME_REGEX: Regex = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_/-]*$").unwrap();
}

pub fn validate_path_exists(path: &str) -> Result<String, String> {
    if Path::new(path).exists() {
        Ok(path.to_string())
    } else {
        Err(format!("Path does not exist: {path}"))
    }
}

pub fn validate_name(name: &str) -> Result<String, String> {
    if name.len() > 50 {
        return Err("Name must be 50 characters or less".to_string());
    }
    if !NAME_REGEX.is_match(name) {
        return Err("Name must match pattern: ^[a-zA-Z_][a-zA-Z0-9_/-]*$".to_string());
    }
    Ok(name.to_string())
}

pub fn validate_variable(variable: &str) -> Result<String, String> {
    let parts: Vec<&str> = variable.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err("Variable must be in format NAME=VALUE".to_string());
    }

    let name = parts[0];
    if let Err(e) = validate_name(name) {
        return Err(format!("Invalid variable name: {e}"));
    }

    Ok(variable.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(validate_name("valid_name").is_ok());
        assert!(validate_name("valid-name").is_ok());
        assert!(validate_name("valid/name").is_ok());
        assert!(validate_name("valid_name/with/slashes").is_ok());

        assert!(validate_name("1invalid").is_err());
        assert!(validate_name("-invalid").is_err());
        assert!(validate_name("/invalid").is_err());
        assert!(validate_name("invalid name").is_err());
        assert!(validate_name("invalid!name").is_err());
    }
}
