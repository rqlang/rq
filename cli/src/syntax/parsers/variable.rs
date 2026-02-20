use super::{
    parse_trait::Parse,
    utils::{normalize_multiline_string, parse_system_function},
};
use crate::syntax::{
    error::SyntaxError,
    keywords::{
        KW_LET, OP_ASSIGN, PUNC_COLON, PUNC_COMMA, PUNC_DOLLAR, PUNC_DOT, PUNC_LBRACE,
        PUNC_LBRACKET, PUNC_RBRACE, PUNC_RBRACKET, PUNC_SEMI,
    },
    parse_result::ParseResult,
    reader::{expect, TokenReader},
    token::TokenType,
    variable_context::{Variable, VariableValue},
};

pub struct VariableParser;
impl Parse for VariableParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        r.is_keyword(KW_LET)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let var = parse_variable_declaration(r)?;
        result.file_variables.push(var);
        Ok(())
    }
}

pub fn parse_variable_declaration(r: &mut TokenReader) -> Result<Variable, SyntaxError> {
    expect(
        r,
        |t| t.token_type == TokenType::Keyword && t.value == KW_LET,
        format!("Expected '{KW_LET}'"),
    )?;
    r.advance();
    r.skip_ignorable();
    let name_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let name = name_tok.value.clone();
    r.advance();
    r.skip_ignorable();
    expect(
        r,
        |t| t.token_type == TokenType::Operator && t.value == OP_ASSIGN,
        format!("Expected '{OP_ASSIGN}'"),
    )?;
    r.advance();
    r.skip_ignorable();
    let value = parse_variable_value(r)?;
    r.skip_ignorable();

    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI,
        format!("Expected '{PUNC_SEMI}'"),
    )?;
    r.advance();

    Ok(Variable { name, value })
}

fn parse_variable_value(r: &mut TokenReader) -> Result<VariableValue, SyntaxError> {
    let token = if let Some(t) = r.cur() {
        t.clone()
    } else {
        return Err(r.create_error(
            "Expected variable value".into(),
            r.source.len()..r.source.len(),
        ));
    };

    match token.token_type {
        TokenType::Identifier => parse_identifier_value(r, &token),
        TokenType::String => {
            let raw = token.value.trim_matches('"').trim_matches('\'');
            let s = normalize_multiline_string(raw, " ");
            r.advance();
            Ok(VariableValue::String(s))
        }
        TokenType::Punctuation if token.value == PUNC_LBRACKET => parse_array_or_headers(r),
        TokenType::Punctuation if token.value == PUNC_DOLLAR => parse_json_value(r),
        TokenType::Punctuation if token.value == PUNC_LBRACE => Err(r.create_error_with_file(
            "Bare '{' syntax is not supported. Use '${' prefix.".into(),
            token.span.clone(),
        )),
        _ => Err(r.create_error(
            "Expected identifier, string, array, or JSON object for variable value".into(),
            token.span.clone(),
        )),
    }
}

fn parse_identifier_value(
    r: &mut TokenReader,
    t: &crate::syntax::token::Token,
) -> Result<VariableValue, SyntaxError> {
    let ref_name = t.value.clone();
    if crate::syntax::functions::is_known_namespace(&ref_name) {
        r.advance();
        r.skip_ignorable();
        if let Some(dot) = r.cur() {
            if dot.token_type == TokenType::Punctuation && dot.value == PUNC_DOT {
                r.advance();
                r.skip_ignorable();
                parse_system_function(r, &ref_name)
            } else {
                Ok(VariableValue::Reference(ref_name))
            }
        } else {
            Ok(VariableValue::Reference(ref_name))
        }
    } else {
        r.advance();
        Ok(VariableValue::Reference(ref_name))
    }
}

fn parse_array_or_headers(r: &mut TokenReader) -> Result<VariableValue, SyntaxError> {
    // lookahead to decide if this is a headers style array ["Name": "Val", ...] or a simple array of strings
    r.advance();
    r.skip_ignorable();
    let mut is_headers = false;
    if let Some(first) = r.cur() {
        if first.token_type == TokenType::String {
            let save_idx = r.idx; // no need for mut; single assignment
            r.advance();
            r.skip_ignorable();
            if let Some(col) = r.cur() {
                if col.token_type == TokenType::Punctuation && col.value == PUNC_COLON {
                    is_headers = true;
                }
            }
            r.idx = save_idx - 1; // because we advanced once after '['
            r.advance();
        }
    }
    if is_headers {
        parse_headers_variable(r)
    } else {
        parse_array_variable(r)
    }
}

fn parse_headers_variable(r: &mut TokenReader) -> Result<VariableValue, SyntaxError> {
    let mut headers = Vec::new();
    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACKET {
                r.advance();
                break;
            }
            if ct.token_type == TokenType::String {
                let key = ct.value.trim_matches('"').to_string();
                r.advance();
                r.skip_ignorable();
                expect(
                    r,
                    |t| t.token_type == TokenType::Punctuation && t.value == PUNC_COLON,
                    format!("Expected '{PUNC_COLON}'"),
                )?;
                r.advance();
                r.skip_ignorable();
                let val_tok = expect(
                    r,
                    |t| matches!(t.token_type, TokenType::String | TokenType::Identifier),
                    "Expected string literal or identifier",
                )?;
                let val = if val_tok.token_type == TokenType::String {
                    val_tok.value.trim_matches('"').to_string()
                } else {
                    val_tok.value.clone()
                };
                r.advance();
                headers.push((key, val));
                r.skip_ignorable();
                if let Some(com) = r.cur() {
                    if com.token_type == TokenType::Punctuation && com.value == PUNC_COMMA {
                        r.advance();
                    }
                }
                continue;
            }
        }
        if r.is_end() {
            break;
        } else {
            r.advance();
        }
    }
    Ok(VariableValue::Headers(headers))
}

fn parse_array_variable(r: &mut TokenReader) -> Result<VariableValue, SyntaxError> {
    let mut arr = Vec::new();
    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACKET {
                r.advance();
                break;
            }
            if ct.token_type == TokenType::String {
                let sv = ct.value.trim_matches('"').to_string();
                arr.push(sv);
                r.advance();
                r.skip_ignorable();
                if let Some(com) = r.cur() {
                    if com.token_type == TokenType::Punctuation && com.value == PUNC_COMMA {
                        r.advance();
                    }
                }
                continue;
            }
        }
        if r.is_end() {
            break;
        } else {
            r.advance();
        }
    }
    Ok(VariableValue::Array(arr))
}

fn parse_json_value(r: &mut TokenReader) -> Result<VariableValue, SyntaxError> {
    r.advance();
    r.skip_ignorable();
    let _ = expect(
        r,
        |tk| tk.token_type == TokenType::Punctuation && tk.value == PUNC_LBRACE,
        format!("Expected '{PUNC_LBRACE}'"),
    )?;
    let mut depth = 0;
    let mut collected = String::new();
    while let Some(tok) = r.cur() {
        if tok.token_type == TokenType::Punctuation {
            if tok.value == PUNC_LBRACE {
                depth += 1;
            }
            if tok.value == PUNC_RBRACE {
                depth -= 1;
            }
            collected.push_str(&tok.value);
            r.advance();
            if depth == 0 {
                break;
            }
        } else {
            collected.push_str(&tok.value);
            r.advance();
        }
    }
    Ok(VariableValue::Json(collected))
}
