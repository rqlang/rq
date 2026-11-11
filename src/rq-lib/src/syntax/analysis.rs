use super::{
    error::SyntaxError,
    fs::Fs,
    parse_result::ParseResult,
    parsers::{
        AuthParser, EndpointParser, EnvironmentParser, ImportParser, Parse, RequestParser,
        VariableParser,
    },
    reader::TokenReader,
    token::TokenType,
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
        required_variable_locations: std::collections::HashMap::new(),
    })
}

fn skip_to_next_statement(r: &mut TokenReader, parsers: &[Box<dyn Parse>]) {
    let mut braces = 0usize;
    let mut groups = 0usize;
    let mut at_statement_start = true;
    while let Some(tok) = r.cur() {
        if !at_statement_start && braces == 0 && parsers.iter().any(|p| p.can_parse(r)) {
            return;
        }
        at_statement_start = false;
        let ends_statement = match (&tok.token_type, tok.value.as_str()) {
            (TokenType::Punctuation, "{") => {
                braces += 1;
                false
            }
            (TokenType::Punctuation, "}") => {
                braces = braces.saturating_sub(1);
                false
            }
            (TokenType::Punctuation, "(" | "[") => {
                groups += 1;
                false
            }
            (TokenType::Punctuation, ")" | "]") => {
                groups = groups.saturating_sub(1);
                false
            }
            (TokenType::Punctuation, ";") | (TokenType::Newline, _) => braces == 0 && groups == 0,
            _ => false,
        };
        r.advance();
        if ends_statement {
            return;
        }
    }
}

fn analyze_impl(
    tokens: &[super::token::Token],
    file_path: PathBuf,
    source: &str,
    fs: &dyn Fs,
    lenient: bool,
) -> Result<ParseResult, SyntaxError> {
    let mut r = TokenReader::new(tokens.to_vec(), file_path, source.to_string());
    let mut result = ParseResult {
        requests: Vec::new(),
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
        required_variable_locations: std::collections::HashMap::new(),
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
                        skip_to_next_statement(&mut r, &parsers);
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
