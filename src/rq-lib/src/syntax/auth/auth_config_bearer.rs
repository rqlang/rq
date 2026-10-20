use crate::syntax::auth::AuthConfig;
use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

const TOKEN_FIELD: &str = "token";

pub struct BearerAuthConfig;

impl BearerAuthConfig {
    pub fn new() -> Self {
        BearerAuthConfig
    }
}

impl Default for BearerAuthConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthConfig for BearerAuthConfig {
    fn auth_type(&self) -> &str {
        "bearer"
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        if let Some(token) = fields.get(TOKEN_FIELD) {
            if token.value.trim().is_empty() {
                return Err(SyntaxError::new(
                    format!("Bearer auth '{name}' has empty '{TOKEN_FIELD}' field"),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_auth_type() {
        let config = BearerAuthConfig::new();
        assert_eq!(config.auth_type(), "bearer");
    }

    #[test]
    fn test_valid_bearer_auth() {
        let config = BearerAuthConfig::new();
        let mut fields = HashMap::new();
        fields.insert(
            "token".to_string(),
            Token {
                token_type: crate::syntax::token::TokenType::String,
                value: "my-secret-token".to_string(),
                span: 0..0,
            },
        );

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_bearer_auth_missing_token() {
        let config = BearerAuthConfig::new();
        let fields = HashMap::new();

        let result = config.validate("test_auth", &fields);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bearer_auth_empty_token() {
        let config = BearerAuthConfig::new();
        let mut fields = HashMap::new();
        fields.insert(
            "token".to_string(),
            Token {
                token_type: crate::syntax::token::TokenType::String,
                value: "   ".to_string(),
                span: 10..15,
            },
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("empty 'token' field"));
        assert_eq!(err.span.start, 10);
    }

    #[test]
    fn test_bearer_auth_unexpected_field() {
        let config = BearerAuthConfig::new();
        let mut fields = HashMap::new();
        fields.insert(
            "token".to_string(),
            Token {
                token_type: crate::syntax::token::TokenType::String,
                value: "my-token".to_string(),
                span: 0..0,
            },
        );
        fields.insert(
            "extra".to_string(),
            Token {
                token_type: crate::syntax::token::TokenType::String,
                value: "not-allowed".to_string(),
                span: 0..0,
            },
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_ok());
    }
}
