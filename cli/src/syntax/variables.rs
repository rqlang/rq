use crate::syntax::{Variable, VariableValue};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn load_all_variables(
    file_path: &Path,
    source_path: &Path,
    environment_name: Option<&str>,
) -> Result<(Vec<Variable>, Vec<PathBuf>), Box<dyn std::error::Error>> {
    let mut all_variables = Vec::new();

    let content = fs::read_to_string(file_path)?;
    let tokens = crate::syntax::tokenize(&content).map_err(|mut e| {
        e.file_path = Some(file_path.to_string_lossy().to_string());
        e
    })?;
    let parse_result = crate::syntax::analyze(&tokens, file_path.to_path_buf(), &content)?;

    all_variables.extend(parse_result.file_variables.clone());

    if let Some(env_name) = environment_name {
        if let Some(env_vars) = parse_result.environments.get(env_name) {
            all_variables.extend(env_vars.clone());
        } else {
            return Err(format!("Environment '{env_name}' not found").into());
        }
    }

    let secret_vars = load_secrets(source_path, environment_name)?;
    all_variables.extend(secret_vars);

    Ok((all_variables, parse_result.imported_files))
}

pub fn load_secrets(
    source_path: &Path,
    environment_name: Option<&str>,
) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    let mut variables = Vec::new();

    let dir = if source_path.is_dir() {
        source_path
    } else if let Some(parent) = source_path.parent() {
        parent
    } else {
        return Ok(variables); // No directory to search
    };

    let env_file = dir.join(".env");
    if env_file.exists() {
        let env_vars = load_env_file(&env_file, environment_name)?;
        variables.extend(env_vars);
    }

    let os_vars = load_os_env_variables(environment_name);
    variables.extend(os_vars);

    Ok(variables)
}

fn load_env_file(
    env_file: &Path,
    selected_env: Option<&str>,
) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    let env_content = fs::read_to_string(env_file)?;
    let mut general: HashMap<String, String> = HashMap::new();
    let mut env_specific: HashMap<String, String> = HashMap::new();

    for line in env_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let key_raw = trimmed[..eq].trim();
            let key_lower = key_raw.to_lowercase();
            let value = trimmed[eq + 1..].trim().trim_matches('"').to_string();
            if let Some(rest) = key_lower.strip_prefix("env__") {
                if let Some(sel) = selected_env {
                    if let Some(pos2) = rest.find("__") {
                        let env_name = &rest[..pos2];
                        let var_name = &rest[pos2 + 2..];
                        if !env_name.is_empty()
                            && !var_name.is_empty()
                            && env_name.eq_ignore_ascii_case(sel)
                        {
                            env_specific.insert(var_name.to_string(), value);
                        }
                    }
                }
            } else if !key_raw.is_empty() {
                general.insert(key_lower, value);
            }
        }
    }

    for (k, v) in env_specific.into_iter() {
        general.insert(k, v);
    }

    let variables: Vec<Variable> = general
        .into_iter()
        .map(|(name, val)| Variable {
            name,
            value: VariableValue::String(val),
        })
        .collect();

    Ok(variables)
}

fn load_os_env_variables(selected_env: Option<&str>) -> Vec<Variable> {
    let mut variables = Vec::new();

    for (k, v) in std::env::vars() {
        let k_lower = k.to_lowercase();
        if let Some(stripped) = k_lower.strip_prefix("rq__env__") {
            if let Some(sel) = selected_env {
                let rest = stripped;
                if let Some(pos2) = rest.find("__") {
                    let env_name = &rest[..pos2];
                    let var_name = &rest[pos2 + 2..];
                    if !env_name.is_empty()
                        && !var_name.is_empty()
                        && env_name.eq_ignore_ascii_case(sel)
                    {
                        variables.push(Variable {
                            name: var_name.to_string(),
                            value: VariableValue::String(v),
                        });
                    }
                }
            }
        } else if let Some(stripped) = k_lower.strip_prefix("rq__") {
            if !stripped.is_empty() {
                variables.push(Variable {
                    name: stripped.to_string(),
                    value: VariableValue::String(v),
                });
            }
        }
    }

    variables
}
