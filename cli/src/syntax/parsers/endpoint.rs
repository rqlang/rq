use super::{
    attributes::{
        parse_attributes, AttributeContext, AttributeParser, AuthAttributeParser,
        TimeoutAttributeParser,
    },
    parse_trait::Parse,
    request::parse_request_with_context,
    utils::{
        can_parse_attributed, check_variable_type, is_headers_like, is_string_like,
        parse_headers_array, parse_string_or_identifier,
    },
    variable::parse_variable_declaration,
};
use crate::syntax::{
    error::SyntaxError,
    keywords::{
        KW_EP, KW_LET, KW_RQ, OP_GT, OP_LT, PUNC_COLON, PUNC_COMMA, PUNC_LBRACE, PUNC_LBRACKET,
        PUNC_LPAREN, PUNC_RBRACE, PUNC_RPAREN, PUNC_SEMI,
    },
    parse_result::{EndpointDefinition, ParseResult},
    reader::{expect, TokenReader},
    token::TokenType,
};

use crate::syntax::variable_context::Variable;

pub struct EndpointParser;
impl Parse for EndpointParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        can_parse_attributed(r, KW_EP)
    }
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError> {
        let (mut ep, ep_def) = parse_endpoint_with_context(
            r,
            &result.file_variables,
            &result.requests,
            &result.endpoints,
        )?;
        result.requests.append(&mut ep);
        result.endpoints.insert(ep_def.name.clone(), ep_def);
        Ok(())
    }
}

pub type EndpointConstructorParams = (
    String,                // url
    Vec<(String, String)>, // headers
    Option<String>,        // headers_var
    Option<String>,        // qs (without leading '?')
);

pub fn parse_endpoint_constructor_params(
    r: &mut TokenReader,
    file_vars: &[Variable],
) -> Result<EndpointConstructorParams, SyntaxError> {
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut headers_var: Option<String> = None;
    let mut qs: Option<String> = None;
    let mut positional_index = 0;
    loop {
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation {
                if t.value == PUNC_RPAREN {
                    break;
                }
                if t.value == PUNC_LBRACE {
                    // If we encounter a brace, we assume the parameter list is over (malformed or not).
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
                "qs" => {
                    if let Some(t) = r.cur() {
                        if t.token_type == TokenType::Identifier {
                            check_variable_type(&t.value, &[is_string_like], file_vars, t, r)?;
                        }
                    }
                    let raw_qs = parse_string_or_identifier(r)?;
                    let cleaned = raw_qs.strip_prefix('?').unwrap_or(&raw_qs).to_string();
                    qs = Some(cleaned);
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
                    if let Some(t) = r.cur() {
                        if t.token_type == TokenType::Identifier {
                            check_variable_type(&t.value, &[is_string_like], file_vars, t, r)?;
                        }
                    }
                    let raw_qs = parse_string_or_identifier(r)?;
                    let cleaned = raw_qs.strip_prefix('?').unwrap_or(&raw_qs).to_string();
                    qs = Some(cleaned);
                }
                _ => {
                    let span = if let Some(t) = r.cur() {
                        t.span.clone()
                    } else {
                        r.source.len()..r.source.len()
                    };
                    return Err(r.create_error(
                        "Too many positional parameters (max 3: url, headers, qs)".into(),
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
    Ok((url, headers, headers_var, qs))
}

pub(crate) fn parse_endpoint_with_context(
    r: &mut TokenReader,
    file_vars: &[crate::syntax::variable_context::Variable],
    existing_requests: &[crate::syntax::parse_result::RequestWithVariables],
    existing_endpoints: &std::collections::HashMap<String, EndpointDefinition>,
) -> Result<
    (
        Vec<crate::syntax::parse_result::RequestWithVariables>,
        EndpointDefinition,
    ),
    SyntaxError,
> {
    let mut ctx = AttributeContext::default();
    let parsers: Vec<&dyn AttributeParser> = vec![&AuthAttributeParser, &TimeoutAttributeParser];
    parse_attributes(r, &parsers, &mut ctx)?;

    expect(
        r,
        |t| t.token_type == TokenType::Keyword && t.value == KW_EP,
        format!("Expected '{KW_EP}'"),
    )?;
    r.advance();
    r.skip_ignorable();
    let name_tok = expect(
        r,
        |t| matches!(t.token_type, TokenType::Identifier),
        "Expected identifier",
    )?;
    let ep_name = name_tok.value.clone();
    r.advance();
    r.skip_ignorable();

    let mut parent_ep: Option<EndpointDefinition> = None;
    if let Some(t) = r.cur() {
        if t.token_type == TokenType::Operator && t.value == OP_LT {
            r.advance();
            r.skip_ignorable();
            let parent_tok = expect(
                r,
                |t| matches!(t.token_type, TokenType::Identifier),
                "Expected parent endpoint identifier",
            )?;
            let parent_name = parent_tok.value.clone();
            if let Some(p) = existing_endpoints.get(&parent_name) {
                if p.has_requests {
                    return Err(r.create_error_with_file(
                        format!("Endpoint '{parent_name}' cannot be used as template because it contains requests"),
                        parent_tok.span.clone(),
                    ));
                }
                parent_ep = Some(p.clone());
            } else {
                return Err(r.create_error_with_file(
                    format!("Unknown template endpoint: {parent_name}"),
                    parent_tok.span.clone(),
                ));
            }
            r.advance();
            r.skip_ignorable();
            expect(
                r,
                |t| t.token_type == TokenType::Operator && t.value == OP_GT,
                format!("Expected '{OP_GT}'"),
            )?;
            r.advance();
            r.skip_ignorable();
        }
    }

    let (mut base_url, mut ep_headers, mut ep_headers_var, mut ep_qs) = if let Some(t) = r.cur() {
        if t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN {
            r.advance();
            r.skip_ignorable();
            let params = parse_endpoint_constructor_params(r, file_vars)?;
            expect(
                r,
                |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
                format!("Expected '{PUNC_RPAREN}'"),
            )?;
            r.advance();
            r.skip_ignorable();
            params
        } else {
            (String::new(), Vec::new(), None, None)
        }
    } else {
        (String::new(), Vec::new(), None, None)
    };

    let mut endpoint_variables = Vec::new();
    let mut related_files = Vec::new();

    if let Some(parent) = parent_ep {
        if base_url.is_empty() {
            base_url = parent.url;
        }
        let mut merged_headers = parent.headers;
        for (k, v) in ep_headers {
            if let Some(i) = merged_headers
                .iter()
                .position(|(pk, _)| pk.eq_ignore_ascii_case(&k))
            {
                merged_headers[i] = (k, v);
            } else {
                merged_headers.push((k, v));
            }
        }
        ep_headers = merged_headers;

        if ep_headers_var.is_none() {
            ep_headers_var = parent.headers_var;
        }
        if ep_qs.is_none() {
            ep_qs = parent.qs;
        }
        if ctx.auth.is_none() {
            ctx.auth = parent.auth;
        }
        if ctx.timeout.is_none() {
            ctx.timeout = parent.timeout;
        }
        endpoint_variables = parent.variables.clone();

        if let Some(src) = &parent.source_path {
            if !related_files.contains(src) {
                related_files.push(src.clone());
            }
        }
        for rf in &parent.related_files {
            if !related_files.contains(rf) {
                related_files.push(rf.clone());
            }
        }
    }

    let mut children = Vec::new();

    if let Some(t) = r.cur() {
        if t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI {
            r.advance();
            // Empty body, return early
            let ep_def = EndpointDefinition {
                name: ep_name,
                url: base_url,
                headers: ep_headers,
                headers_var: ep_headers_var,
                qs: ep_qs,
                auth: ctx.auth,
                timeout: ctx.timeout,
                variables: endpoint_variables,
                has_requests: false,
                source_path: Some(r.file_path.to_string_lossy().to_string()),
                related_files,
            };
            return Ok((children, ep_def));
        }
    }

    expect(
        r,
        |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACE,
        format!("Expected '{PUNC_LBRACE}' or '{PUNC_SEMI}'"),
    )?;
    r.advance();

    loop {
        r.skip_ignorable();
        if let Some(ct) = r.cur() {
            if ct.token_type == TokenType::Punctuation && ct.value == PUNC_RBRACE {
                r.advance();
                break;
            }
        }
        if r.is_keyword(KW_LET) {
            let var = parse_variable_declaration(r)?;
            endpoint_variables.push(var);
            continue;
        }
        if r.is_keyword(KW_RQ)
            || (r
                .cur()
                .map(|t| t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACKET)
                .unwrap_or(false))
        {
            // Combine existing requests with those already parsed in this endpoint
            // This is needed to check for duplicates within the same endpoint
            let mut all_requests = existing_requests.to_vec();
            all_requests.extend(children.clone());

            let (mut req, req_vars) = parse_request_with_context(
                r,
                file_vars,
                &endpoint_variables,
                Some(&ep_name),
                &all_requests,
            )?;
            if req.url.is_empty() {
                req.url = base_url.clone();
            } else if !req.url.starts_with("http://")
                && !req.url.starts_with("https://")
                && !base_url.is_empty()
            {
                req.url = format!(
                    "{}/{}",
                    base_url.trim_end_matches('/'),
                    req.url.trim_start_matches('/')
                );
            }
            if let Some(ref qs) = ep_qs {
                if !qs.is_empty() {
                    if req.url.contains('?') {
                        req.url.push('&');
                        req.url.push_str(qs);
                    } else {
                        req.url.push('?');
                        req.url.push_str(qs);
                    }
                }
            }
            // Note: req.name is already set to "ep_name/req_name" inside parse_request_with_context?
            // No, parse_request_with_context sets it to "req_name".
            // Wait, let's check parse_request_with_context again.
            // It sets: endpoint: endpoint_name.map(|s| s.to_string()),
            // But name is just name.
            // The duplicate check inside parse_request_with_context constructs full_name.

            // However, here we are modifying req.name AFTER parsing.
            req.name = format!("{}/{}", ep_name, req.name);

            let mut merged = ep_headers.clone();
            for (k, v) in req.headers.iter() {
                if let Some(i) = merged.iter().position(|(ek, _)| ek.eq_ignore_ascii_case(k)) {
                    merged[i] = (k.clone(), v.clone());
                } else {
                    merged.push((k.clone(), v.clone()));
                }
            }
            req.headers = merged;
            if req.headers_var.is_none() {
                if let Some(ref hv) = ep_headers_var {
                    req.headers_var = Some(hv.clone());
                }
            }
            if req.auth.is_none() {
                if let Some(ref ea) = ctx.auth {
                    req.auth = Some(ea.clone());
                }
            }
            if req.timeout.is_none() {
                if let Some(ref et) = ctx.timeout {
                    req.timeout = Some(et.clone());
                }
            }

            let current_src = r.file_path.to_string_lossy().to_string();
            if !req.related_files.contains(&current_src) {
                req.related_files.push(current_src);
            }
            for rf in &related_files {
                if !req.related_files.contains(rf) {
                    req.related_files.push(rf.clone());
                }
            }

            children.push(crate::syntax::parse_result::RequestWithVariables {
                request: req,
                endpoint_variables: endpoint_variables.clone(),
                request_variables: req_vars,
            });
            continue;
        }
        if r.is_end() {
            let span = if r.tokens.is_empty() {
                0..0
            } else {
                let last = r.tokens.last().unwrap();
                last.span.end..last.span.end
            };

            return Err(r.create_error("Expected '}'".into(), span));
        }
        r.advance();
    }

    let ep_def = EndpointDefinition {
        name: ep_name,
        url: base_url,
        headers: ep_headers,
        headers_var: ep_headers_var,
        qs: ep_qs,
        auth: ctx.auth,
        timeout: ctx.timeout,
        variables: endpoint_variables,
        has_requests: !children.is_empty(),
        source_path: Some(r.file_path.to_string_lossy().to_string()),
        related_files,
    };

    Ok((children, ep_def))
}
