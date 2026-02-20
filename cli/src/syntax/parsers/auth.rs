use super::parse_trait::Parse;
use crate::syntax::{
    auth::{AuthType, Config as AuthProvider},
    error::SyntaxError,
    keywords::{
        KW_AUTH, PUNC_COLON, PUNC_COMMA, PUNC_DOT, PUNC_LBRACE, PUNC_LPAREN, PUNC_RBRACE,
        PUNC_RPAREN, PUNC_SEMI,
    },
    parse_result::ParseResult,
    reader::{expect, TokenReader},
    token::{Token, TokenType},
};
use std::collections::HashMap;

pub struct AuthParser;
impl Parse for AuthParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_AUTH)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let auth_provider = parse_auth_definition(r, &result.auth_providers)?;
        result
            .auth_providers
            .insert(auth_provider.name.clone(), auth_provider);
        Ok(())
    }
}

pub(crate) fn parse_auth_definition(
    r: &mut TokenReader,
    existing_providers: &HashMap<String, AuthProvider>,
) -> Result<AuthProvider, SyntaxError> {
    let auth_token = expect(
        r,
        |t| t.token_type == TokenType::Keyword && t.value == KW_AUTH,
        format!("Expected '{KW_AUTH}'"),
    )?;
    let span = auth_token.span.clone();

    r.advance();
    r.skip_ignorable();

    let name_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let auth_name = name_tok.value.clone();

    if existing_providers.contains_key(&auth_name) {
        return Err(r.create_error_with_file(
            format!("Duplicate auth provider definition: '{auth_name}'"),
            name_tok.span.clone(),
        ));
    }

    r.advance();
    r.skip_ignorable();

    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
        format!("Expected '{PUNC_LPAREN}'"),
    )?;
    r.advance();
    r.skip_ignorable();

    let namespace_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let namespace = namespace_tok.value.clone();
    r.advance();
    r.skip_ignorable();

    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_DOT,
        format!("Expected '{PUNC_DOT}'"),
    )?;
    r.advance();
    r.skip_ignorable();

    let type_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let type_value = type_tok.value.clone();
    r.advance();
    r.skip_ignorable();

    if namespace != "auth_type" {
        return Err(r.create_error(
            format!("Expected namespace 'auth_type', got '{namespace}'"),
            namespace_tok.span.clone(),
        ));
    }

    // Parse the auth type value (e.g., "bearer")
    let auth_type = AuthType::from_str(&type_value)
        .map_err(|e| r.create_error(e.message, type_tok.span.clone()))?;

    let required_fields = auth_type.required_fields();
    let optional_fields = auth_type.optional_fields();

    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
        format!("Expected '{PUNC_RPAREN}'"),
    )?;
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
            break;
        }

        let key_tok = expect(
            r,
            |t| matches!(t.token_type, TokenType::Identifier),
            "Expected identifier",
        )?;
        let key = key_tok.value.clone();

        validate_field_is_allowed(
            &auth_type,
            &auth_name,
            &key,
            &required_fields,
            &optional_fields,
            &key_tok,
            r,
        )?;

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
            format!("{{{{{}}}}}", value_tok.value)
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

        vars.push((
            key,
            Token {
                token_type: value_tok.token_type.clone(),
                value,
                span: value_tok.span.clone(),
            },
        ));
    }

    let mut fields = HashMap::new();
    for (key, token) in vars {
        fields.insert(key, token);
    }

    validate_required_fields_present(
        &auth_type,
        &auth_name,
        &fields,
        &required_fields,
        span.clone(),
        r,
    )?;

    let config = AuthProvider {
        name: auth_name,
        auth_type,
        fields,
        file_path: r.file_path.clone(),
    };

    if let Err(mut e) = config.validate() {
        if e.line == 0 || e.column == 0 {
            let err_span = if e.span.start == 0 && e.span.end == 0 {
                span.clone()
            } else {
                e.span.clone()
            };
            let (l, c) = r.get_line_col(err_span.start);
            e.line = l;
            e.column = c;
            e.span = err_span;
        }
        e.file_path = Some(r.file_path.to_string_lossy().to_string());
        return Err(e);
    }

    Ok(config)
}

fn validate_field_is_allowed(
    auth_type: &AuthType,
    auth_name: &str,
    key: &str,
    required_fields: &[&str],
    optional_fields: &[&str],
    token: &crate::syntax::token::Token,
    r: &TokenReader,
) -> Result<(), SyntaxError> {
    if !required_fields.contains(&key) && !optional_fields.contains(&key) {
        return Err(r.create_error_with_file(
            format!(
                "{} auth '{}' has unexpected field '{}'. Expected fields: {}",
                auth_type.as_str(),
                auth_name,
                key,
                required_fields.join(", ")
            ),
            token.span.clone(),
        ));
    }
    Ok(())
}

fn validate_required_fields_present(
    auth_type: &AuthType,
    auth_name: &str,
    fields: &HashMap<String, Token>,
    required_fields: &[&str],
    span: std::ops::Range<usize>,
    r: &TokenReader,
) -> Result<(), SyntaxError> {
    for field in required_fields {
        if !fields.contains_key(*field) {
            return Err(r.create_error_with_file(
                format!(
                    "{} auth '{}' missing required field '{}'",
                    auth_type.as_str(),
                    auth_name,
                    field
                ),
                span.clone(),
            ));
        }
    }
    Ok(())
}
