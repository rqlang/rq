use super::{
    error::SyntaxError,
    functions::{self, traits::FunctionContext},
    parse_result::Request,
    parsers::utils::parse_system_function,
    reader::TokenReader,
    token::TokenType,
    tokenize::tokenize,
    variable_context::{VariableContext, VariableValue},
};
use lazy_static::lazy_static;
use std::io::BufRead;
use std::path::{Path, PathBuf};

lazy_static! {
    static ref FUNC_PATTERN: regex::Regex =
        regex::Regex::new(r"\{\{\$([a-zA-Z0-9_]+)\.([a-zA-Z0-9_]+)\x1E(.*?)\}\}").unwrap();
    static ref USER_FUNC_PATTERN: regex::Regex =
        regex::Regex::new(r"\{\{\s*([a-zA-Z0-9_]+)\.([a-zA-Z0-9_]+)\s*\((.*?)\)\s*\}\}").unwrap();
    static ref UNRESOLVED_PATTERN: regex::Regex =
        regex::Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}").unwrap();
}

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

pub fn find_all_variable_references(
    paths: &[PathBuf],
    var_name: &str,
) -> Vec<(usize, usize, PathBuf)> {
    let mut results = Vec::new();
    for path in paths {
        for (line, col) in find_all_var_refs_in_file(path, var_name) {
            results.push((line, col, path.clone()));
        }
    }
    results
}

pub fn find_all_endpoint_references(
    paths: &[PathBuf],
    ep_name: &str,
) -> Vec<(usize, usize, PathBuf)> {
    let mut results = Vec::new();
    for path in paths {
        for (line, col) in find_all_ep_refs_in_file(path, ep_name) {
            results.push((line, col, path.clone()));
        }
    }
    results
}

fn zero_based_line_col(content: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(content.len());
    let prefix = &content[..pos];
    let line = prefix.matches('\n').count();
    let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = prefix[last_line_start..].chars().count();
    (line, column)
}

fn is_in_env_block(tokens: &[super::token::Token], idx: usize) -> bool {
    use super::token::TokenType as TT;
    let mut depth = 0i32;
    let mut i = idx;
    loop {
        if i == 0 {
            return false;
        }
        i -= 1;
        match (&tokens[i].token_type, tokens[i].value.as_str()) {
            (TT::Punctuation, "}") => depth += 1,
            (TT::Punctuation, "{") => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    let mut k = i;
                    loop {
                        if k == 0 {
                            return false;
                        }
                        k -= 1;
                        match tokens[k].token_type {
                            TT::Whitespace | TT::Newline | TT::Comment => {}
                            TT::Identifier => break,
                            _ => return false,
                        }
                    }
                    loop {
                        if k == 0 {
                            return false;
                        }
                        k -= 1;
                        match tokens[k].token_type {
                            TT::Whitespace | TT::Newline | TT::Comment => {}
                            TT::Keyword => return tokens[k].value == "env",
                            _ => return false,
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn find_all_var_refs_in_file(path: &Path, var_name: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return results;
    };
    let Ok(tokens) = tokenize(&content) else {
        return results;
    };

    let escaped = regex::escape(var_name);
    let pattern_interp = format!(r"\{{\{{\s*{escaped}\s*\}}\}}");
    if let Ok(re) = regex::Regex::new(&pattern_interp) {
        for token in &tokens {
            if token.token_type == TokenType::String {
                let mut search_start = 0usize;
                while search_start < token.value.len() {
                    let Some(mat) = re.find(&token.value[search_start..]) else {
                        break;
                    };
                    let name_offset = mat.as_str().find(var_name).unwrap_or(0);
                    let abs_start = token.span.start + search_start + mat.start() + name_offset;
                    results.push(zero_based_line_col(&content, abs_start));
                    search_start += mat.start() + mat.len().max(1);
                }
            }
        }
    }

    for (i, token) in tokens.iter().enumerate() {
        if token.token_type == TokenType::Comment {
            continue;
        }
        if token.token_type == TokenType::Identifier && token.value == var_name {
            let mut is_key = false;
            for next in tokens.iter().skip(i + 1) {
                match next.token_type {
                    TokenType::Whitespace | TokenType::Newline | TokenType::Comment => continue,
                    TokenType::Punctuation => {
                        if next.value == ":" {
                            is_key = true;
                        }
                        break;
                    }
                    _ => break,
                }
            }
            if !is_key || is_in_env_block(&tokens, i) {
                results.push(zero_based_line_col(&content, token.span.start));
            }
        }
    }

    results.sort_unstable();
    results
}

fn find_all_ep_refs_in_file(path: &Path, ep_name: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return results;
    };
    let Ok(tokens) = tokenize(&content) else {
        return results;
    };

    for (i, token) in tokens.iter().enumerate() {
        if token.token_type != TokenType::Identifier || token.value != ep_name {
            continue;
        }
        let prev = tokens[..i].iter().rev().find(|t| {
            !matches!(
                t.token_type,
                TokenType::Whitespace | TokenType::Newline | TokenType::Comment
            )
        });
        let is_ref = prev.map(|t| t.value == "<").unwrap_or(false);
        let is_def = prev
            .map(|t| t.token_type == TokenType::Keyword && t.value == "ep")
            .unwrap_or(false);
        if is_ref || is_def {
            results.push(zero_based_line_col(&content, token.span.start));
        }
    }

    results.sort_unstable();
    results
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
    map: &std::collections::HashMap<&str, &VariableValue>,
    visited: &mut std::collections::HashSet<String>,
    source_files: &[PathBuf],
) -> ResolutionStatus {
    if visited.contains(var_name) {
        return ResolutionStatus::CircularReference;
    }
    visited.insert(var_name.to_string());
    if let Some(v) = map.get(var_name) {
        match v {
            VariableValue::String(s) => ResolutionStatus::Resolved(s.clone()),
            VariableValue::Array(_) => ResolutionStatus::Error(
                format!("Variable '{var_name}' is an array and cannot be used in a string context"),
                None,
            ),
            VariableValue::Json(j) => ResolutionStatus::Resolved(j.clone()),
            VariableValue::Reference(rn) => {
                let status = resolve_variable_value(rn, map, visited, source_files);
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
            VariableValue::Headers(_) => ResolutionStatus::Error(
                format!("Variable '{var_name}' is a headers object and cannot be used in a string context"),
                None,
            ),
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

fn resolve_vars_in_string(
    result: &mut String,
    map: &std::collections::HashMap<&str, &VariableValue>,
    source_files: &[PathBuf],
) -> Result<bool, SyntaxError> {
    let mut changed = false;
    for var_name in map.keys() {
        let patterns = [
            format!("{{{{{var_name}}}}}"),
            format!("{{{{ {var_name} }}}}"),
            format!("{{{{ {var_name}}}}}"),
            format!("{{{{{var_name} }}}}"),
        ];
        if !patterns.iter().any(|p| result.contains(p.as_str())) {
            continue;
        }
        let mut visited = std::collections::HashSet::new();
        match resolve_variable_value(var_name, map, &mut visited, source_files) {
            ResolutionStatus::Resolved(replacement) => {
                for pattern in &patterns {
                    if result.contains(pattern.as_str()) {
                        let new_res = result.replace(pattern.as_str(), &replacement);
                        if new_res != *result {
                            changed = true;
                            *result = new_res;
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
    Ok(changed)
}

fn try_resolve_system_func(
    result: &mut String,
    context: &VariableContext,
    source_files: &[PathBuf],
    func_pattern: &regex::Regex,
) -> Result<bool, SyntaxError> {
    if !result.contains("{{$") {
        return Ok(false);
    }
    let match_data = if let Some(caps) = func_pattern.captures(result) {
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
        let replacement = match execute_system_function(&full_func_name, &args, source_files) {
            Ok(res) => res,
            Err(msg) => {
                let (line, col, path) = find_sys_call_location(source_files, &full_func_name);
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
        return Ok(true);
    }
    Ok(false)
}

fn try_resolve_user_func(
    result: &mut String,
    context: &VariableContext,
    source_files: &[PathBuf],
    user_func_pattern: &regex::Regex,
) -> Result<bool, SyntaxError> {
    let Some(caps) = user_func_pattern.captures(result) else {
        return Ok(false);
    };
    let full_match = caps.get(0).unwrap();
    let range = full_match.range();
    let namespace = caps[1].to_string();
    let func_name = caps[2].to_string();
    let args_raw = caps[3].to_string();

    if !functions::is_known_namespace(&namespace) {
        return Ok(false);
    }

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
    reader.advance();
    reader.skip_ignorable();
    reader.advance();

    match parse_system_function(&mut reader, &namespace) {
        Ok(VariableValue::SystemFunction { name: _, args }) => {
            let mut resolved_args = Vec::new();
            for arg in args {
                resolved_args.push(resolve_string(&arg, context, source_files)?);
            }
            let full_func_name = format!("{namespace}.{func_name}");
            let replacement =
                match execute_system_function(&full_func_name, &resolved_args, source_files) {
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
            Ok(true)
        }
        Ok(_) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn resolve_string(
    input: &str,
    context: &VariableContext,
    source_files: &[PathBuf],
) -> Result<String, SyntaxError> {
    let map = context.as_map();
    let mut result = input.to_string();

    let mut iterations = 0;
    while iterations < 10 {
        iterations += 1;
        let mut changed = resolve_vars_in_string(&mut result, &map, source_files)?;
        changed |= try_resolve_system_func(&mut result, context, source_files, &FUNC_PATTERN)?;
        if !changed {
            changed =
                try_resolve_user_func(&mut result, context, source_files, &USER_FUNC_PATTERN)?;
        }
        if !changed {
            break;
        }
    }

    if let Some(caps) = UNRESOLVED_PATTERN.captures(&result) {
        let var_name = &caps[1];
        let msg = if iterations >= 10 {
            format!("Recursion limit exceeded resolving '{var_name}': check for deeply nested variable references")
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
    if let Some(auth) = &request.auth {
        request.auth = Some(resolve_string(auth, context, source_files)?);
    }
    Ok(request)
}

pub fn resolve_auth_provider(
    mut auth_config: super::auth::Config,
    context: &VariableContext,
    source_files: &[PathBuf],
) -> Result<super::auth::Config, SyntaxError> {
    for (_, token) in auth_config.fields.iter_mut() {
        token.value = resolve_string(&token.value, context, source_files)?;
    }
    Ok(auth_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::auth::{AuthType, Config};
    use crate::syntax::token::{Token, TokenType};
    use crate::syntax::variable_context::{Variable, VariableContext, VariableValue};

    fn make_context(vars: Vec<Variable>) -> VariableContext {
        VariableContext {
            file_variables: vars,
            environment_variables: Vec::new(),
            secret_variables: Vec::new(),
            endpoint_variables: Vec::new(),
            request_variables: Vec::new(),
            cli_variables: Vec::new(),
        }
    }
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
            line: 0,
            character: 0,
        };

        let env_vars = vec![Variable {
            name: "token_value".to_string(),
            value: VariableValue::String("secret-token-123".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

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
            line: 0,
            character: 0,
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

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

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
            line: 0,
            character: 0,
        };

        let env_vars = vec![Variable {
            name: "token_value".to_string(),
            value: VariableValue::String("spaced-token".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

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
            line: 0,
            character: 0,
        };

        let env_vars = vec![Variable {
            name: "unused_var".to_string(),
            value: VariableValue::String("unused-value".to_string()),
        }];

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

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
            line: 0,
            character: 0,
        };

        let env_vars = vec![Variable {
            name: "other_var".to_string(),
            value: VariableValue::String("other-value".to_string()),
        }];

        let result = resolve_auth_provider(config, &make_context(env_vars), &[]);
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
            line: 0,
            character: 0,
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

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

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
            line: 0,
            character: 0,
        };

        let env_vars: Vec<Variable> = vec![];

        let result = resolve_auth_provider(config, &make_context(env_vars), &[]);
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
            line: 0,
            character: 0,
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

        let resolved = resolve_auth_provider(config, &make_context(env_vars), &[]).unwrap();

        assert_eq!(
            resolved.fields.get("client_id").unwrap().value,
            "client-123"
        );
        assert_eq!(resolved.fields.get("token").unwrap().value, "secret-token");
    }

    fn var(name: &str, value: VariableValue) -> Variable {
        Variable {
            name: name.to_string(),
            value,
        }
    }

    #[test]
    fn test_headers_variable_in_string_context_errors() {
        let ctx = make_context(vec![var(
            "my_headers",
            VariableValue::Headers(vec![("X-Foo".to_string(), "bar".to_string())]),
        )]);
        let result = resolve_string("{{my_headers}}", &ctx, &[]);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("headers object") && msg.contains("string context"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_reference_chain_resolves_string() {
        let ctx = make_context(vec![
            var("a", VariableValue::String("hello".to_string())),
            var("b", VariableValue::Reference("a".to_string())),
            var("c", VariableValue::Reference("b".to_string())),
        ]);
        let result = resolve_string("{{c}}", &ctx, &[]).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_reference_to_headers_in_string_context_errors() {
        let ctx = make_context(vec![
            var(
                "base",
                VariableValue::Headers(vec![("X-H".to_string(), "v".to_string())]),
            ),
            var("h", VariableValue::Reference("base".to_string())),
        ]);
        let result = resolve_string("{{h}}", &ctx, &[]);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("headers object") && msg.contains("string context"),
            "unexpected error: {msg}"
        );
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
