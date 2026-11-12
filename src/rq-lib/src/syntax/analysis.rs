use super::{
    error::SyntaxError,
    fs::Fs,
    parse_result::ParseResult,
    parsers::utils::starts_top_level_statement,
    parsers::{
        AuthParser, EndpointParser, EnvironmentParser, ImportParser, Parse, RequestParser,
        VariableParser,
    },
    reader::TokenReader,
};
use std::path::PathBuf;

pub fn analyze(
    tokens: &[super::token::Token],
    file_path: PathBuf,
    source: &str,
    fs: &dyn Fs,
) -> Result<ParseResult, SyntaxError> {
    analyze_impl(tokens, file_path, source, fs, false)
}

pub fn analyze_lenient(
    tokens: &[super::token::Token],
    file_path: PathBuf,
    source: &str,
    fs: &dyn Fs,
) -> ParseResult {
    analyze_impl(tokens, file_path, source, fs, true).unwrap_or_else(|_| ParseResult {
        requests: Vec::new(),
        request_scopes: Vec::new(),
        environments: std::collections::HashMap::new(),
        environment_locations: std::collections::HashMap::new(),
        environment_colors: std::collections::HashMap::new(),
        confirm_environments: std::collections::HashSet::new(),
        auth_providers: std::collections::HashMap::new(),
        endpoints: std::collections::HashMap::new(),
        file_variables: Vec::new(),
        imported_files: Vec::new(),
        let_variable_locations: std::collections::HashMap::new(),
        env_variable_locations: std::collections::HashMap::new(),
    })
}

fn analyze_impl(
    tokens: &[super::token::Token],
    file_path: PathBuf,
    source: &str,
    fs: &dyn Fs,
    lenient: bool,
) -> Result<ParseResult, SyntaxError> {
    let mut r = TokenReader::new(tokens.to_vec(), file_path, source.to_string());
    r.lenient = lenient;
    let mut result = ParseResult {
        requests: Vec::new(),
        request_scopes: Vec::new(),
        environments: std::collections::HashMap::new(),
        environment_locations: std::collections::HashMap::new(),
        environment_colors: std::collections::HashMap::new(),
        confirm_environments: std::collections::HashSet::new(),
        auth_providers: std::collections::HashMap::new(),
        endpoints: std::collections::HashMap::new(),
        file_variables: Vec::new(),
        imported_files: Vec::new(),
        let_variable_locations: std::collections::HashMap::new(),
        env_variable_locations: std::collections::HashMap::new(),
    };

    let parsers: Vec<Box<dyn Parse>> = vec![
        Box::new(ImportParser),
        Box::new(VariableParser),
        Box::new(EnvironmentParser),
        Box::new(AuthParser),
        Box::new(EndpointParser),
        Box::new(RequestParser),
    ];

    while !r.is_end() {
        r.skip_ignorable();
        if r.is_end() {
            break;
        }

        let mut parsed = false;
        for parser in &parsers {
            if parser.can_parse(&r) {
                let saved_idx = r.idx;
                match parser.parse(&mut r, &mut result, fs) {
                    Ok(()) => {}
                    Err(_) if lenient => {
                        r.idx = saved_idx;
                        r.skip_statement(&starts_top_level_statement);
                    }
                    Err(e) => return Err(e),
                }
                parsed = true;
                break;
            }
        }

        if !parsed {
            if lenient {
                r.advance();
            } else if let Some(tok) = r.cur() {
                return Err(r.create_error_with_file(
                    format!("Unexpected token '{}' at top level", tok.value),
                    tok.span.clone(),
                ));
            } else {
                return Err(r.create_error_with_file(
                    "Unexpected end of tokens".into(),
                    r.source.len()..r.source.len(),
                ));
            }
        }
    }
    Ok(result)
}
