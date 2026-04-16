use super::auth_provider::{AuthFuture, AuthProvider};
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

    fn find_auth_token(variables: &[Variable]) -> Option<String> {
        for var in variables {
            if var.name.eq_ignore_ascii_case("auth_token") {
                if let VariableValue::String(token) = &var.value {
                    return Some(token.clone());
                }
            }
        }
        None
    }

    pub fn add_bearer_header(headers: &mut Vec<(String, String)>, token: &str) {
        headers.push((
            reqwest::header::AUTHORIZATION.as_str().to_string(),
            format!("Bearer {token}"),
        ));
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
}
