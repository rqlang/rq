use super::auth_executor::{AuthExecutor, AuthFuture};
use super::bearer::BearerExecutor;
use crate::syntax::error::AuthError;

pub struct OAuth2ImplicitExecutor;

impl OAuth2ImplicitExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OAuth2ImplicitExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthExecutor for OAuth2ImplicitExecutor {
    fn auth_type(&self) -> &str {
        "oauth2_implicit"
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

    #[test]
    fn test_implicit_type() {
        let executor = OAuth2ImplicitExecutor::new();
        assert_eq!(executor.auth_type(), "oauth2_implicit");
    }
}
