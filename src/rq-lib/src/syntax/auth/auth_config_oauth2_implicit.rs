use crate::syntax::auth::auth_config::require_non_empty;
use crate::syntax::auth::AuthConfig;
use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
const AUTHORIZATION_URL_FIELD: &str = "authorization_url";
const SCOPE_FIELD: &str = "scope";
const REDIRECT_URI_FIELD: &str = "redirect_uri";

pub struct OAuth2ImplicitConfig;

impl OAuth2ImplicitConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OAuth2ImplicitConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthConfig for OAuth2ImplicitConfig {
    fn auth_type(&self) -> &'static str {
        "oauth2_implicit"
    }

    fn required_fields(&self) -> &'static [&'static str] {
        &[CLIENT_ID_FIELD, AUTHORIZATION_URL_FIELD, REDIRECT_URI_FIELD]
    }

    fn optional_fields(&self) -> &'static [&'static str] {
        &[SCOPE_FIELD]
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        require_non_empty("OAuth2 Implicit", name, fields, self.required_fields())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::token::{Token, TokenType};

    fn t(s: &str) -> Token {
        Token {
            token_type: TokenType::String,
            value: s.to_string(),
            span: 0..0,
        }
    }

    #[test]
    fn test_implicit_type() {
        let config = OAuth2ImplicitConfig::new();
        assert_eq!(config.auth_type(), "oauth2_implicit");
    }

    #[test]
    fn test_valid_implicit_minimal() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(REDIRECT_URI_FIELD.to_string(), t("http://localhost:8080"));

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_valid_implicit_full() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(SCOPE_FIELD.to_string(), t("read write"));
        fields.insert(REDIRECT_URI_FIELD.to_string(), t("http://localhost:8080"));

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_implicit_blank_redirect_uri() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(REDIRECT_URI_FIELD.to_string(), t("  "));

        let result = config.validate("test_auth", &fields);
        assert!(result
            .unwrap_err()
            .message
            .contains("has empty 'redirect_uri' field"));
    }

    #[test]
    fn test_implicit_empty_authorization_url() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(AUTHORIZATION_URL_FIELD.to_string(), t("  "));
        fields.insert(REDIRECT_URI_FIELD.to_string(), t("http://localhost:8080"));

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("has empty 'authorization_url' field"));
    }
}
