use crate::syntax::parse_result::ParseResult;
use crate::syntax::variable_context::{Variable, VariableValue};
use std::collections::HashMap;
use std::path::Path;

pub trait SecretProvider: Send + Sync {
    fn collect(&self, dir: &Path, selected_env: Option<&str>) -> Vec<Variable>;
}

pub fn collect_all_variables(
    parse_result: &ParseResult,
    environment_name: Option<&str>,
    secret_provider: &dyn SecretProvider,
    source_path: &Path,
) -> Result<Vec<Variable>, Box<dyn std::error::Error>> {
    let mut all_variables = parse_result.file_variables.clone();

    if let Some(env_name) = environment_name {
        if let Some(env_vars) = parse_result.environments.get(env_name) {
            all_variables.extend(env_vars.clone());
        } else {
            return Err(format!("Environment '{env_name}' not found").into());
        }
    }

    let dir = if source_path.is_dir() {
        source_path.to_path_buf()
    } else if let Some(parent) = source_path.parent() {
        parent.to_path_buf()
    } else {
        return Ok(all_variables);
    };

    all_variables.extend(secret_provider.collect(&dir, environment_name));
    Ok(all_variables)
}

pub fn parse_env_file(content: &str, selected_env: Option<&str>) -> Vec<Variable> {
    let mut general: HashMap<String, String> = HashMap::new();
    let mut env_specific: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
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

    for (k, v) in env_specific {
        general.insert(k, v);
    }

    general
        .into_iter()
        .map(|(name, val)| Variable {
            name,
            value: VariableValue::String(val),
        })
        .collect()
}

pub fn parse_os_vars(vars: &[(String, String)], selected_env: Option<&str>) -> Vec<Variable> {
    let mut variables = Vec::new();

    for (k, v) in vars {
        let k_lower = k.to_lowercase();
        if let Some(stripped) = k_lower.strip_prefix("rq__env__") {
            if let Some(sel) = selected_env {
                if let Some(pos2) = stripped.find("__") {
                    let env_name = &stripped[..pos2];
                    let var_name = &stripped[pos2 + 2..];
                    if !env_name.is_empty()
                        && !var_name.is_empty()
                        && env_name.eq_ignore_ascii_case(sel)
                    {
                        variables.push(Variable {
                            name: var_name.to_string(),
                            value: VariableValue::String(v.clone()),
                        });
                    }
                }
            }
        } else if let Some(stripped) = k_lower.strip_prefix("rq__") {
            if !stripped.is_empty() {
                variables.push(Variable {
                    name: stripped.to_string(),
                    value: VariableValue::String(v.clone()),
                });
            }
        }
    }

    variables
}

pub fn collect_secrets(
    env_file_content: Option<&str>,
    os_vars: &[(String, String)],
    selected_env: Option<&str>,
) -> Vec<Variable> {
    let mut variables = Vec::new();
    if let Some(content) = env_file_content {
        variables.extend(parse_env_file(content, selected_env));
    }
    variables.extend(parse_os_vars(os_vars, selected_env));
    variables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_file_general() {
        let content = "API_KEY=secret\nOTHER=value\n";
        let vars = parse_env_file(content, None);
        assert!(
            vars.iter()
                .any(|v| v.name == "api_key"
                    && v.value == VariableValue::String("secret".to_string()))
        );
    }

    #[test]
    fn test_parse_env_file_env_specific() {
        let content = "API_KEY=general\nENV__LOCAL__API_KEY=local-secret\n";
        let vars = parse_env_file(content, Some("local"));
        assert!(vars
            .iter()
            .any(|v| v.name == "api_key"
                && v.value == VariableValue::String("local-secret".to_string())));
    }

    #[test]
    fn test_parse_env_file_env_specific_not_selected() {
        let content = "API_KEY=general\nENV__PROD__API_KEY=prod-secret\n";
        let vars = parse_env_file(content, Some("local"));
        assert!(vars.iter().any(
            |v| v.name == "api_key" && v.value == VariableValue::String("general".to_string())
        ));
        assert!(!vars.iter().any(|v| v.name == "prod__api_key"));
    }

    #[test]
    fn test_parse_env_file_ignores_comments_and_blanks() {
        let content = "# comment\n\nKEY=val\n";
        let vars = parse_env_file(content, None);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "key");
    }

    #[test]
    fn test_parse_os_vars_rq_prefix() {
        let vars = vec![
            ("RQ__API_KEY".to_string(), "mykey".to_string()),
            ("OTHER".to_string(), "ignored".to_string()),
        ];
        let result = parse_os_vars(&vars, None);
        assert!(result
            .iter()
            .any(|v| v.name == "api_key" && v.value == VariableValue::String("mykey".to_string())));
        assert!(!result.iter().any(|v| v.name == "other"));
    }

    #[test]
    fn test_parse_os_vars_env_specific() {
        let vars = vec![
            ("RQ__ENV__LOCAL__TOKEN".to_string(), "local-tok".to_string()),
            ("RQ__ENV__PROD__TOKEN".to_string(), "prod-tok".to_string()),
        ];
        let result = parse_os_vars(&vars, Some("local"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "token");
        assert_eq!(
            result[0].value,
            VariableValue::String("local-tok".to_string())
        );
    }

    #[test]
    fn test_collect_secrets_merges_both() {
        let env_content = "KEY=from_file\n";
        let os_vars = vec![("RQ__OTHER".to_string(), "from_os".to_string())];
        let result = collect_secrets(Some(env_content), &os_vars, None);
        assert!(result.iter().any(|v| v.name == "key"));
        assert!(result.iter().any(|v| v.name == "other"));
    }

    #[test]
    fn test_collect_secrets_none_env_file() {
        let os_vars = vec![("RQ__KEY".to_string(), "val".to_string())];
        let result = collect_secrets(None, &os_vars, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "key");
    }
}
