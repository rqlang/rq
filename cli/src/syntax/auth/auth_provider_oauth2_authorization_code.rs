use crate::syntax::auth::{AuthFuture, AuthProvider};
use crate::syntax::error::{AuthError, SyntaxError};
use crate::syntax::token::Token;
use std::collections::HashMap;

const CLIENT_ID_FIELD: &str = "client_id";
#[allow(dead_code)]
const CLIENT_SECRET_FIELD: &str = "client_secret";
const AUTHORIZATION_URL_FIELD: &str = "authorization_url";
const TOKEN_URL_FIELD: &str = "token_url";
#[allow(dead_code)]
const REDIRECT_URI_FIELD: &str = "redirect_uri";
#[allow(dead_code)]
const SCOPE_FIELD: &str = "scope";
const CODE_CHALLENGE_METHOD_FIELD: &str = "code_challenge_method";
const USE_STATE_FIELD: &str = "use_state";
const USE_PKCE_FIELD: &str = "use_pkce";

pub struct OAuth2AuthorizationCodeProvider;

impl OAuth2AuthorizationCodeProvider {
    pub fn new() -> Self {
        OAuth2AuthorizationCodeProvider
    }
}

impl AuthProvider for OAuth2AuthorizationCodeProvider {
    fn auth_type(&self) -> &str {
        "oauth2_authorization_code"
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        const REQUIRED_FIELDS: &[&str] =
            &[CLIENT_ID_FIELD, AUTHORIZATION_URL_FIELD, TOKEN_URL_FIELD];

        const OPTIONAL_FIELDS: &[&str] = &[
            CLIENT_SECRET_FIELD,
            REDIRECT_URI_FIELD,
            SCOPE_FIELD,
            CODE_CHALLENGE_METHOD_FIELD,
            USE_STATE_FIELD,
            USE_PKCE_FIELD,
        ];

        for field in REQUIRED_FIELDS {
            if !fields.contains_key(*field) {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Authorization Code auth '{name}' missing required field '{field}'"
                    ),
                    0,
                    0,
                    0..0,
                ));
            }
        }

        for field in REQUIRED_FIELDS {
            if let Some(token) = fields.get(*field) {
                if token.value.trim().is_empty() {
                    return Err(SyntaxError::new(
                        format!(
                            "OAuth2 Authorization Code auth '{name}' has empty '{field}' field"
                        ),
                        0,
                        0,
                        token.span.clone(),
                    ));
                }
            }
        }

        if let Some(token) = fields.get(CODE_CHALLENGE_METHOD_FIELD) {
            let valid_methods = ["S256", "plain"];
            if !valid_methods.contains(&token.value.as_str()) {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Authorization Code auth '{name}' has invalid '{CODE_CHALLENGE_METHOD_FIELD}': '{}'. Valid values: S256, plain",
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
                        "OAuth2 Authorization Code auth '{name}' has invalid '{USE_STATE_FIELD}': '{}'. Valid values: true, false",
                        token.value
                    ),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        if let Some(token) = fields.get(USE_PKCE_FIELD) {
            if token.value != "true" && token.value != "false" {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Authorization Code auth '{name}' has invalid '{USE_PKCE_FIELD}': '{}'. Valid values: true, false",
                        token.value
                    ),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        for (field_name, token) in fields {
            if !REQUIRED_FIELDS.contains(&field_name.as_str())
                && !OPTIONAL_FIELDS.contains(&field_name.as_str())
            {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Authorization Code auth '{name}' has unexpected field '{field_name}'. Expected fields: {}, {}",
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
                crate::syntax::auth::BearerAuthProvider::apply_from_variables(&variables, headers);

            if applied {
                return Ok((url, modified_headers));
            }

            Err(AuthError::new(format!(
                "OAuth2 Authorization Code auth '{}' requires interactive authentication. This feature is not yet implemented.",
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
    fn test_oauth2_auth_code_type() {
        let config = OAuth2AuthorizationCodeProvider::new();
        assert_eq!(config.auth_type(), "oauth2_authorization_code");
    }

    #[test]
    fn test_valid_oauth2_auth_code_minimal() {
        let config = OAuth2AuthorizationCodeProvider::new();
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

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_use_state() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert(USE_STATE_FIELD.to_string(), t("false"));

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_invalid_oauth2_auth_code_use_state() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert(USE_STATE_FIELD.to_string(), t("invalid"));

        assert!(config.validate("test_auth", &fields).is_err());
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_pkce() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        let config = OAuth2AuthorizationCodeProvider::new();
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

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_oauth2_auth_code_missing_client_id() {
        let config = OAuth2AuthorizationCodeProvider::new();
        let mut fields = HashMap::new();
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("missing required field 'client_id'"));
    }

    #[test]
    fn test_oauth2_auth_code_missing_authorization_url() {
        let config = OAuth2AuthorizationCodeProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            TOKEN_URL_FIELD.to_string(),
            t("https://auth.example.com/token"),
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("missing required field 'authorization_url'"));
    }

    #[test]
    fn test_oauth2_auth_code_missing_token_url() {
        let config = OAuth2AuthorizationCodeProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client-id"));
        fields.insert(
            AUTHORIZATION_URL_FIELD.to_string(),
            t("https://auth.example.com/authorize"),
        );

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("missing required field 'token_url'"));
    }

    #[test]
    fn test_oauth2_auth_code_empty_field() {
        let config = OAuth2AuthorizationCodeProvider::new();
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

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("has empty 'client_id' field"));
    }

    #[test]
    fn test_oauth2_auth_code_invalid_challenge_method() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert(CODE_CHALLENGE_METHOD_FIELD.to_string(), t("invalid"));

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("invalid 'code_challenge_method'"));
    }

    #[test]
    fn test_oauth2_auth_code_unexpected_field() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert("unexpected".to_string(), t("field"));

        let result = config.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("unexpected field 'unexpected'"));
    }

    #[tokio::test]
    async fn test_oauth2_configure_fallback_to_bearer_with_auth_token() {
        use crate::syntax::variable_context::{Variable, VariableValue};

        let provider = OAuth2AuthorizationCodeProvider::new();

        let auth_config = crate::syntax::auth::Config {
            name: "test_oauth".to_string(),
            auth_type: crate::syntax::auth::AuthType::OAuth2AuthorizationCode,
            fields: HashMap::new(),
            file_path: std::path::PathBuf::new(),
        };

        let context = crate::syntax::variable_context::VariableContext {
            file_variables: vec![Variable {
                name: "auth_token".to_string(),
                value: VariableValue::String("fallback-bearer-token".to_string()),
            }],
            environment_variables: vec![],
            secret_variables: vec![],
            endpoint_variables: vec![],
            request_variables: vec![],
            cli_variables: vec![],
        };

        let url = "https://api.example.com/data".to_string();
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];

        let result = provider
            .configure(&auth_config, &context, url.clone(), headers)
            .await;

        assert!(result.is_ok());
        let (returned_url, returned_headers) = result.unwrap();
        assert_eq!(returned_url, url);
        assert_eq!(returned_headers.len(), 2);
        assert_eq!(
            returned_headers[1],
            (
                "authorization".to_string(),
                "Bearer fallback-bearer-token".to_string()
            )
        );
    }

    #[tokio::test]
    async fn test_oauth2_configure_no_fallback_returns_error() {
        use crate::syntax::variable_context::{Variable, VariableValue};

        let config = OAuth2AuthorizationCodeProvider::new();

        let variables = vec![Variable {
            name: "other_var".to_string(),
            value: VariableValue::String("other-value".to_string()),
        }];

        let auth_config = crate::syntax::auth::Config {
            name: "test_oauth".to_string(),
            auth_type: crate::syntax::auth::AuthType::OAuth2AuthorizationCode,
            fields: HashMap::new(),
            file_path: std::path::PathBuf::new(),
        };

        let context = crate::syntax::variable_context::VariableContext {
            file_variables: variables,
            environment_variables: vec![],
            secret_variables: vec![],
            endpoint_variables: vec![],
            request_variables: vec![],
            cli_variables: vec![],
        };

        let url = "https://api.example.com/data".to_string();
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];

        let result = config.configure(&auth_config, &context, url, headers).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires interactive authentication"));
    }

    #[test]
    fn test_valid_oauth2_auth_code_with_use_pkce() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert(USE_PKCE_FIELD.to_string(), t("false"));

        assert!(config.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_invalid_oauth2_auth_code_use_pkce() {
        let config = OAuth2AuthorizationCodeProvider::new();
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
        fields.insert(USE_PKCE_FIELD.to_string(), t("invalid"));

        assert!(config.validate("test_auth", &fields).is_err());
    }
}
