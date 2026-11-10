use crate::syntax::auth::auth_config::require_non_empty;
use crate::syntax::auth::AuthConfig;
use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
const CLIENT_SECRET_FIELD: &str = "client_secret";
const AUTHORIZATION_URL_FIELD: &str = "authorization_url";
const TOKEN_URL_FIELD: &str = "token_url";
const REDIRECT_URI_FIELD: &str = "redirect_uri";
const SCOPE_FIELD: &str = "scope";
const CODE_CHALLENGE_METHOD_FIELD: &str = "code_challenge_method";
const USE_STATE_FIELD: &str = "use_state";

pub struct OAuth2AuthorizationCodeConfig;

impl OAuth2AuthorizationCodeConfig {
    pub fn new() -> Self {
        OAuth2AuthorizationCodeConfig
    }
}

impl Default for OAuth2AuthorizationCodeConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthConfig for OAuth2AuthorizationCodeConfig {
    fn auth_type(&self) -> &str {
        "oauth2_authorization_code"
    }

    fn required_fields(&self) -> &'static [&'static str] {
        &[
            CLIENT_ID_FIELD,
            AUTHORIZATION_URL_FIELD,
            TOKEN_URL_FIELD,
            REDIRECT_URI_FIELD,
        ]
    }

    fn optional_fields(&self) -> &'static [&'static str] {
        &[
            CLIENT_SECRET_FIELD,
            SCOPE_FIELD,
            CODE_CHALLENGE_METHOD_FIELD,
            USE_STATE_FIELD,
        ]
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        require_non_empty(self.auth_type(), name, fields, self.required_fields())?;

        if let Some(token) = fields.get(CODE_CHALLENGE_METHOD_FIELD) {
            let valid_methods = ["S256", "plain"];
            if !valid_methods.contains(&token.value.as_str()) {
                return Err(SyntaxError::new(
                    format!(
                        "{} auth '{name}' has invalid '{CODE_CHALLENGE_METHOD_FIELD}': '{}'. Valid values: S256, plain",
                        self.auth_type(),
                        token.value
                    ),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        if let Some(token) = fields.get(USE_STATE_FIELD) {
            if token.value != "true" && token.value != "false" {
                return Err(SyntaxError::new(
                    format!(
                        "{} auth '{name}' has invalid '{USE_STATE_FIELD}': '{}'. Valid values: true, false",
                        self.auth_type(),
                        token.value
                    ),
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
    use crate::syntax::token::{Token, TokenType};

    fn t(s: &str) -> Token {
        Token {
            token_type: TokenType::String,
            value: s.to_string(),
            span: 0..0,
        }
    }

    #[test]
    fn test_oauth2_auth_code_type() {
        let config = OAuth2AuthorizationCodeConfig::new();
        assert_eq!(config.auth_type(), "oauth2_authorization_code");
    }

    #[test]
    fn test_valid_oauth2_auth_code_minimal() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_use_state() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );
        fields.insert(USE_STATE_FIELD.to_string(), t("false"));

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_invalid_oauth2_auth_code_use_state() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );
        fields.insert(USE_STATE_FIELD.to_string(), t("invalid"));

        assert!(config.validate("test_auth", &fields).is_err());
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_pkce() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(CODE_CHALLENGE_METHOD_FIELD.to_string(), t("S256"));
        fields.insert(SCOPE_FIELD.to_string(), t("read write"));
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_client_secret() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(CLIENT_SECRET_FIELD.to_string(), t("secret123"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_oauth2_auth_code_empty_field() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("   "));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("has empty 'client_id' field"));
    }

    #[test]
    fn test_oauth2_auth_code_invalid_challenge_method() {
        let config = OAuth2AuthorizationCodeConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );
        fields.insert(
            REDIRECT_URI_FIELD.to_string(),
            t("http://localhost:8080/callback"),
        );
        fields.insert(CODE_CHALLENGE_METHOD_FIELD.to_string(), t("invalid"));

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("invalid 'code_challenge_method'"));
    }
}
