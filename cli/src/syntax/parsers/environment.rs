use super::parse_trait::Parse;
use crate::syntax::{
    error::SyntaxError,
    keywords::{KW_ENV, PUNC_COLON, PUNC_COMMA, PUNC_LBRACE, PUNC_RBRACE, PUNC_SEMI},
    parse_result::ParseResult,
    reader::{expect, TokenReader},
    token::TokenType,
    variable_context::{Variable, VariableValue},
};

use std::collections::HashMap;

type EnvDefinition = (
    String,
    Vec<Variable>,
    usize,
    usize,
    Vec<(String, usize, usize)>,
);

pub struct EnvironmentParser;
impl Parse for EnvironmentParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_ENV)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let (env_name, vars, line, character, key_locs) =
            parse_environment_definition(r, &result.environments)?;
        let file = r.file_path.to_string_lossy().to_string();
        result
            .environment_locations
            .insert(env_name.clone(), (file.clone(), line, character));
        let env_var_map = result
            .env_variable_locations
            .entry(env_name.clone())
            .or_default();
        for (key_name, key_line, key_char) in key_locs {
            env_var_map.insert(key_name, (file.clone(), key_line, key_char));
        }
        result.environments.insert(env_name, vars);
        Ok(())
    }
}

pub(crate) fn parse_environment_definition(
    r: &mut TokenReader,
    existing_environments: &HashMap<String, Vec<Variable>>,
) -> Result<EnvDefinition, SyntaxError> {
    expect(
        r,
        |t| t.token_type == TokenType::Keyword && t.value == KW_ENV,
        format!("Expected '{KW_ENV}'"),
    )?;
    r.advance();
    r.skip_ignorable();
    let name_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let env_name = name_tok.value.clone();
    let (line_1, col_1) = r.get_line_col(name_tok.span.start);
    let line = line_1.saturating_sub(1);
    let character = col_1.saturating_sub(1);

    if existing_environments.contains_key(&env_name) {
        return Err(r.create_error_with_file(
            format!("Duplicate environment definition: '{env_name}'"),
            name_tok.span.clone(),
        ));
    }

    r.advance();
    r.skip_ignorable();
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACE,
        format!("Expected '{PUNC_LBRACE}'"),
    )?;
    r.advance();
    let mut vars = Vec::new();
    let mut key_locs: Vec<(String, usize, usize)> = Vec::new();
    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACE {
                r.advance();
                break;
            }
        } else {
            expect(
                r,
                |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACE,
                "Expected '}'",
            )?;
        }
        let key_tok = expect(
            r,
            |t| matches!(t.token_type, TokenType::Identifier),
            "Expected identifier",
        )?;
        let key = key_tok.value.clone();
        let (key_line_1, key_col_1) = r.get_line_col(key_tok.span.start);
        let key_line = key_line_1.saturating_sub(1);
        let key_char = key_col_1.saturating_sub(1);
        r.advance();
        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_COLON,
            format!("Expected '{PUNC_COLON}'"),
        )?;
        r.advance();
        r.skip_ignorable();
        let value_tok = expect(
            r,
            |t| matches!(t.token_type, TokenType::String | TokenType::Identifier),
            "Expected string literal or identifier",
        )?;
        let value = if value_tok.token_type == TokenType::String {
            value_tok.value.trim_matches('"').to_string()
        } else {
            value_tok.value.clone()
        };
        r.advance();
        r.skip_ignorable();
        if let Some(tk) = r.cur() {
            if tk.token_type == TokenType::Punctuation
                && (tk.value == PUNC_COMMA || tk.value == PUNC_SEMI)
            {
                r.advance();
            }
        }
        key_locs.push((key.clone(), key_line, key_char));
        vars.push(Variable {
            name: key,
            value: VariableValue::String(value),
        });
    }
    Ok((env_name, vars, line, character, key_locs))
}
