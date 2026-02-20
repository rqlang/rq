use crate::syntax::auth::{AuthFuture, AuthProvider};
use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use crate::syntax::variable_context::Variable;
use std::collections::HashMap;

const TOKEN_FIELD: &str = "token";

pub struct BearerAuthProvider;

impl BearerAuthProvider {
    pub fn new() -> Self {
        BearerAuthProvider
    }

    pub fn apply_from_variables(
        variables: &[Variable],
        mut headers: Vec<(String, String)>,
    ) -> (Vec<(String, String)>, bool) {
        if let Some(token) = Self::find_auth_token(variables) {
            Self::add_bearer_header(&mut headers, &token);
            (headers, true)
        } else {
            (headers, false)
        }
    }

    fn find_auth_token(variables: &[Variable]) -> Option<String> {
        for var in variables {
            if var.name.eq_ignore_ascii_case("auth_token") {
                if let crate::syntax::variable_context::VariableValue::String(token) = &var.value {
                    return Some(token.clone());
                }
            }
        }
        None
    }

    fn add_bearer_header(headers: &mut Vec<(String, String)>, token: &str) {
        headers.push((
            reqwest::header::AUTHORIZATION.as_str().to_string(),
            format!("Bearer {token}"),
        ));
    }
}

impl AuthProvider for BearerAuthProvider {
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

    fn configure<'a>(
        &'a self,
        auth_config: &'a crate::syntax::auth::Config,
        _context: &'a crate::syntax::variable_context::VariableContext,
        url: String,
        mut headers: Vec<(String, String)>,
    ) -> AuthFuture<'a> {
        Box::pin(async move {
            let token = &auth_config.fields[TOKEN_FIELD].value;
            Self::add_bearer_header(&mut headers, token);
            Ok((url, headers))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_auth_type() {
        let config = BearerAuthProvider::new();
        assert_eq!(config.auth_type(), "bearer");
    }

    #[test]
    fn test_valid_bearer_auth() {
        let config = BearerAuthProvider::new();
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
        let config = BearerAuthProvider::new();
        let fields = HashMap::new();

        let result = config.validate("test_auth", &fields);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bearer_auth_empty_token() {
        let config = BearerAuthProvider::new();
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
        let config = BearerAuthProvider::new();
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

    #[test]
    fn test_apply_from_variables_with_auth_token() {
        use crate::syntax::variable_context::{Variable, VariableValue};

        let variables = vec![
            Variable {
                name: "auth_token".to_string(),
                value: VariableValue::String("test-token-123".to_string()),
            },
            Variable {
                name: "other_var".to_string(),
                value: VariableValue::String("other-value".to_string()),
            },
        ];

        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];

        let (result, applied) = BearerAuthProvider::apply_from_variables(&variables, headers);

        assert!(applied);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            ("Content-Type".to_string(), "application/json".to_string())
        );
        assert_eq!(
            result[1],
            (
                "authorization".to_string(),
                "Bearer test-token-123".to_string()
            )
        );
    }

    #[test]
    fn test_apply_from_variables_without_auth_token() {
        use crate::syntax::variable_context::{Variable, VariableValue};

        let variables = vec![Variable {
            name: "other_var".to_string(),
            value: VariableValue::String("other-value".to_string()),
        }];

        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];

        let (result, applied) = BearerAuthProvider::apply_from_variables(&variables, headers);

        assert!(!applied);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            ("Content-Type".to_string(), "application/json".to_string())
        );
    }

    #[test]
    fn test_apply_from_variables_empty_headers() {
        use crate::syntax::variable_context::{Variable, VariableValue};

        let variables = vec![Variable {
            name: "auth_token".to_string(),
            value: VariableValue::String("my-bearer-token".to_string()),
        }];

        let headers = vec![];

        let (result, applied) = BearerAuthProvider::apply_from_variables(&variables, headers);

        assert!(applied);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            (
                "authorization".to_string(),
                "Bearer my-bearer-token".to_string()
            )
        );
    }
}
