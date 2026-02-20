use super::{
    attributes::{
        parse_attributes, AttributeContext, AttributeParser, AuthAttributeParser,
        MethodAttributeParser, TimeoutAttributeParser,
    },
    parse_trait::Parse,
    utils::{
        can_parse_attributed, check_variable_type, is_headers_like, is_string_like,
        normalize_multiline_string, parse_headers_array, parse_string_or_identifier,
        parse_system_function,
    },
};
use crate::syntax::{
    error::SyntaxError,
    http_method::HttpMethod,
    keywords::{
        KW_RQ, PUNC_COLON, PUNC_COMMA, PUNC_DOLLAR, PUNC_LBRACE, PUNC_LPAREN, PUNC_RBRACE,
        PUNC_RPAREN, PUNC_SEMI,
    },
    parse_result::{ParseResult, Request},
    reader::{expect, TokenReader},
    token::TokenType,
    variable_context::{Variable, VariableValue},
};

pub struct RequestParser;
impl Parse for RequestParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        can_parse_attributed(r, KW_RQ)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let (req, req_vars) = parse_request_with_context(
            r,
            &result.file_variables,
            &Vec::new(),
            None,
            &result.requests,
        )?;
        result
            .requests
            .push(crate::syntax::parse_result::RequestWithVariables {
                request: req,
                endpoint_variables: Vec::new(),
                request_variables: req_vars,
            });
        Ok(())
    }
}

pub fn parse_body_value(r: &mut TokenReader) -> Result<String, SyntaxError> {
    if let Some(val) = r.cur() {
        match val.token_type {
            TokenType::String => {
                let raw = val.value.trim_matches('"').trim_matches('\'');
                let b = normalize_multiline_string(raw, " ");
                r.advance();
                Ok(b)
            }
            TokenType::Identifier => {
                let ident = val.value.clone();
                if crate::syntax::functions::is_known_namespace(&ident) {
                    r.advance();
                    r.skip_ignorable();
                    if let Some(dot) = r.cur() {
                        if dot.token_type == TokenType::Punctuation && dot.value == "." {
                            r.advance();
                            r.skip_ignorable();
                            let sys_func = parse_system_function(r, &ident)?;
                            if let VariableValue::SystemFunction { name, args } = sys_func {
                                let args_str = args.join("\x1F"); // Use unit separator as delimiter
                                return Ok(format!("{{{{${name}\x1E{args_str}}}}}"));
                            }
                        }
                    }
                }
                r.advance();
                Ok(format!("{{{{{ident}}}}}"))
            }
            TokenType::Punctuation if val.value == PUNC_DOLLAR => {
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
                Ok(collected)
            }
            TokenType::Punctuation if val.value == PUNC_LBRACE => Err(r.create_error_with_file(
                "Bare '{' syntax is not supported. Use '${' prefix.".into(),
                val.span.clone(),
            )),
            _ => Err(r.create_error(
                "Expected string, identifier, or JSON object for body".into(),
                val.span.clone(),
            )),
        }
    } else {
        Err(r.create_error("Expected body value".into(), r.source.len()..r.source.len()))
    }
}

pub type ConstructorParams = (
    String,                // url
    Vec<(String, String)>, // headers
    Option<String>,        // body (rq only; endpoints will reject)
    Option<String>,        // headers_var
    Vec<Variable>,         // variables (from attributes)
);

pub fn parse_constructor_params(
    r: &mut TokenReader,
    file_vars: &[Variable],
) -> Result<ConstructorParams, SyntaxError> {
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut body = None;
    let mut headers_var: Option<String> = None;
    let request_variables = Vec::new();
    let mut positional_index = 0;
    loop {
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation {
                if t.value == PUNC_RPAREN {
                    break;
                }
                if t.value == PUNC_SEMI || t.value == PUNC_LBRACE {
                    // If we encounter a semi or brace, we assume the parameter list is over.
                    // We let the caller handle the missing ')' check.
                    break;
                }
            }
        } else {
            break;
        }
        let is_named = if let Some(t) = r.cur() {
            if t.token_type == TokenType::Identifier {
                let save = r.idx;
                r.advance();
                r.skip_ignorable();
                let has_colon = r
                    .cur()
                    .map(|t| t.token_type == TokenType::Punctuation && t.value == PUNC_COLON)
                    .unwrap_or(false);
                r.idx = save;
                has_colon
            } else {
                false
            }
        } else {
            false
        };
        if is_named {
            let name_tok = expect(
                r,
                |t| t.token_type == TokenType::Identifier,
                "Expected identifier",
            )?;
            let param_name = name_tok.value.clone();
            r.advance();
            r.skip_ignorable();
            expect(
                r,
                |t| t.token_type == TokenType::Punctuation && t.value == PUNC_COLON,
                format!("Expected '{PUNC_COLON}'"),
            )?;
            r.advance();
            r.skip_ignorable();
            match param_name.as_str() {
                "url" => {
                    if let Some(t) = r.cur() {
                        if t.token_type == TokenType::Identifier {
                            check_variable_type(&t.value, &[is_string_like], file_vars, t, r)?;
                        }
                    }
                    url = parse_string_or_identifier(r)?;
                }
                "headers" => {
                    if let Some(tk) = r.cur() {
                        if tk.token_type == TokenType::Identifier {
                            check_variable_type(&tk.value, &[is_headers_like], file_vars, tk, r)?;
                            headers_var = Some(tk.value.clone());
                            r.advance();
                        } else {
                            headers = parse_headers_array(r)?;
                        }
                    } else {
                        return Err(r.create_error(
                            "Expected headers value".into(),
                            r.source.len()..r.source.len(),
                        ));
                    }
                }
                "body" => {
                    body = Some(parse_body_value(r)?);
                }
                _ => {
                    return Err(r.create_error(
                        format!("Unknown parameter name: {param_name}"),
                        name_tok.span.clone(),
                    ));
                }
            }
        } else {
            match positional_index {
                0 => {
                    if let Some(t) = r.cur() {
                        if t.token_type == TokenType::Identifier {
                            check_variable_type(&t.value, &[is_string_like], file_vars, t, r)?;
                        }
                    }
                    url = parse_string_or_identifier(r)?;
                }
                1 => {
                    if let Some(tk) = r.cur() {
                        if tk.token_type == TokenType::Identifier {
                            check_variable_type(&tk.value, &[is_headers_like], file_vars, tk, r)?;
                            headers_var = Some(tk.value.clone());
                            r.advance();
                        } else {
                            headers = parse_headers_array(r)?;
                        }
                    } else {
                        return Err(r.create_error(
                            "Expected headers value".into(),
                            r.source.len()..r.source.len(),
                        ));
                    }
                }
                2 => {
                    body = Some(parse_body_value(r)?);
                }
                _ => {
                    let span = if let Some(t) = r.cur() {
                        t.span.clone()
                    } else {
                        r.source.len()..r.source.len()
                    };
                    return Err(r.create_error(
                        "Too many positional parameters (max 3: url, headers, body)".into(),
                        span,
                    ));
                }
            }
            positional_index += 1;
        }
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation && t.value == PUNC_COMMA {
                r.advance();
            }
        }
    }
    Ok((url, headers, body, headers_var, request_variables))
}

pub(crate) fn parse_request_with_context(
    r: &mut TokenReader,
    file_vars: &[Variable],
    _endpoint_vars: &[Variable],
    endpoint_name: Option<&str>,
    existing_requests: &[crate::syntax::parse_result::RequestWithVariables],
) -> Result<(Request, Vec<Variable>), SyntaxError> {
    let mut ctx = AttributeContext::default();
    let parsers: Vec<&dyn AttributeParser> = vec![
        &MethodAttributeParser,
        &AuthAttributeParser,
        &TimeoutAttributeParser,
    ];
    parse_attributes(r, &parsers, &mut ctx)?;

    expect(
        r,
        |t| t.token_type == TokenType::Keyword && t.value == KW_RQ,
        "Expected 'rq'",
    )?;
    r.advance();
    r.skip_ignorable();
    let name_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let name = name_tok.value.clone();

    // Check for duplicate request name
    // If endpoint_name is present, the full name is "endpoint/name"
    // If not, it's just "name"
    let full_name = if let Some(ep) = endpoint_name {
        format!("{ep}/{name}")
    } else {
        name.clone()
    };

    if existing_requests
        .iter()
        .any(|r| r.request.name == full_name)
    {
        return Err(r.create_error_with_file(
            format!("Duplicate request definition: '{full_name}'"),
            name_tok.span.clone(),
        ));
    }

    let method = ctx
        .method
        .unwrap_or_else(|| HttpMethod::from_str(&name.to_lowercase()).unwrap_or(HttpMethod::GET));
    r.advance();
    r.skip_ignorable();
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
        format!("Expected '{PUNC_LPAREN}'"),
    )?;
    r.advance();
    r.skip_ignorable();
    let (url, headers, body, headers_var, request_variables) =
        parse_constructor_params(r, file_vars)?;
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
        format!("Expected '{PUNC_RPAREN}'"),
    )?;
    r.advance();

    r.skip_ignorable();
    let vars = request_variables;
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI,
        format!("Expected '{PUNC_SEMI}'"),
    )?;
    r.advance();

    let request = Request {
        name,
        url: url.clone(),
        raw_url: url,
        method,
        headers,
        body,
        headers_var,
        endpoint: endpoint_name.map(|s| s.to_string()),
        auth: ctx.auth,
        timeout: ctx.timeout,
        source_path: Some(r.file_path.to_string_lossy().to_string()),
        related_files: Vec::new(),
    };
    Ok((request, vars))
}
