use std::ops::Range;

use super::{
    attributes::{
        parse_attributes, AttributeContext, AttributeParser, AuthAttributeParser,
        ConfirmAttributeParser, DescriptionAttributeParser, MethodAttributeParser,
        RequiredAttributeParser, TimeoutAttributeParser,
    },
    parse_trait::Parse,
    utils::{
        binding_position, can_parse_attributed, claim_parameter_slot, parse_headers_array,
        parse_string_value, parse_url_value, peek_slot_value, record_binding,
        starts_top_level_statement, SlotTracker,
    },
};
use crate::syntax::fs::Fs;
use crate::syntax::{
    error::SyntaxError,
    http_method::HttpMethod,
    keywords::{
        KW_RQ, PUNC_COMMA, PUNC_DOLLAR, PUNC_LBRACE, PUNC_LPAREN, PUNC_RBRACE, PUNC_RPAREN,
        PUNC_SEMI,
    },
    parse_result::{ParseResult, Request, RequestScope, RequestWithVariables},
    reader::{expect, TokenReader},
    token::TokenType,
    types::{ParameterSlot, SlotBinding, SlotValue, ValueType},
    variable_context::Variable,
};

pub struct RequestParser;
impl Parse for RequestParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        can_parse_attributed(r, KW_RQ)
    }
    fn parse(
        &self,
        r: &mut TokenReader,
        result: &mut ParseResult,
        _fs: &dyn Fs,
    ) -> Result<(), SyntaxError> {
        let context = RequestContext {
            endpoint_variables: &[],
            endpoint_name: None,
            endpoint_header: None,
            starts_statement: &starts_top_level_statement,
        };
        if let Some((req, req_vars)) =
            parse_scoped_request(r, &context, &result.requests, &mut result.request_scopes)?
        {
            result.requests.push(RequestWithVariables {
                request: req,
                endpoint_variables: Vec::new(),
                request_variables: req_vars,
            });
        }
        Ok(())
    }
}

pub fn parse_body_value(r: &mut TokenReader) -> Result<(String, SlotValue), SyntaxError> {
    let Some(val) = r.cur().cloned() else {
        return Err(r.create_error("Expected body value".into(), r.source.len()..r.source.len()));
    };
    match val.token_type {
        TokenType::String => Ok((
            parse_string_value(r, " ", ParameterSlot::Body)?,
            SlotValue::Literal(ValueType::String),
        )),
        TokenType::Identifier => {
            let slot_value = peek_slot_value(r).unwrap_or(SlotValue::Literal(ValueType::String));
            Ok((parse_string_value(r, " ", ParameterSlot::Body)?, slot_value))
        }
        TokenType::Punctuation if val.value == PUNC_DOLLAR => {
            Ok((parse_json_literal(r)?, SlotValue::Literal(ValueType::Json)))
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
}

fn parse_json_literal(r: &mut TokenReader) -> Result<String, SyntaxError> {
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

pub struct ConstructorParams {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub headers_var: Option<String>,
    pub variables: Vec<Variable>,
    pub slot_bindings: Vec<SlotBinding>,
}

pub fn parse_constructor_params(
    r: &mut TokenReader,
    allow_empty_url: bool,
) -> Result<ConstructorParams, SyntaxError> {
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut body = None;
    let mut headers_var: Option<String> = None;
    let mut slot_bindings = Vec::new();
    let mut slots = SlotTracker::new(["url", "headers", "body"]);
    loop {
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation {
                if t.value == PUNC_RPAREN {
                    break;
                }
                if t.value == PUNC_SEMI || t.value == PUNC_LBRACE {
                    break;
                }
            }
        } else {
            break;
        }
        let slot = claim_parameter_slot(r, &mut slots)?;
        match slot {
            0 => {
                record_binding(r, &mut slot_bindings, ParameterSlot::Url);
                url = parse_url_value(r, allow_empty_url)?;
            }
            1 => {
                parse_headers_parameter(r, &mut headers, &mut headers_var, &mut slot_bindings)?;
            }
            _ => {
                let (line, character) = binding_position(r);
                let (value, slot_value) = parse_body_value(r)?;
                slot_bindings.push(SlotBinding {
                    slot: ParameterSlot::Body,
                    value: slot_value,
                    line,
                    character,
                });
                body = Some(value);
            }
        }
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation && t.value == PUNC_COMMA {
                r.advance();
            }
        }
    }
    Ok(ConstructorParams {
        url,
        headers,
        body,
        headers_var,
        variables: Vec::new(),
        slot_bindings,
    })
}

fn parse_headers_parameter(
    r: &mut TokenReader,
    headers: &mut Vec<(String, String)>,
    headers_var: &mut Option<String>,
    slot_bindings: &mut Vec<SlotBinding>,
) -> Result<(), SyntaxError> {
    let Some(tk) = r.cur().cloned() else {
        return Err(r.create_error(
            "Expected headers value".into(),
            r.source.len()..r.source.len(),
        ));
    };
    if tk.token_type != TokenType::Identifier {
        *headers = parse_headers_array(r, slot_bindings)?;
        return Ok(());
    }
    record_binding(r, slot_bindings, ParameterSlot::Headers);
    match peek_slot_value(r) {
        Some(SlotValue::Variable(name)) => {
            *headers_var = Some(name);
            r.advance();
            Ok(())
        }
        _ => {
            parse_string_value(r, "", ParameterSlot::Headers)?;
            Ok(())
        }
    }
}

pub(crate) struct RequestContext<'a> {
    pub endpoint_variables: &'a [Variable],
    pub endpoint_name: Option<&'a str>,
    pub endpoint_header: Option<Range<usize>>,
    pub starts_statement: &'a dyn Fn(&TokenReader) -> bool,
}

pub(crate) fn parse_scoped_request(
    r: &mut TokenReader,
    context: &RequestContext,
    existing_requests: &[RequestWithVariables],
    scopes: &mut Vec<RequestScope>,
) -> Result<Option<(Request, Vec<Variable>)>, SyntaxError> {
    let start_idx = r.idx;
    let span_start = r.cur().map_or(r.source.len(), |t| t.span.start);
    let source_path = r.file_path.to_string_lossy().to_string();
    match parse_request_with_context(
        r,
        context.endpoint_variables,
        context.endpoint_name,
        existing_requests,
    ) {
        Ok((request, variables)) => {
            let span_end = r
                .idx
                .checked_sub(1)
                .and_then(|last| r.tokens.get(last))
                .map_or(span_start, |t| t.span.end);
            scopes.push(RequestScope {
                source_path,
                span: span_start..span_end,
                endpoint_header: context.endpoint_header.clone(),
                required_variables: request.required_variables.clone(),
            });
            Ok(Some((request, variables)))
        }
        Err(_) if r.lenient => {
            r.idx = start_idx;
            let mut ctx = AttributeContext::default();
            parse_attributes(r, &request_attribute_parsers(), KW_RQ, &mut ctx).ok();
            let span_end = r.skip_statement(context.starts_statement);
            scopes.push(RequestScope {
                source_path,
                span: span_start..span_end,
                endpoint_header: context.endpoint_header.clone(),
                required_variables: ctx.required_variables,
            });
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn request_attribute_parsers() -> Vec<&'static dyn AttributeParser> {
    vec![
        &MethodAttributeParser,
        &AuthAttributeParser,
        &TimeoutAttributeParser,
        &RequiredAttributeParser,
        &DescriptionAttributeParser,
        &ConfirmAttributeParser,
    ]
}

fn parse_request_with_context(
    r: &mut TokenReader,
    _endpoint_vars: &[Variable],
    endpoint_name: Option<&str>,
    existing_requests: &[RequestWithVariables],
) -> Result<(Request, Vec<Variable>), SyntaxError> {
    let mut ctx = AttributeContext::default();
    parse_attributes(r, &request_attribute_parsers(), KW_RQ, &mut ctx)?;

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
    let (line_1, col_1) = r.get_line_col(name_tok.span.start);
    let req_line = line_1.saturating_sub(1);
    let req_character = col_1.saturating_sub(1);

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
    let params = parse_constructor_params(r, endpoint_name.is_some())?;
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
        format!("Expected '{PUNC_RPAREN}'"),
    )?;
    r.advance();

    r.skip_ignorable();
    let vars = params.variables;
    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI,
        format!("Expected '{PUNC_SEMI}'"),
    )?;
    r.advance();

    let file = r.file_path.to_string_lossy().to_string();
    let request = Request {
        name,
        url: params.url.clone(),
        raw_url: params.url,
        method,
        headers: params.headers,
        body: params.body,
        body_type: None,
        headers_var: params.headers_var,
        slot_bindings: params.slot_bindings,
        endpoint: endpoint_name.map(|s| s.to_string()),
        auth: ctx.auth,
        auth_location: ctx.auth_location,
        timeout: ctx.timeout,
        description: ctx.description,
        confirm: ctx.confirm,
        required_variables: ctx.required_variables,
        source_path: Some(file),
        related_files: Vec::new(),
        line: req_line,
        character: req_character,
    };
    Ok((request, vars))
}
