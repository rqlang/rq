use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

pub trait AuthConfig: Send + Sync {
    #[allow(dead_code)]
    fn auth_type(&self) -> &str;

    fn required_fields(&self) -> &'static [&'static str];

    fn optional_fields(&self) -> &'static [&'static str];

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError>;
}

pub fn require_non_empty(
    label: &str,
    name: &str,
    fields: &HashMap<String, Token>,
    required_fields: &[&str],
) -> Result<(), SyntaxError> {
    for field in required_fields {
        if let Some(token) = fields.get(*field) {
            if token.value.trim().is_empty() {
                return Err(SyntaxError::new(
                    format!("{label} auth '{name}' has empty '{field}' field"),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }
    }
    Ok(())
}
