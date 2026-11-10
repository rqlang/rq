use crate::syntax::auth::auth_config::require_non_empty;
use crate::syntax::auth::AuthConfig;
use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
const CLIENT_SECRET_FIELD: &str = "client_secret";
const TOKEN_URL_FIELD: &str = "token_url";
const SCOPE_FIELD: &str = "scope";
const CERT_FILE_FIELD: &str = "cert_file";
const CERT_PASSWORD_FIELD: &str = "cert_password";

pub struct OAuth2ClientCredentialsConfig;

impl OAuth2ClientCredentialsConfig {
    pub fn new() -> Self {
        OAuth2ClientCredentialsConfig
    }
}

impl Default for OAuth2ClientCredentialsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthConfig for OAuth2ClientCredentialsConfig {
    fn auth_type(&self) -> &str {
        "oauth2_client_credentials"
    }

    fn required_fields(&self) -> &'static [&'static str] {
        &[CLIENT_ID_FIELD, TOKEN_URL_FIELD]
    }

    fn optional_fields(&self) -> &'static [&'static str] {
        &[
            CLIENT_SECRET_FIELD,
            SCOPE_FIELD,
            CERT_FILE_FIELD,
            CERT_PASSWORD_FIELD,
        ]
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        if !fields.contains_key(CLIENT_SECRET_FIELD) && !fields.contains_key(CERT_FILE_FIELD) {
            return Err(SyntaxError::new(
                format!(
                    "OAuth2 Client Credentials auth '{name}' must have either '{CLIENT_SECRET_FIELD}' or '{CERT_FILE_FIELD}'"
                ),
                0,
                0,
                0..0,
            ));
        }

        require_non_empty(
            "OAuth2 Client Credentials",
            name,
            fields,
            self.required_fields(),
        )
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
    fn test_client_credentials_type() {
        let provider = OAuth2ClientCredentialsConfig::new();
        assert_eq!(provider.auth_type(), "oauth2_client_credentials");
    }

    #[test]
    fn test_valid_client_credentials() {
        let provider = OAuth2ClientCredentialsConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(CLIENT_SECRET_FIELD.to_string(), t("my-secret"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_client_credentials_empty_field() {
        let provider = OAuth2ClientCredentialsConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t(""));
        fields.insert(CLIENT_SECRET_FIELD.to_string(), t("my-secret"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_err());
    }

    #[test]
    fn test_valid_client_credentials_with_cert() {
        let provider = OAuth2ClientCredentialsConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(CERT_FILE_FIELD.to_string(), t("/path/to/cert.p12"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_client_credentials_missing_secret_and_cert() {
        let provider = OAuth2ClientCredentialsConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        let result = provider.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must have either"));
    }
}
