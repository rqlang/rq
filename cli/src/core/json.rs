pub const CONTENT_TYPE: &str = "application/json";

pub fn is_json_content(content: &str) -> bool {
    let trimmed = content.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_json_content_object() {
        assert!(is_json_content(r#"{"key": "value"}"#));
        assert!(is_json_content(r#"  {"key": "value"}  "#));
        assert!(is_json_content("{}"));
    }

    #[test]
    fn test_is_json_content_array() {
        assert!(is_json_content(r#"["item1", "item2"]"#));
        assert!(is_json_content(r#"  ["item1", "item2"]  "#));
        assert!(is_json_content("[]"));
    }

    #[test]
    fn test_is_json_content_not_json() {
        assert!(!is_json_content("plain text"));
        assert!(!is_json_content("{not closed"));
        assert!(!is_json_content("[not closed"));
        assert!(!is_json_content("closed}"));
        assert!(!is_json_content("closed]"));
    }
}
