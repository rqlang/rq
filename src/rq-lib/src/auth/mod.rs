pub mod auth_provider;
pub mod bearer;
pub mod oauth2_authorization_code;
pub mod oauth2_client_credentials;
pub mod oauth2_implicit;

pub use auth_provider::{AuthFuture, AuthProvider, ConfiguredRequest};
pub use bearer::BearerProvider;
pub use oauth2_authorization_code::OAuth2AuthorizationCodeProvider;
pub use oauth2_client_credentials::OAuth2ClientCredentialsProvider;
pub use oauth2_implicit::OAuth2ImplicitProvider;

pub use crate::syntax::auth::{AuthType, Config};

pub fn get_provider(auth_type: &AuthType) -> Box<dyn AuthProvider> {
    match auth_type {
        AuthType::Bearer => Box::new(BearerProvider::new()),
        AuthType::OAuth2AuthorizationCode => Box::new(OAuth2AuthorizationCodeProvider::new()),
        AuthType::OAuth2ClientCredentials => Box::new(OAuth2ClientCredentialsProvider::new()),
        AuthType::OAuth2Implicit => Box::new(OAuth2ImplicitProvider::new()),
    }
}
