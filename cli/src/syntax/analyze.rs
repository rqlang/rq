use super::{
    error::SyntaxError,
    parse_result::ParseResult,
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
) -> Result<ParseResult, SyntaxError> {
    let mut r = TokenReader::new(tokens.to_vec(), file_path, source.to_string());
    let mut result = ParseResult {
        requests: Vec::new(),
        environments: std::collections::HashMap::new(),
        auth_providers: std::collections::HashMap::new(),
        endpoints: std::collections::HashMap::new(),
        file_variables: Vec::new(),
        imported_files: Vec::new(),
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
                parser.parse(&mut r, &mut result)?;
                parsed = true;
                break;
            }
        }

        if !parsed {
            if let Some(tok) = r.cur() {
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
