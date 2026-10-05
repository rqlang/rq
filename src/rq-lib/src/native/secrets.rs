use crate::logger::Logger;
use crate::syntax::secrets::{collect_secrets, parse_env_file, parse_os_vars, SecretProvider};
use crate::syntax::variable_context::Variable;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

lazy_static! {
    static ref LOGGED_SOURCES: Mutex<HashSet<(PathBuf, Option<String>)>> =
        Mutex::new(HashSet::new());
}

pub struct NativeSecretProvider;

impl SecretProvider for NativeSecretProvider {
    fn collect(&self, dir: &Path, selected_env: Option<&str>) -> Vec<Variable> {
        let env_file = dir.join(".env");
        let env_file_content = std::fs::read_to_string(&env_file).ok();
        let os_vars = std::env::vars().collect::<Vec<_>>();
        if Logger::is_debug_enabled() && first_time_logged(dir, selected_env) {
            log_secret_sources(
                &env_file,
                env_file_content.as_deref(),
                &os_vars,
                selected_env,
            );
        }
        collect_secrets(env_file_content.as_deref(), &os_vars, selected_env)
    }
}

fn first_time_logged(dir: &Path, selected_env: Option<&str>) -> bool {
    LOGGED_SOURCES.lock().is_ok_and(|mut logged| {
        logged.insert((dir.to_path_buf(), selected_env.map(str::to_string)))
    })
}

fn log_secret_sources(
    env_file: &Path,
    env_file_content: Option<&str>,
    os_vars: &[(String, String)],
    selected_env: Option<&str>,
) {
    let env_file = crate::paths::clean_path(env_file);
    match env_file_content {
        Some(content) => Logger::debug(&format!(
            "* Secrets from {env_file}: {}",
            variable_names(&parse_env_file(content, selected_env))
        )),
        None => Logger::debug(&format!("* No secrets file at {env_file}")),
    }
    Logger::debug(&format!(
        "* Secrets from RQ__ environment variables: {}",
        variable_names(&parse_os_vars(os_vars, selected_env))
    ));
}

fn variable_names(variables: &[Variable]) -> String {
    if variables.is_empty() {
        return "none".to_string();
    }
    variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
