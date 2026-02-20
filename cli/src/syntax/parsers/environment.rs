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

pub struct EnvironmentParser;
impl Parse for EnvironmentParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_ENV)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let (env_name, vars) = parse_environment_definition(r, &result.environments)?;
        result.environments.insert(env_name, vars);
        Ok(())
    }
}

pub(crate) fn parse_environment_definition(
    r: &mut TokenReader,
    existing_environments: &HashMap<String, Vec<Variable>>,
) -> Result<(String, Vec<Variable>), SyntaxError> {
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
    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACE {
                r.advance();
                break;
            }
        } else {
            // Use expect to generate the error with proper backtracking
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
        vars.push(Variable {
            name: key,
            value: VariableValue::String(value),
        });
    }
    Ok((env_name, vars))
}
