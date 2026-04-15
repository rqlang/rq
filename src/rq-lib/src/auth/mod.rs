pub mod auth_executor;
pub mod bearer;
pub mod oauth2_authorization_code;
pub mod oauth2_client_credentials;
pub mod oauth2_implicit;

pub use auth_executor::{AuthExecutor, AuthFuture, ConfiguredRequest};
pub use bearer::BearerExecutor;
pub use oauth2_authorization_code::OAuth2AuthorizationCodeExecutor;
pub use oauth2_client_credentials::OAuth2ClientCredentialsExecutor;
pub use oauth2_implicit::OAuth2ImplicitExecutor;

pub use crate::syntax::auth::{AuthType, Config};

pub fn get_executor(auth_type: &AuthType) -> Box<dyn AuthExecutor> {
    match auth_type {
        AuthType::Bearer => Box::new(BearerExecutor::new()),
        AuthType::OAuth2AuthorizationCode => Box::new(OAuth2AuthorizationCodeExecutor::new()),
        AuthType::OAuth2ClientCredentials => Box::new(OAuth2ClientCredentialsExecutor::new()),
        AuthType::OAuth2Implicit => Box::new(OAuth2ImplicitExecutor::new()),
    }
}
