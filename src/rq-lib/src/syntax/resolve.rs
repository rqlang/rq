use super::{
    error::SyntaxError,
    fs::Fs,
    functions::{self, traits::FunctionContext},
    parse_result::Request,
    parsers::utils::parse_system_function,
    reader::TokenReader,
    token::TokenType,
    tokenizer::tokenize,
    variable_context::{VariableContext, VariableValue},
};
use lazy_static::lazy_static;
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

fn find_in_file(fs: &dyn Fs, path: &Path, var_name: &str) -> (usize, usize) {
    let Ok(content) = fs.read(path) else {
        return (0, 0);
    };
    let Ok(tokens) = tokenize(&content) else {
        return (0, 0);
    };

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

    if let Some(re) = &re_interp {
        for token in &tokens {
            if token.token_type == TokenType::String {
                if let Some(mat) = re.find(&token.value) {
                    return get_line_col(token.span.start + mat.start());
                }
            }
        }
    }

    for (i, token) in tokens.iter().enumerate() {
        match token.token_type {
            TokenType::Comment => continue,
            TokenType::Identifier if token.value == var_name => {
                let mut is_key = false;
                for next_token in tokens.iter().skip(i + 1) {
                    match next_token.token_type {
                        TokenType::Whitespace | TokenType::Newline | TokenType::Comment => {
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
            _ => {}
        }
    }
    (0, 0)
}

pub fn find_variable_location(
    fs: &dyn Fs,
    paths: &[PathBuf],
    var_name: &str,
) -> (usize, usize, PathBuf) {
    for path in paths {
        let (line, col) = find_in_file(fs, path, var_name);
        if line > 0 {
            return (line, col, path.clone());
        }
    }
    if let Some(ext_path) = find_variable_in_io_files(fs, paths, var_name) {
        return (0, 0, ext_path);
    }
    (0, 0, paths.first().cloned().unwrap_or_default())
}

fn find_variable_in_io_files(
    fs: &dyn Fs,
    source_files: &[PathBuf],
    var_name: &str,
) -> Option<PathBuf> {
    let re = regex::Regex::new(r#"io\.read_file\(\s*"([^"]+)"\s*\)"#).ok()?;
    let var_re = regex::Regex::new(&format!(r"\{{\{{\s*{var_name}\s*\}}\}}")).ok()?;
    for source in source_files {
        let Ok(content) = fs.read(source) else {
            continue;
        };
        let parent = source.parent().unwrap_or(Path::new("."));
        for caps in re.captures_iter(&content) {
            let ext_path = parent.join(&caps[1]);
            if let Ok(ext_content) = fs.read(&ext_path) {
                if var_re.is_match(&ext_content) {
                    return Some(ext_path);
                }
            }
        }
    }
    None
}

pub fn find_all_variable_references(
    fs: &dyn Fs,
    paths: &[PathBuf],
    var_name: &str,
) -> Vec<(usize, usize, PathBuf)> {
    let mut results = Vec::new();
    for path in paths {
        for (line, col) in find_all_var_refs_in_file(fs, path, var_name) {
            results.push((line, col, path.clone()));
        }
    }
    results
}

pub fn find_all_endpoint_references(
    fs: &dyn Fs,
    paths: &[PathBuf],
    ep_name: &str,
) -> Vec<(usize, usize, PathBuf)> {
    let mut results = Vec::new();
    for path in paths {
        for (line, col) in find_all_ep_refs_in_file(fs, path, ep_name) {
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

fn find_all_var_refs_in_file(fs: &dyn Fs, path: &Path, var_name: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let Ok(content) = fs.read(path) else {
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

fn find_all_ep_refs_in_file(fs: &dyn Fs, path: &Path, ep_name: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let Ok(content) = fs.read(path) else {
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

fn find_sys_call_location(
    fs: &dyn Fs,
    paths: &[PathBuf],
    full_func_name: &str,
    args: &[String],
) -> (usize, usize, PathBuf) {
    let escaped = regex::escape(full_func_name);
    let pattern = if let Some(first_arg) = args.first() {
        let escaped_arg = regex::escape(first_arg);
        format!(r#"{escaped}\s*\(\s*["']{escaped_arg}["']"#)
    } else {
        format!(r"({escaped}|\{{\{{\s*\${escaped})")
    };
    if let Ok(re) = regex::Regex::new(&pattern) {
        for path in paths {
            if let Ok(content) = fs.read(path) {
                for (index, line) in content.lines().enumerate() {
                    if let Some(mat) = re.find(line) {
                        return (index + 1, mat.start() + 1, path.clone());
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
    fs: &dyn Fs,
) -> Result<String, String> {
    let parts: Vec<&str> = full_name.split('.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid function name: {full_name}"));
    }
    let namespace = parts[0];
    let name = parts[1];

    if let Some(func) = functions::get_function(namespace, name) {
        let ctx = FunctionContext { source_files, fs };
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
    dry_run: bool,
    fs: &dyn Fs,
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
                let status = resolve_variable_value(rn, map, visited, source_files, dry_run, fs);
                if let ResolutionStatus::NotFound = status {
                    let (line, col, path) = find_variable_location(fs, source_files, rn);
                    let loc = if line > 0 { Some((line, col, path)) } else { None };
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
                if dry_run {
                    ResolutionStatus::Resolved(String::new())
                } else {
                    match execute_system_function(name, args, source_files, fs) {
                        Ok(res) => ResolutionStatus::Resolved(res),
                        Err(e) => ResolutionStatus::Error(e, None),
                    }
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
    dry_run: bool,
    fs: &dyn Fs,
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
        match resolve_variable_value(var_name, map, &mut visited, source_files, dry_run, fs) {
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
                let (line, col, path) = find_variable_location(fs, source_files, var_name);
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
                    find_variable_location(fs, source_files, var_name)
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

fn validate_system_func(
    result: &mut String,
    func_pattern: &regex::Regex,
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Result<bool, SyntaxError> {
    if !result.contains("{{$") {
        return Ok(false);
    }
    let match_data = if let Some(caps) = func_pattern.captures(result) {
        let full_match = caps.get(0).unwrap();
        let range = full_match.range();
        let namespace = caps[1].to_string();
        let func_name = caps[2].to_string();
        Some((range, namespace, func_name))
    } else {
        None
    };
    if let Some((range, namespace, func_name)) = match_data {
        let full_func_name = format!("{namespace}.{func_name}");
        if functions::get_function(&namespace, &func_name).is_none() {
            let (line, col, path) = find_sys_call_location(fs, source_files, &full_func_name, &[]);
            return Err(SyntaxError::with_file(
                format!("Unknown function: {full_func_name}"),
                line,
                col,
                0..0,
                format_path(&path),
            ));
        }
        result.replace_range(range, "");
        return Ok(true);
    }
    Ok(false)
}

fn try_resolve_system_func(
    result: &mut String,
    context: &VariableContext,
    source_files: &[PathBuf],
    func_pattern: &regex::Regex,
    fs: &dyn Fs,
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
                resolved_args.push(resolve_string(s, context, source_files, fs)?);
            }
            resolved_args
        };
        let full_func_name = format!("{namespace}.{func_name}");
        let replacement = match execute_system_function(&full_func_name, &args, source_files, fs) {
            Ok(res) => res,
            Err(msg) => {
                let (line, col, path) =
                    find_sys_call_location(fs, source_files, &full_func_name, &args);
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

fn validate_user_func(
    result: &mut String,
    context: &VariableContext,
    source_files: &[PathBuf],
    user_func_pattern: &regex::Regex,
    fs: &dyn Fs,
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
    let mut token_reader = TokenReader::new(tokens, PathBuf::from(""), fake_source);
    token_reader.advance();
    token_reader.skip_ignorable();
    token_reader.advance();

    match parse_system_function(&mut token_reader, &namespace) {
        Ok(VariableValue::SystemFunction { name: _, args }) => {
            let full_func_name = format!("{namespace}.{func_name}");
            let parts: Vec<&str> = full_func_name.split('.').collect();
            if parts.len() == 2 {
                if let Some(func) = functions::get_function(parts[0], parts[1]) {
                    let resolved_args: Vec<String> = args
                        .iter()
                        .map(|a| check_string(a, context, source_files, fs).unwrap_or_default())
                        .collect();
                    if let Err(msg) = func.validate_args(&resolved_args) {
                        let (line, col, path) = find_sys_call_location(
                            fs,
                            source_files,
                            &full_func_name,
                            &resolved_args,
                        );
                        return Err(SyntaxError::with_file(
                            msg,
                            line,
                            col,
                            0..0,
                            format_path(&path),
                        ));
                    }
                }
            }
            result.replace_range(range, "");
            Ok(true)
        }
        Ok(_) => Ok(false),
        Err(e) => Err(e),
    }
}

fn try_resolve_user_func(
    result: &mut String,
    context: &VariableContext,
    source_files: &[PathBuf],
    user_func_pattern: &regex::Regex,
    fs: &dyn Fs,
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
    let mut token_reader = TokenReader::new(tokens, PathBuf::from(""), fake_source);
    token_reader.advance();
    token_reader.skip_ignorable();
    token_reader.advance();

    match parse_system_function(&mut token_reader, &namespace) {
        Ok(VariableValue::SystemFunction { name: _, args }) => {
            let mut resolved_args = Vec::new();
            for arg in args {
                resolved_args.push(resolve_string(&arg, context, source_files, fs)?);
            }
            let full_func_name = format!("{namespace}.{func_name}");
            let replacement =
                match execute_system_function(&full_func_name, &resolved_args, source_files, fs) {
                    Ok(res) => res,
                    Err(msg) => {
                        let (line, col, path) = find_sys_call_location(
                            fs,
                            source_files,
                            &full_func_name,
                            &resolved_args,
                        );
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

pub fn check_string(
    input: &str,
    context: &VariableContext,
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Result<String, SyntaxError> {
    let map = context.as_map();
    let mut result = input.to_string();

    let mut iterations = 0;
    while iterations < 10 {
        iterations += 1;
        let mut changed = resolve_vars_in_string(&mut result, &map, source_files, true, fs)?;
        changed |= validate_system_func(&mut result, &FUNC_PATTERN, source_files, fs)?;
        if !changed {
            changed =
                validate_user_func(&mut result, context, source_files, &USER_FUNC_PATTERN, fs)?;
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
        let (line, col, path) = find_variable_location(fs, source_files, var_name);
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

pub fn resolve_string(
    input: &str,
    context: &VariableContext,
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Result<String, SyntaxError> {
    let map = context.as_map();
    let mut result = input.to_string();

    let mut iterations = 0;
    while iterations < 10 {
        iterations += 1;
        let mut changed = resolve_vars_in_string(&mut result, &map, source_files, false, fs)?;
        changed |= try_resolve_system_func(&mut result, context, source_files, &FUNC_PATTERN, fs)?;
        if !changed {
            changed =
                try_resolve_user_func(&mut result, context, source_files, &USER_FUNC_PATTERN, fs)?;
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
        let (line, col, path) = find_variable_location(fs, source_files, var_name);
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
    fs: &dyn Fs,
) -> Result<Request, SyntaxError> {
    request.url = resolve_string(&request.url, context, source_files, fs)?;
    for (k, v) in &mut request.headers {
        *k = resolve_string(k, context, source_files, fs)?;
        *v = resolve_string(v, context, source_files, fs)?;
    }
    if let Some(body) = &request.body {
        request.body = Some(resolve_string(body, context, source_files, fs)?);
    }
    if let Some(timeout) = &request.timeout {
        request.timeout = Some(resolve_string(timeout, context, source_files, fs)?);
    }
    if let Some(auth) = &request.auth {
        request.auth = Some(resolve_string(auth, context, source_files, fs)?);
    }
    Ok(request)
}

pub fn collect_variable_errors(
    request: &Request,
    context: &VariableContext,
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    let request_line_1 = request.line + 1;
    let request_col_1 = request.character + 1;
    let mut error_index = 0usize;
    let mut try_resolve = |s: &str| {
        if let Err(mut e) = check_string(s, context, source_files, fs) {
            if e.line == 0 || e.line < request_line_1 {
                e.line = request_line_1;
                e.column = request_col_1 + error_index;
                if let Some(ref path) = request.source_path {
                    e.file_path = Some(path.clone());
                }
            }
            error_index += 1;
            errors.push(e);
        }
    };
    try_resolve(&request.url);
    for (k, v) in &request.headers {
        try_resolve(k);
        try_resolve(v);
    }
    if let Some(ref var_name) = request.headers_var {
        let is_defined_headers = matches!(
            context.as_map().get(var_name.as_str()),
            Some(super::variable_context::VariableValue::Headers(_))
        );
        if !is_defined_headers {
            try_resolve(&format!("{{{{{var_name}}}}}"));
        }
    }
    if let Some(ref body) = request.body {
        try_resolve(body);
    }
    if let Some(ref timeout) = request.timeout {
        try_resolve(timeout);
    }
    if let Some(ref auth) = request.auth {
        try_resolve(auth);
    }
    errors
}

pub fn collect_declared_variable_errors(
    variables: &[crate::syntax::variable_context::Variable],
    extra_context: &[crate::syntax::variable_context::Variable],
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Vec<SyntaxError> {
    let known_names: std::collections::HashSet<&str> = variables
        .iter()
        .chain(extra_context.iter())
        .map(|v| v.name.as_str())
        .collect();
    let broken_ref_names: std::collections::HashSet<&str> = variables
        .iter()
        .filter_map(|v| {
            if let crate::syntax::variable_context::VariableValue::Reference(ref_name) = &v.value {
                if !known_names.contains(ref_name.as_str()) {
                    return Some(ref_name.as_str());
                }
            }
            None
        })
        .collect();
    let string_context = crate::syntax::variable_context::VariableContext::builder()
        .file_variables(
            variables
                .iter()
                .chain(extra_context.iter())
                .cloned()
                .collect(),
        )
        .build();
    let mut errors = Vec::new();
    for var in variables {
        match &var.value {
            crate::syntax::variable_context::VariableValue::SystemFunction { name, args } => {
                let parts: Vec<&str> = name.split('.').collect();
                let validation_error = if parts.len() == 2 {
                    match functions::get_function(parts[0], parts[1]) {
                        None => Some(format!("Unknown function: {name}")),
                        Some(func) => func.validate_args(args).err(),
                    }
                } else {
                    Some(format!("Invalid function name: {name}"))
                };
                if let Some(msg) = validation_error {
                    let (line, col, path) = find_variable_location(fs, source_files, &var.name);
                    errors.push(SyntaxError::with_file(
                        msg,
                        line,
                        col,
                        0..0,
                        format_path(&path),
                    ));
                }
            }
            crate::syntax::variable_context::VariableValue::Reference(ref_name)
                if !known_names.contains(ref_name.as_str()) =>
            {
                let (line, col, path) = find_variable_location(fs, source_files, ref_name);
                errors.push(SyntaxError::with_file(
                    format!("Variable '{ref_name}' is not defined"),
                    line,
                    col,
                    0..0,
                    format_path(&path),
                ));
            }
            crate::syntax::variable_context::VariableValue::String(s) if s.contains("{{") => {
                if let Err(e) = check_string(s, &string_context, source_files, fs) {
                    let covered = extract_var_name_from_unresolved(&e.message)
                        .map(|n| broken_ref_names.contains(n.as_str()))
                        .unwrap_or(false);
                    if !covered {
                        errors.push(e);
                    }
                }
            }
            _ => {}
        }
    }
    errors
}

fn extract_var_name_from_unresolved(message: &str) -> Option<String> {
    let prefix = "Unresolved variable: '";
    let start = message.find(prefix)? + prefix.len();
    let end = message[start..].find('\'')?;
    Some(message[start..start + end].to_string())
}

pub fn resolve_auth_provider(
    mut auth_config: crate::syntax::auth::Config,
    context: &VariableContext,
    source_files: &[PathBuf],
    fs: &dyn Fs,
) -> Result<crate::syntax::auth::Config, SyntaxError> {
    for (_, token) in auth_config.fields.iter_mut() {
        token.value = resolve_string(&token.value, context, source_files, fs)?;
    }
    Ok(auth_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::auth::{AuthType, Config};
    use crate::syntax::token::{Token, TokenType};
    use crate::syntax::variable_context::{Variable, VariableContext, VariableValue};
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct NoopReader;
    impl Fs for NoopReader {
        fn read(&self, _path: &Path) -> Result<String, String> {
            Err("no filesystem".to_string())
        }
        fn resolve_path(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
            Ok(base.parent().unwrap_or(base).join(relative))
        }
        fn exists(&self, _path: &Path) -> bool {
            false
        }
        fn is_file(&self, _path: &Path) -> bool {
            false
        }
        fn is_dir(&self, _path: &Path) -> bool {
            false
        }
        fn read_dir(&self, _dir: &Path) -> Result<Vec<PathBuf>, String> {
            Ok(vec![])
        }
        fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
            Ok(path.to_path_buf())
        }
    }

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
            name: "test".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };
        let env_vars = vec![Variable {
            name: "token_value".to_string(),
            value: VariableValue::String("my-secret-token".to_string()),
        }];
        let resolved =
            resolve_auth_provider(config, &make_context(env_vars), &[], &NoopReader).unwrap();
        assert_eq!(resolved.fields["token"].value, "my-secret-token");
    }

    #[test]
    fn test_resolve_auth_provider_unresolved_variable() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("{{missing_var}}"));
        let config = Config {
            name: "test".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };
        let result = resolve_auth_provider(config, &make_context(vec![]), &[], &NoopReader);
        assert!(result.is_err());
    }
}
