use super::auth_executor::{AuthExecutor, AuthFuture};
use super::bearer::BearerExecutor;
use crate::syntax::error::AuthError;

pub struct OAuth2AuthorizationCodeExecutor;

impl OAuth2AuthorizationCodeExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OAuth2AuthorizationCodeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthExecutor for OAuth2AuthorizationCodeExecutor {
    fn auth_type(&self) -> &str {
        "oauth2_authorization_code"
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
                BearerExecutor::apply_from_variables(&variables, headers);

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
    use crate::syntax::variable_context::{Variable, VariableContext, VariableValue};

    #[tokio::test]
    async fn test_configure_fallback_to_bearer_with_auth_token() {
        let executor = OAuth2AuthorizationCodeExecutor::new();
        let auth_config = crate::syntax::auth::Config {
            name: "test_oauth".to_string(),
            auth_type: crate::syntax::auth::AuthType::OAuth2AuthorizationCode,
            fields: std::collections::HashMap::new(),
            file_path: std::path::PathBuf::new(),
            line: 0,
            character: 0,
        };
        let context = VariableContext {
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
        let result = executor
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
    async fn test_configure_no_fallback_returns_error() {
        let executor = OAuth2AuthorizationCodeExecutor::new();
        let auth_config = crate::syntax::auth::Config {
            name: "test_oauth".to_string(),
            auth_type: crate::syntax::auth::AuthType::OAuth2AuthorizationCode,
            fields: std::collections::HashMap::new(),
            file_path: std::path::PathBuf::new(),
            line: 0,
            character: 0,
        };
        let context = VariableContext {
            file_variables: vec![Variable {
                name: "other_var".to_string(),
                value: VariableValue::String("other-value".to_string()),
            }],
            environment_variables: vec![],
            secret_variables: vec![],
            endpoint_variables: vec![],
            request_variables: vec![],
            cli_variables: vec![],
        };
        let url = "https://api.example.com/data".to_string();
        let headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        let result = executor
            .configure(&auth_config, &context, url, headers)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires interactive authentication"));
    }
}
