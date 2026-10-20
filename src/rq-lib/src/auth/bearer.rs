use super::auth_provider::{AuthFuture, AuthProvider};
use crate::syntax::auth::Config;
use crate::syntax::error::AuthError;
use crate::syntax::variable_context::{Variable, VariableValue};

const TOKEN_FIELD: &str = "token";

pub struct BearerProvider;

impl BearerProvider {
    pub fn new() -> Self {
        BearerProvider
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

    pub fn apply_configured_token(
        auth_config: &Config,
        mut headers: Vec<(String, String)>,
    ) -> (Vec<(String, String)>, bool) {
        match auth_config.fields.get(TOKEN_FIELD) {
            Some(token) => {
                Self::add_bearer_header(&mut headers, &token.value);
                (headers, true)
            }
            None => (headers, false),
        }
    }

    pub fn add_bearer_header(headers: &mut Vec<(String, String)>, token: &str) {
        let auth_key = reqwest::header::AUTHORIZATION.as_str();
        headers.retain(|(k, _)| !k.eq_ignore_ascii_case(auth_key));
        headers.push((auth_key.to_string(), format!("Bearer {token}")));
    }

    fn find_auth_token(variables: &[Variable]) -> Option<String> {
        for var in variables.iter().rev() {
            if var.name.eq_ignore_ascii_case("auth_token") {
                if let VariableValue::String(token) = &var.value {
                    return Some(token.clone());
                }
            }
        }
        None
    }
}

impl Default for BearerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthProvider for BearerProvider {
    fn auth_type(&self) -> &str {
        "bearer"
    }

    fn configure<'a>(
        &'a self,
        auth_config: &'a crate::syntax::auth::Config,
        _context: &'a crate::syntax::variable_context::VariableContext,
        url: String,
        headers: Vec<(String, String)>,
    ) -> AuthFuture<'a> {
        Box::pin(async move {
            match Self::apply_configured_token(auth_config, headers) {
                (headers, true) => Ok((url, headers)),
                _ => Err(AuthError::new(format!(
                    "Bearer auth '{}' has no '{TOKEN_FIELD}' field",
                    auth_config.name
                ))
                .into()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::variable_context::Variable;

    #[test]
    fn test_bearer_type() {
        let executor = BearerProvider::new();
        assert_eq!(executor.auth_type(), "bearer");
    }

    #[test]
    fn test_apply_from_variables_with_auth_token() {
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
        let (result, applied) = BearerProvider::apply_from_variables(&variables, headers);
        assert!(applied);
        assert_eq!(result.len(), 2);
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
        let variables = vec![Variable {
            name: "other_var".to_string(),
            value: VariableValue::String("other-value".to_string()),
        }];
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        let (result, applied) = BearerProvider::apply_from_variables(&variables, headers);
        assert!(!applied);
        assert_eq!(result.len(), 1);
    }

    fn auth_token(value: &str) -> Variable {
        Variable {
            name: "auth_token".to_string(),
            value: VariableValue::String(value.to_string()),
        }
    }

    #[test]
    fn the_highest_precedence_auth_token_wins() {
        let variables = vec![auth_token("from-let"), auth_token("from-runtime")];
        let (target, _) = BearerProvider::apply_from_variables(&variables, Vec::new());
        assert_eq!(
            target,
            vec![(
                "authorization".to_string(),
                "Bearer from-runtime".to_string()
            )]
        );
    }

    #[test]
    fn a_config_without_token_field_is_not_applied() {
        let config = Config {
            name: "token_auth".to_string(),
            auth_type: crate::syntax::auth::AuthType::Bearer,
            fields: std::collections::HashMap::new(),
            file_path: std::path::PathBuf::new(),
            line: 0,
            character: 0,
        };
        let (_, applied) = BearerProvider::apply_configured_token(&config, Vec::new());
        assert!(!applied);
    }
}
