use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;

pub trait AuthConfig: Send + Sync {
    #[allow(dead_code)]
    fn auth_type(&self) -> &str;

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError>;
}
