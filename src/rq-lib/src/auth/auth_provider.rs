use crate::syntax::auth::Config;
use crate::syntax::variable_context::VariableContext;
use std::future::Future;
use std::pin::Pin;

pub type ConfiguredRequest = (String, Vec<(String, String)>);

#[cfg(not(target_arch = "wasm32"))]
pub type AuthFuture<'a> = Pin<
    Box<dyn Future<Output = Result<ConfiguredRequest, Box<dyn std::error::Error>>> + Send + 'a>,
>;

#[cfg(target_arch = "wasm32")]
pub type AuthFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ConfiguredRequest, Box<dyn std::error::Error>>> + 'a>>;

pub trait AuthProvider: Send + Sync {
    #[allow(dead_code)]
    fn auth_type(&self) -> &str;

    fn configure<'a>(
        &'a self,
        auth_config: &'a Config,
        context: &'a VariableContext,
        url: String,
        headers: Vec<(String, String)>,
    ) -> AuthFuture<'a>;
}
