use super::parse_trait::Parse;
use crate::syntax::{
    error::SyntaxError,
    fs::Fs,
    keywords::{KW_IMPORT, PUNC_SEMI},
    parse_result::ParseResult,
    reader::{expect, TokenReader},
    token::TokenType,
};

pub struct ImportParser;
impl Parse for ImportParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_IMPORT)
    }
    fn parse(
        &self,
        r: &mut TokenReader,
        result: &mut ParseResult,
        fs: &dyn Fs,
    ) -> Result<(), SyntaxError> {
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

        let mut import_path_str = path.clone();
        if !import_path_str.ends_with(".rq") {
            import_path_str.push_str(".rq");
        }

        let canonical_path = fs
            .resolve_path(&r.file_path, &import_path_str)
            .map_err(|_| {
                r.create_error_with_file(
                    format!("Import file not found: '{import_path_str}'"),
                    start_span.clone(),
                )
            })?;

        let content = fs.read(&canonical_path).map_err(|e| {
            r.create_error_with_file(
                format!("Failed to read imported file: {e}"),
                start_span.clone(),
            )
        })?;

        let tokens = crate::syntax::tokenize(&content).map_err(|mut e| {
            e.file_path = Some(canonical_path.to_string_lossy().to_string());
            e
        })?;
        let imported_result =
            crate::syntax::analysis::analyze(&tokens, canonical_path.clone(), &content, fs)?;

        result.imported_files.push(canonical_path);
        result.imported_files.extend(imported_result.imported_files);
        result.requests.extend(imported_result.requests);
        result.file_variables.extend(imported_result.file_variables);
        result.environments.extend(imported_result.environments);
        result
            .environment_locations
            .extend(imported_result.environment_locations);
        result.auth_providers.extend(imported_result.auth_providers);
        result.endpoints.extend(imported_result.endpoints);
        result
            .let_variable_locations
            .extend(imported_result.let_variable_locations);
        for (name, loc) in imported_result.required_variable_locations {
            result
                .required_variable_locations
                .entry(name)
                .or_insert(loc);
        }
        for (env_name, key_map) in imported_result.env_variable_locations {
            result
                .env_variable_locations
                .entry(env_name)
                .or_default()
                .extend(key_map);
        }

        Ok(())
    }
}
