use crate::syntax::secrets::{collect_secrets, SecretProvider};
use crate::syntax::variable_context::Variable;
use std::path::Path;

pub struct NativeSecretProvider;

impl SecretProvider for NativeSecretProvider {
    fn collect(&self, dir: &Path, selected_env: Option<&str>) -> Vec<Variable> {
        let env_file_content = std::fs::read_to_string(dir.join(".env")).ok();
        let os_vars = std::env::vars().collect::<Vec<_>>();
        collect_secrets(env_file_content.as_deref(), &os_vars, selected_env)
    }
}
