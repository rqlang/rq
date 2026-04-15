use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use crate::syntax::variable_context::VariableContext;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub type ConfiguredRequest = (String, Vec<(String, String)>);

pub type AuthFuture<'a> = Pin<
    Box<dyn Future<Output = Result<ConfiguredRequest, Box<dyn std::error::Error>>> + Send + 'a>,
>;

pub trait AuthProvider: Send + Sync {
    #[allow(dead_code)]
    fn auth_type(&self) -> &str;

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError>;

    fn configure<'a>(
        &'a self,
        auth_config: &'a super::Config,
        context: &'a VariableContext,
        url: String,
        headers: Vec<(String, String)>,
    ) -> AuthFuture<'a>;
}
