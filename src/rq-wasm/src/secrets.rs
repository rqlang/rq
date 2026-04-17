use rq_lib::syntax::secrets::{collect_secrets, SecretProvider};
use rq_lib::syntax::variable_context::Variable;
use std::path::Path;

pub struct WasmSecretProvider {
    env_file_content: Option<String>,
    os_vars: Vec<(String, String)>,
}

impl WasmSecretProvider {
    pub fn new(env_file_content: Option<String>, os_vars: Vec<(String, String)>) -> Self {
        Self {
            env_file_content,
            os_vars,
        }
    }
}

impl SecretProvider for WasmSecretProvider {
    fn collect(&self, _dir: &Path, selected_env: Option<&str>) -> Vec<Variable> {
        collect_secrets(
            self.env_file_content.as_deref(),
            &self.os_vars,
            selected_env,
        )
    }
}
