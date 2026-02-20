use super::{
    error::SyntaxError,
    functions::{self, traits::FunctionContext},
    parse_result::Request,
    parsers::utils::parse_system_function,
    reader::TokenReader,
    token::TokenType,
    tokenize::tokenize,
    variable_context::{Variable, VariableContext, VariableValue},
};
use std::io::BufRead;
use std::path::{Path, PathBuf};

enum ResolutionStatus {
    Resolved(String),
    NotFound,
    CircularReference,
    Error(String, Option<(usize, usize, PathBuf)>),
}

fn find_in_file(path: &Path, var_name: &str) -> (usize, usize) {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(tokens) = tokenize(&content) {
            let get_line_col = |pos: usize| -> (usize, usize) {
                if pos > content.len() {
                    return (1, 1);
                }
                let prefix = &content[..pos];
                let line = prefix.matches('\n').count() + 1;
                let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
                let column = prefix[last_line_start..].chars().count() + 1;
                (line, column)
            };

            let escaped = regex::escape(var_name);
            let pattern_interp = format!(r"\{{\{{\s*{escaped}\s*\}}\}}");
            let re_interp = regex::Regex::new(&pattern_interp).ok();

            // First pass: look for interpolation in strings
            // This mimics the old behavior which prioritized interpolation over bare usage
            if let Some(re) = &re_interp {
                for token in &tokens {
                    if token.token_type == TokenType::String {
                        if let Some(mat) = re.find(&token.value) {
                            return get_line_col(token.span.start + mat.start());
                        }
                    }
                }
            }

            // Second pass: look for bare identifier
            for (i, token) in tokens.iter().enumerate() {
                match token.token_type {
                    TokenType::Comment => continue,
                    TokenType::Identifier => {
                        if token.value == var_name {
                            // Check if it's a key (followed by colon)
                            let mut is_key = false;
                            for next_token in tokens.iter().skip(i + 1) {
                                match next_token.token_type {
                                    TokenType::Whitespace
                                    | TokenType::Newline
                                    | TokenType::Comment => {
                                        continue;
                                    }
                                    TokenType::Punctuation => {
                                        if next_token.value == ":" {
                                            is_key = true;
                                        }
                                        break;
                                    }
                                    _ => break,
                                }
                            }

                            if !is_key {
                                return get_line_col(token.span.start);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (0, 0)
}

pub fn find_variable_location(paths: &[PathBuf], var_name: &str) -> (usize, usize, PathBuf) {
    for path in paths {
        let (line, col) = find_in_file(path, var_name);
        if line > 0 {
            return (line, col, path.clone());
        }
    }
    (0, 0, paths.first().cloned().unwrap_or_default())
}

fn find_sys_call_location(paths: &[PathBuf], full_func_name: &str) -> (usize, usize, PathBuf) {
    for path in paths {
        if let Ok(file) = std::fs::File::open(path) {
            let reader = std::io::BufReader::new(file);
            let escaped = regex::escape(full_func_name);
            let pattern = format!(r"({escaped}|\{{\{{\s*\${escaped})");
            if let Ok(re) = regex::Regex::new(&pattern) {
                for (index, line) in reader.lines().enumerate() {
                    if let Ok(l) = line {
                        if let Some(mat) = re.find(&l) {
                            return (index + 1, mat.start() + 1, path.clone());
                        }
                    }
                }
            }
        }
    }
    (0, 0, paths.first().cloned().unwrap_or_default())
}

fn format_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

fn execute_system_function(
    full_name: &str,
    args: &[String],
    source_files: &[PathBuf],
) -> Result<String, String> {
    let parts: Vec<&str> = full_name.split('.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid function name: {full_name}"));
    }
    let namespace = parts[0];
    let name = parts[1];

    if let Some(func) = functions::get_function(namespace, name) {
        let ctx = FunctionContext { source_files };
        func.execute(args, &ctx)
    } else {
        Err(format!("Unknown system function: {full_name}"))
    }
}

fn resolve_variable_value(
    var_name: &str,
    context: &VariableContext,
    visited: &mut std::collections::HashSet<String>,
    source_files: &[PathBuf],
) -> ResolutionStatus {
    if visited.contains(var_name) {
        return ResolutionStatus::CircularReference;
    }
    visited.insert(var_name.to_string());
    let all_vars: std::collections::HashMap<String, &VariableValue> = {
        let mut map = std::collections::HashMap::new();
        for var in &context.file_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.environment_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.secret_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.endpoint_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.request_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.cli_variables {
            map.insert(var.name.clone(), &var.value);
        }
        map
    };
    if let Some(v) = all_vars.get(var_name) {
        match v {
            VariableValue::String(s) => ResolutionStatus::Resolved(s.clone()),
            VariableValue::Array(arr) => ResolutionStatus::Resolved(arr.join(",")),
            VariableValue::Json(j) => ResolutionStatus::Resolved(j.clone()),
            VariableValue::Reference(rn) => {
                let status = resolve_variable_value(rn, context, visited, source_files);
                if let ResolutionStatus::NotFound = status {
                    let (line, col, path) = find_variable_location(source_files, rn);
                    let loc = if line > 0 {
                        Some((line, col, path))
                    } else {
                        None
                    };
                    ResolutionStatus::Error(format!("Unresolved variable: '{rn}'"), loc)
                } else {
                    status
                }
            }
            VariableValue::Headers(_hdrs) => ResolutionStatus::Resolved(String::new()), // headers not interpolated directly
            VariableValue::SystemFunction { name, args } => {
                match execute_system_function(name, args, source_files) {
                    Ok(res) => ResolutionStatus::Resolved(res),
                    Err(e) => ResolutionStatus::Error(e, None),
                }
            }
        }
    } else {
        ResolutionStatus::NotFound
    }
}

pub fn resolve_string(
    input: &str,
    context: &VariableContext,
    source_files: &[PathBuf],
) -> Result<String, SyntaxError> {
    let all_vars: std::collections::HashMap<String, &VariableValue> = {
        let mut map = std::collections::HashMap::new();
        for var in &context.file_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.environment_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.secret_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.endpoint_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.request_variables {
            map.insert(var.name.clone(), &var.value);
        }
        for var in &context.cli_variables {
            map.insert(var.name.clone(), &var.value);
        }
        map
    };
    let mut result = input.to_string();

    let mut iterations = 0;
    let func_pattern =
        regex::Regex::new(r"\{\{\$([a-zA-Z0-9_]+)\.([a-zA-Z0-9_]+)\x1E(.*?)\}\}").unwrap();

    // Regex for user-facing function syntax {{ namespace.func(...) }}
    let user_func_pattern =
        regex::Regex::new(r"\{\{\s*([a-zA-Z0-9_]+)\.([a-zA-Z0-9_]+)\s*\((.*?)\)\s*\}\}").unwrap();

    while iterations < 10 {
        iterations += 1;
        let mut changed = false;
        for var_name in all_vars.keys() {
            let patterns = [
                format!("{{{{{var_name}}}}}"),
                format!("{{{{ {var_name} }}}}"),
                format!("{{{{ {var_name}}}}}"),
                format!("{{{{{var_name} }}}}"),
            ];

            let is_used = patterns.iter().any(|p| result.contains(p));
            if !is_used {
                continue;
            }

            let mut visited = std::collections::HashSet::new();
            match resolve_variable_value(var_name, context, &mut visited, source_files) {
                ResolutionStatus::Resolved(replacement) => {
                    for pattern in patterns.iter() {
                        if result.contains(pattern) {
                            let new_res = result.replace(pattern, &replacement);
                            if new_res != result {
                                changed = true;
                                result = new_res;
                            }
                        }
                    }
                }
                ResolutionStatus::CircularReference => {
                    let (line, col, path) = find_variable_location(source_files, var_name);
                    return Err(SyntaxError::with_file(
                        format!("Circular reference detected for variable: '{var_name}'"),
                        line,
                        col,
                        0..0,
                        format_path(&path),
                    ));
                }
                ResolutionStatus::Error(msg, loc) => {
                    let (line, col, path) = if let Some((l, c, p)) = loc {
                        (l, c, p)
                    } else {
                        find_variable_location(source_files, var_name)
                    };
                    return Err(SyntaxError::with_file(
                        msg,
                        line,
                        col,
                        0..0,
                        format_path(&path),
                    ));
                }
                ResolutionStatus::NotFound => {}
            }
        }

        if result.contains("{{$") {
            let match_data = if let Some(caps) = func_pattern.captures(&result) {
                let full_match = caps.get(0).unwrap();
                let range = full_match.range();
                let namespace = caps[1].to_string();
                let func_name = caps[2].to_string();
                let args_str = caps[3].to_string();
                Some((range, namespace, func_name, args_str))
            } else {
                None
            };

            if let Some((range, namespace, func_name, args_str)) = match_data {
                let args: Vec<String> = if args_str.is_empty() {
                    Vec::new()
                } else {
                    let mut resolved_args = Vec::new();
                    for s in args_str.split('\x1F') {
                        resolved_args.push(resolve_string(s, context, source_files)?);
                    }
                    resolved_args
                };

                let full_func_name = format!("{namespace}.{func_name}");
                let replacement =
                    match execute_system_function(&full_func_name, &args, source_files) {
                        Ok(res) => res,
                        Err(msg) => {
                            let (line, col, path) =
                                find_sys_call_location(source_files, &full_func_name);
                            return Err(SyntaxError::with_file(
                                msg,
                                line,
                                col,
                                0..0,
                                format_path(&path),
                            ));
                        }
                    };

                result.replace_range(range, &replacement);
                changed = true;
            }
        }

        if !changed {
            // Also check for user-facing function syntax {{ namespace.func(...) }}
            if let Some(caps) = user_func_pattern.captures(&result) {
                let full_match = caps.get(0).unwrap();
                let range = full_match.range();
                let namespace = caps[1].to_string();
                let func_name = caps[2].to_string();
                let args_raw = caps[3].to_string();

                if functions::is_known_namespace(&namespace) {
                    // We need to parse the function call properly to handle arguments
                    // Construct a fake source string: namespace.func(args)
                    let fake_source = format!("{namespace}.{func_name}({args_raw})");
                    let tokens = tokenize(&fake_source).map_err(|e| {
                        SyntaxError::with_file(
                            format!("Failed to tokenize function call: {e:?}"),
                            0,
                            0,
                            0..0,
                            String::new(),
                        )
                    })?;
                    let mut reader = TokenReader::new(tokens, PathBuf::from(""), fake_source);
                    // consume namespace
                    reader.advance();
                    reader.skip_ignorable();
                    // consume dot
                    reader.advance();
                    // parse_system_function expects to be called AFTER the dot

                    match parse_system_function(&mut reader, &namespace) {
                        Ok(VariableValue::SystemFunction { name: _, args }) => {
                            // Resolve args recursively
                            let mut resolved_args = Vec::new();
                            for arg in args {
                                resolved_args.push(resolve_string(&arg, context, source_files)?);
                            }

                            let full_func_name = format!("{namespace}.{func_name}");
                            let replacement = match execute_system_function(
                                &full_func_name,
                                &resolved_args,
                                source_files,
                            ) {
                                Ok(res) => res,
                                Err(msg) => {
                                    let (line, col, path) =
                                        find_sys_call_location(source_files, &full_func_name);
                                    return Err(SyntaxError::with_file(
                                        msg,
                                        line,
                                        col,
                                        0..0,
                                        format_path(&path),
                                    ));
                                }
                            };
                            result.replace_range(range, &replacement);
                            changed = true;
                        }
                        Ok(_) => {} // Should not happen
                        Err(e) => {
                            // Only report error if we are sure it was meant to be a function call
                            // check if regex match is valid
                            return Err(e);
                        }
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }

    let unresolved_pattern = regex::Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}").unwrap();
    if let Some(caps) = unresolved_pattern.captures(&result) {
        let var_name = &caps[1];

        let msg = if iterations >= 10 {
            format!("Circular reference or recursion limit exceeded for variable: '{var_name}'")
        } else {
            format!("Unresolved variable: '{var_name}'")
        };

        let (line, col, path) = find_variable_location(source_files, var_name);

        return Err(SyntaxError::with_file(
            msg,
            line,
            col,
            0..0,
            format_path(&path),
        ));
    }

    Ok(result)
}

pub fn resolve_variables(
    mut request: Request,
    context: &VariableContext,
    source_files: &[PathBuf],
) -> Result<Request, SyntaxError> {
    request.url = resolve_string(&request.url, context, source_files)?;
    for (k, v) in &mut request.headers {
        *k = resolve_string(k, context, source_files)?;
        *v = resolve_string(v, context, source_files)?;
    }
    if let Some(body) = &request.body {
        request.body = Some(resolve_string(body, context, source_files)?);
    }
    if let Some(timeout) = &request.timeout {
        request.timeout = Some(resolve_string(timeout, context, source_files)?);
    }
    Ok(request)
}

pub fn resolve_auth_provider(
    mut auth_config: super::auth::Config,
    env_variables: &[Variable],
    source_files: &[PathBuf],
) -> Result<super::auth::Config, SyntaxError> {
    let context = VariableContext {
        file_variables: env_variables.to_vec(),
        environment_variables: Vec::new(),
        secret_variables: Vec::new(),
        endpoint_variables: Vec::new(),
        request_variables: Vec::new(),
        cli_variables: Vec::new(),
    };

    for (_, token) in auth_config.fields.iter_mut() {
        token.value = resolve_string(&token.value, &context, source_files)?;
    }

    Ok(auth_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::auth::{AuthType, Config};
    use crate::syntax::token::{Token, TokenType};
    use crate::syntax::variable_context::{Variable, VariableValue};
    use std::collections::HashMap;

    fn t(s: &str) -> Token {
        Token {
            token_type: TokenType::String,
            value: s.to_string(),
            span: 0..0,
        }
    }

    #[test]
    fn test_resolve_auth_provider_single_variable() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("{{token_value}}"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![Variable {
            name: "token_value".to_string(),
            value: VariableValue::String("secret-token-123".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(
            resolved.fields.get("token").unwrap().value,
            "secret-token-123"
        );
    }

    #[test]
    fn test_resolve_auth_provider_multiple_variables() {
        let mut fields = HashMap::new();
        fields.insert("client_id".to_string(), t("{{client_id}}"));
        fields.insert("client_secret".to_string(), t("{{client_secret}}"));
        fields.insert("authorization_url".to_string(), t("{{auth_url}}"));
        fields.insert("token_url".to_string(), t("{{token_url}}"));

        let config = Config {
            name: "oauth_auth".to_string(),
            auth_type: AuthType::OAuth2AuthorizationCode,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![
            Variable {
                name: "client_id".to_string(),
                value: VariableValue::String("my-client-id".to_string()),
            },
            Variable {
                name: "client_secret".to_string(),
                value: VariableValue::String("my-secret".to_string()),
            },
            Variable {
                name: "auth_url".to_string(),
                value: VariableValue::String("https://auth.example.com/authorize".to_string()),
            },
            Variable {
                name: "token_url".to_string(),
                value: VariableValue::String("https://auth.example.com/token".to_string()),
            },
        ];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(
            resolved.fields.get("client_id").unwrap().value,
            "my-client-id"
        );
        assert_eq!(
            resolved.fields.get("client_secret").unwrap().value,
            "my-secret"
        );
        assert_eq!(
            resolved.fields.get("authorization_url").unwrap().value,
            "https://auth.example.com/authorize"
        );
        assert_eq!(
            resolved.fields.get("token_url").unwrap().value,
            "https://auth.example.com/token"
        );
    }

    #[test]
    fn test_resolve_auth_provider_with_spaces() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("{{ token_value }}"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![Variable {
            name: "token_value".to_string(),
            value: VariableValue::String("spaced-token".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(resolved.fields.get("token").unwrap().value, "spaced-token");
    }

    #[test]
    fn test_resolve_auth_provider_no_variables() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("hardcoded-token"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![Variable {
            name: "unused_var".to_string(),
            value: VariableValue::String("unused-value".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(
            resolved.fields.get("token").unwrap().value,
            "hardcoded-token"
        );
    }

    #[test]
    fn test_resolve_auth_provider_unresolved_variable() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("{{missing_var}}"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![Variable {
            name: "other_var".to_string(),
            value: VariableValue::String("other-value".to_string()),
        }];

        let result = resolve_auth_provider(config, &env_vars, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_auth_provider_mixed_content() {
        let mut fields = HashMap::new();
        fields.insert(
            "url".to_string(),
            t("https://{{host}}/auth?client_id={{client_id}}"),
        );

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::OAuth2AuthorizationCode,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![
            Variable {
                name: "host".to_string(),
                value: VariableValue::String("auth.example.com".to_string()),
            },
            Variable {
                name: "client_id".to_string(),
                value: VariableValue::String("my-client".to_string()),
            },
        ];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(
            resolved.fields.get("url").unwrap().value,
            "https://auth.example.com/auth?client_id=my-client"
        );
    }

    #[test]
    fn test_resolve_auth_provider_empty_env() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("{{token_value}}"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars: Vec<Variable> = vec![];

        let result = resolve_auth_provider(config, &env_vars, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_auth_provider_bare_identifier() {
        let mut fields = HashMap::new();
        fields.insert("client_id".to_string(), t("{{my_client}}"));
        fields.insert("token".to_string(), t("{{my_token}}"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: std::path::PathBuf::new(),
        };

        let env_vars = vec![
            Variable {
                name: "my_client".to_string(),
                value: VariableValue::String("client-123".to_string()),
            },
            Variable {
                name: "my_token".to_string(),
                value: VariableValue::String("secret-token".to_string()),
            },
        ];

        let resolved = resolve_auth_provider(config, &env_vars, &[]).unwrap();

        assert_eq!(
            resolved.fields.get("client_id").unwrap().value,
            "client-123"
        );
        assert_eq!(resolved.fields.get("token").unwrap().value, "secret-token");
    }

    #[test]
    fn test_format_path_windows() {
        let path = Path::new(r"\\?\C:\foo\bar");
        assert_eq!(format_path(path), r"C:\foo\bar");
    }

    #[test]
    fn test_format_path_unix() {
        let path = Path::new("/foo/bar");
        assert_eq!(format_path(path), "/foo/bar");
    }

    #[test]
    fn test_find_variable_location_with_spaces() {
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_vars.rq");
        std::fs::write(&file_path, "line1\nline2 {{  my_var  }}\nline3").unwrap();

        let (line, col, _) = find_variable_location(std::slice::from_ref(&file_path), "my_var");
        assert_eq!(line, 2);
        // "line2 {{  my_var  }}"
        //  012345678901
        // It starts at index 10 (0-based) -> column 11 (1-based)
        assert_eq!(col, 11);

        std::fs::remove_file(file_path).unwrap();
    }
}
