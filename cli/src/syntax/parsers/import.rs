use super::parse_trait::Parse;
use crate::syntax::{
    error::SyntaxError,
    keywords::{KW_IMPORT, PUNC_SEMI},
    parse_result::ParseResult,
    reader::{expect, TokenReader},
    token::TokenType,
};
use std::path::Path;

pub struct ImportParser;
impl Parse for ImportParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_IMPORT)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let start_span = if let Some(tok) = r.cur() {
            tok.span.clone()
        } else {
            return Err(r.create_error(
                "Unexpected end of input".into(),
                r.source.len()..r.source.len(),
            ));
        };

        expect(
            r,
            |t| t.token_type == TokenType::Keyword && t.value == KW_IMPORT,
            format!("Expected '{KW_IMPORT}'"),
        )?;
        r.advance();
        r.skip_ignorable();
        let path_tok = expect(
            r,
            |t| matches!(t.token_type, TokenType::String | TokenType::Identifier),
            "Expected string literal or identifier",
        )?;
        let path = if path_tok.token_type == TokenType::String {
            path_tok.value.trim_matches('"').to_string()
        } else {
            path_tok.value.clone()
        };
        r.advance();
        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI,
            format!("Expected '{PUNC_SEMI}'"),
        )?;
        r.advance();

        // Resolve import
        let mut import_path_str = path.clone();
        if !import_path_str.ends_with(".rq") {
            import_path_str.push_str(".rq");
        }

        let current_dir = r.file_path.parent().unwrap_or(Path::new("."));
        let import_path = current_dir.join(&import_path_str);

        let canonical_path = match import_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Err(r.create_error_with_file(
                    format!("Import file not found: '{import_path_str}'"),
                    start_span.clone(),
                ));
            }
        };

        // Read and parse imported file
        let content = match std::fs::read_to_string(&canonical_path) {
            Ok(c) => c,
            Err(e) => {
                return Err(r.create_error_with_file(
                    format!("Failed to read imported file: {e}"),
                    start_span.clone(),
                ));
            }
        };

        let tokens = crate::syntax::tokenize(&content).map_err(|mut e| {
            e.file_path = Some(canonical_path.to_string_lossy().to_string());
            e
        })?;
        let imported_result = crate::syntax::analyze(&tokens, canonical_path.clone(), &content)?;

        // Merge results
        result.imported_files.push(canonical_path);
        result.imported_files.extend(imported_result.imported_files);
        result.requests.extend(imported_result.requests);
        result.file_variables.extend(imported_result.file_variables);
        result.environments.extend(imported_result.environments);
        result.auth_providers.extend(imported_result.auth_providers);
        result.endpoints.extend(imported_result.endpoints);

        Ok(())
    }
}
