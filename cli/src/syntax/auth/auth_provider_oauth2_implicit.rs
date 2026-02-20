use crate::syntax::auth::{AuthFuture, AuthProvider};
use crate::syntax::error::{AuthError, SyntaxError};
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
const AUTHORIZATION_URL_FIELD: &str = "authorization_url";
const SCOPE_FIELD: &str = "scope";
const REDIRECT_URI_FIELD: &str = "redirect_uri";

const REQUIRED_FIELDS: [&str; 2] = [CLIENT_ID_FIELD, AUTHORIZATION_URL_FIELD];
const OPTIONAL_FIELDS: [&str; 2] = [SCOPE_FIELD, REDIRECT_URI_FIELD];

pub struct OAuth2ImplicitProvider;

impl OAuth2ImplicitProvider {
    pub fn new() -> Self {
        Self
    }
}

impl AuthProvider for OAuth2ImplicitProvider {
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
                    // We don't have a specific token for the missing field, so we use a default span or the span of the auth block if passed (but here we only have fields)
                    // The caller handles the position if we return a generic error, or we can use the first field's span if available.
                    // For now, using 0..0 as per existing pattern or minimal info.
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
            // Fallback to manually provided token in environment
            let (modified_headers, applied) =
                crate::syntax::auth::BearerAuthProvider::apply_from_variables(&variables, headers);

            if applied {
                return Ok((url, modified_headers));
            }

            // In the future, this is where we would trigger the interactive browser flow
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
        let config = OAuth2ImplicitProvider::new();
        assert_eq!(config.auth_type(), "oauth2_implicit");
    }

    #[test]
    fn test_valid_implicit_minimal() {
        let config = OAuth2ImplicitProvider::new();
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
        let config = OAuth2ImplicitProvider::new();
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
        let config = OAuth2ImplicitProvider::new();
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
        let config = OAuth2ImplicitProvider::new();
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
