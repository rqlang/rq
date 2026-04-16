use crate::syntax::auth::{AuthConfig, AuthFuture};
use crate::syntax::error::{AuthError, SyntaxError};
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
const AUTHORIZATION_URL_FIELD: &str = "authorization_url";
const SCOPE_FIELD: &str = "scope";
const REDIRECT_URI_FIELD: &str = "redirect_uri";

const REQUIRED_FIELDS: [&str; 2] = [CLIENT_ID_FIELD, AUTHORIZATION_URL_FIELD];
const OPTIONAL_FIELDS: [&str; 2] = [SCOPE_FIELD, REDIRECT_URI_FIELD];

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

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        for field in &REQUIRED_FIELDS {
            if !fields.contains_key(*field) {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Implicit auth '{name}' is missing required field '{field}'. Required fields: {}",
                        REQUIRED_FIELDS.join(", ")
                    ),
                    0,
                    0,
                    if let Some(first) = fields.values().next() {
                        first.span.clone()
                    } else {
                        0..0
                    }
                ));
            }
        }

        for (field_name, token) in fields {
            if !REQUIRED_FIELDS.contains(&field_name.as_str())
                && !OPTIONAL_FIELDS.contains(&field_name.as_str())
            {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Implicit auth '{name}' has unexpected field '{field_name}'. Expected fields: {}, {}",
                        REQUIRED_FIELDS.join(", "),
                        OPTIONAL_FIELDS.join(", ")
                    ),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        Ok(())
    }

    fn configure<'a>(
        &'a self,
        auth_config: &'a crate::syntax::auth::Config,
        context: &'a crate::syntax::variable_context::VariableContext,
        url: String,
        headers: Vec<(String, String)>,
    ) -> AuthFuture<'a> {
        Box::pin(async move {
            let variables = context.all_variables();
            let (modified_headers, applied) =
                crate::syntax::auth::BearerAuthConfig::apply_from_variables(&variables, headers);

            if applied {
                return Ok((url, modified_headers));
            }

            Err(AuthError::new(format!(
                "OAuth2 Implicit auth '{}' requires interactive authentication. This feature is not yet implemented.",
                auth_config.name
            ))
            .into())
        })
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
    fn test_implicit_missing_fields() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        // Missing authorization_url

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("missing required field"));
    }

    #[test]
    fn test_implicit_unexpected_field() {
        let config = OAuth2ImplicitConfig::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert("unknown_field".to_string(), t("value"));

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unexpected field"));
    }
}
