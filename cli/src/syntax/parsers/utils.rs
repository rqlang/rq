use crate::syntax::{
    error::SyntaxError,
    keywords::{PUNC_COLON, PUNC_COMMA, PUNC_LBRACKET, PUNC_LPAREN, PUNC_RBRACKET, PUNC_RPAREN},
    reader::{expect, make_error, TokenReader},
    token::TokenType,
    variable_context::{Variable, VariableValue},
};

pub fn check_variable_type(
    name: &str,
    expected_types: &[fn(&VariableValue) -> bool],
    file_vars: &[Variable],
    token: &crate::syntax::token::Token,
    r: &TokenReader,
) -> Result<(), SyntaxError> {
    if let Some(var) = file_vars.iter().find(|v| v.name == name) {
        let is_valid = expected_types.iter().any(|check| check(&var.value));
        if !is_valid {
            return Err(r.create_error(
                format!("Variable '{name}' has invalid type for this parameter"),
                token.span.clone(),
            ));
        }
    }
    Ok(())
}

pub fn is_string_like(v: &VariableValue) -> bool {
    matches!(
        v,
        VariableValue::String(_)
            | VariableValue::Reference(_)
            | VariableValue::SystemFunction { .. }
    )
}

pub fn is_headers_like(v: &VariableValue) -> bool {
    matches!(v, VariableValue::Headers(_) | VariableValue::Reference(_))
}

pub fn parse_system_function(
    r: &mut TokenReader,
    namespace: &str,
) -> Result<VariableValue, SyntaxError> {
    let func_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let func_name = func_tok.value.clone();

    // Validate that the function exists and validate arguments
    let func = crate::syntax::functions::get_function(namespace, &func_name).ok_or_else(|| {
        r.create_error_with_file(
            format!("Unknown function: {namespace}.{func_name}"),
            func_tok.span.clone(),
        )
    })?;

    r.advance();
    r.skip_ignorable();
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
        format!("Expected '{PUNC_LPAREN}'"),
    )?;
    r.advance();
    r.skip_ignorable();

    let mut args = Vec::new();
    loop {
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN {
                r.advance();
                break;
            }
            if t.token_type == TokenType::String {
                let raw = t.value.trim_matches('"').trim_matches('\'');
                let arg = normalize_multiline_string(raw, " ");
                args.push(arg);
                r.advance();
                r.skip_ignorable();
                if let Some(comma) = r.cur() {
                    if comma.token_type == TokenType::Punctuation && comma.value == PUNC_COMMA {
                        r.advance();
                        r.skip_ignorable();
                    }
                }
            } else {
                return Err(
                    r.create_error_no_file("Expected string literal".into(), t.span.clone())
                );
            }
        } else {
            return Err(r.create_error(
                "Unexpected end of input in system function call".into(),
                r.source.len()..r.source.len(),
            ));
        }
    }

    func.validate_args(&args)
        .map_err(|msg| r.create_error_with_file(msg, func_tok.span.clone()))?;

    Ok(VariableValue::SystemFunction {
        name: format!("{namespace}.{func_name}"),
        args,
    })
}

pub fn normalize_multiline_string(s: &str, separator: &str) -> String {
    if !s.contains('\n') {
        return s.to_string();
    }

    let mut result = String::new();
    let parts: Vec<&str> = s.split('\n').collect();

    for (i, part) in parts.iter().enumerate() {
        let trimmed = if i > 0 { part.trim_start() } else { *part };

        let clean_part = trimmed.trim_end_matches('\r');

        if i > 0 {
            result.push_str(separator);
        }
        result.push_str(clean_part);
    }
    result
}

pub fn parse_string_or_identifier(r: &mut TokenReader) -> Result<String, SyntaxError> {
    if let Some(t) = r.cur() {
        match t.token_type {
            TokenType::String => {
                let raw = t.value.trim_matches('"').trim_matches('\'');
                let value = normalize_multiline_string(raw, "");
                r.advance();
                Ok(value)
            }
            TokenType::Identifier => {
                let ident = t.value.clone();
                r.advance();
                Ok(format!("{{{{{ident}}}}}"))
            }
            _ => Err(r.create_error_no_file(
                "Expected string literal or identifier".into(),
                t.span.clone(),
            )),
        }
    } else {
        Err(r.create_error_no_file("Expected value".into(), r.source.len()..r.source.len()))
    }
}

pub fn parse_headers_array(r: &mut TokenReader) -> Result<Vec<(String, String)>, SyntaxError> {
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACKET,
        format!("Expected '{PUNC_LBRACKET}'"),
    )?;
    r.advance();
    let mut headers = Vec::new();
    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACKET {
                r.advance();
                break;
            }
            if ct.token_type == TokenType::String {
                let key_raw = ct.value.trim_matches('"').trim_matches('\'');
                let key = normalize_multiline_string(key_raw, " ");
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
                    let v_raw = val_tok.value.trim_matches('"').trim_matches('\'');
                    normalize_multiline_string(v_raw, " ")
                } else {
                    format!("{{{{{}}}}}", val_tok.value)
                };
                r.advance();
                headers.push((key, val));
                r.skip_ignorable();
                if let Some(com) = r.cur() {
                    if com.token_type == TokenType::Punctuation && com.value == PUNC_COMMA {
                        r.advance();
                    } else if com.token_type == TokenType::Punctuation && com.value == PUNC_RBRACKET
                    {
                        // Next iteration will handle RBRACKET
                    } else {
                        return Err(make_error(
                            r,
                            com,
                            format!("Expected '{PUNC_COMMA}' or '{PUNC_RBRACKET}'"),
                        ));
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
    Ok(headers)
}

pub fn can_parse_attributed(r: &TokenReader, keyword: &str) -> bool {
    if r.is_keyword(keyword) {
        return true;
    }
    if let Some(t) = r.cur() {
        if t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACKET {
            let mut depth = 0;
            let mut idx = r.idx;
            while idx < r.tokens.len() {
                let tok = &r.tokens[idx];
                if tok.token_type == TokenType::Punctuation {
                    if tok.value == PUNC_LBRACKET {
                        depth += 1;
                    } else if tok.value == PUNC_RBRACKET {
                        depth -= 1;
                    }
                }

                idx += 1;

                if depth == 0 {
                    // We just closed a bracket group. Check what follows.
                    let mut lookahead = idx;
                    while lookahead < r.tokens.len() {
                        let next = &r.tokens[lookahead];
                        if matches!(
                            next.token_type,
                            TokenType::Whitespace | TokenType::Newline | TokenType::Comment
                        ) {
                            lookahead += 1;
                            continue;
                        }

                        if next.token_type == TokenType::Keyword && next.value == keyword {
                            return true;
                        }

                        if next.token_type == TokenType::Punctuation && next.value == PUNC_LBRACKET
                        {
                            // Another attribute group starting, let the main loop handle it
                            break;
                        }

                        // Something else
                        return false;
                    }
                }
            }
        }
    }
    false
}
