use super::{
    attributes::{
        parse_attributes, AttributeContext, AttributeParser, AuthAttributeParser,
        TimeoutAttributeParser,
    },
    parse_trait::Parse,
    request::parse_request_with_context,
    utils::{
        can_parse_attributed, claim_parameter_slot, parse_headers_array, parse_string_value,
        parse_url_value, peek_slot_value, record_binding, SlotTracker,
    },
    variable::parse_variable_declaration,
};
use crate::syntax::fs::Fs;
use crate::syntax::{
    error::SyntaxError,
    keywords::{
        KW_EP, KW_LET, KW_RQ, OP_GT, OP_LT, PUNC_COMMA, PUNC_LBRACE, PUNC_LBRACKET, PUNC_LPAREN,
        PUNC_RBRACE, PUNC_RPAREN, PUNC_SEMI,
    },
    parse_result::{EndpointDefinition, ParseResult},
    reader::{expect, TokenReader},
    token::TokenType,
    types::{ParameterSlot, SlotBinding, SlotValue},
};

pub struct EndpointParser;
impl Parse for EndpointParser {
    fn can_parse(&self, r: &TokenReader) -> bool {
        can_parse_attributed(r, KW_EP)
    }
    fn parse(
        &self,
        r: &mut TokenReader,
        result: &mut ParseResult,
        _fs: &dyn Fs,
    ) -> Result<(), SyntaxError> {
        let (mut ep, ep_def, ep_locs) =
            parse_endpoint_with_context(r, &result.requests, &result.endpoints)?;
        for (name, file, line, character) in ep_locs {
            result
                .required_variable_locations
                .entry(name)
                .or_insert((file, line, character));
        }
        result.requests.append(&mut ep);
        result.endpoints.insert(ep_def.name.clone(), ep_def);
        Ok(())
    }
}

pub struct EndpointConstructorParams {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub headers_var: Option<String>,
    pub qs: Option<String>,
    pub slot_bindings: Vec<SlotBinding>,
}

pub fn parse_endpoint_constructor_params(
    r: &mut TokenReader,
    allow_empty_url: bool,
) -> Result<EndpointConstructorParams, SyntaxError> {
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut headers_var: Option<String> = None;
    let mut qs: Option<String> = None;
    let mut slot_bindings = Vec::new();
    let mut slots = SlotTracker::new(["url", "headers", "qs"]);
    loop {
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation
                && (t.value == PUNC_RPAREN || t.value == PUNC_LBRACE)
            {
                break;
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
                parse_endpoint_headers(r, &mut headers, &mut headers_var, &mut slot_bindings)?;
            }
            _ => {
                record_binding(r, &mut slot_bindings, ParameterSlot::QueryString);
                let raw_qs = parse_string_value(r, "", ParameterSlot::QueryString)?;
                qs = Some(raw_qs.strip_prefix('?').unwrap_or(&raw_qs).to_string());
            }
        }
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation && t.value == PUNC_COMMA {
                r.advance();
            }
        }
    }
    Ok(EndpointConstructorParams {
        url,
        headers,
        headers_var,
        qs,
        slot_bindings,
    })
}

fn parse_endpoint_headers(
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

type EndpointParseResult = (
    Vec<crate::syntax::parse_result::RequestWithVariables>,
    EndpointDefinition,
    Vec<(String, String, usize, usize)>,
);

pub(crate) fn parse_endpoint_with_context(
    r: &mut TokenReader,
    existing_requests: &[crate::syntax::parse_result::RequestWithVariables],
    existing_endpoints: &std::collections::HashMap<String, EndpointDefinition>,
) -> Result<EndpointParseResult, SyntaxError> {
    let declaration_start = r.cur().map(|t| t.span.start);
    let mut ctx = AttributeContext::default();
    let parsers: Vec<&dyn AttributeParser> = vec![&AuthAttributeParser, &TimeoutAttributeParser];
    parse_attributes(r, &parsers, &["method", "required"], &mut ctx)?;

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
    let (line_1, col_1) = r.get_line_col(name_tok.span.start);
    let ep_line = line_1.saturating_sub(1);
    let ep_character = col_1.saturating_sub(1);
    let ep_declaration_line = declaration_start
        .map(|start| r.get_line_col(start).0.saturating_sub(1))
        .unwrap_or(ep_line);

    if existing_endpoints.contains_key(&ep_name) {
        return Err(r.create_error_with_file(
            format!("Duplicate endpoint definition: '{ep_name}'"),
            name_tok.span.clone(),
        ));
    }

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

    let empty_params = || EndpointConstructorParams {
        url: String::new(),
        headers: Vec::new(),
        headers_var: None,
        qs: None,
        slot_bindings: Vec::new(),
    };
    let params = match r.cur() {
        Some(t) if t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN => {
            r.advance();
            r.skip_ignorable();
            let parsed = parse_endpoint_constructor_params(r, parent_ep.is_some())?;
            expect(
                r,
                |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
                format!("Expected '{PUNC_RPAREN}'"),
            )?;
            r.advance();
            r.skip_ignorable();
            parsed
        }
        _ => empty_params(),
    };
    let mut base_url = params.url;
    let mut ep_headers = params.headers;
    let mut ep_headers_var = params.headers_var;
    let mut ep_qs = params.qs;
    let mut ep_slot_bindings = params.slot_bindings;

    let mut endpoint_variables = Vec::new();
    let mut related_files = Vec::new();

    if let Some(parent) = parent_ep {
        ep_slot_bindings.extend(parent.slot_bindings.iter().cloned());
        if !parent.url.is_empty() {
            let is_absolute = base_url.to_lowercase().starts_with("http://")
                || base_url.to_lowercase().starts_with("https://");

            if !is_absolute {
                if base_url.is_empty() {
                    base_url = parent.url;
                } else if parent.url.ends_with('/') && base_url.starts_with('/') {
                    base_url = format!("{}{}", parent.url, &base_url[1..]);
                } else if parent.url.ends_with('/') || base_url.starts_with('/') {
                    base_url = format!("{}{}", parent.url, base_url);
                } else {
                    base_url = format!("{}/{}", parent.url, base_url);
                }
            }
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
        if let Some(p_qs) = &parent.qs {
            match &ep_qs {
                Some(c_qs) => ep_qs = Some(format!("{}&{}", p_qs, c_qs)),
                None => ep_qs = Some(p_qs.clone()),
            }
        }
        if ctx.auth.is_none() {
            ctx.auth = parent.auth;
            ctx.auth_location = parent.auth_location;
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

    let declaration_end = r.cur().map(|t| t.span.start);
    let ep_declaration_end_line = declaration_end
        .map(|start| r.get_line_col(start).0.saturating_sub(1))
        .unwrap_or(ep_line);

    let mut children = Vec::new();
    let mut required_locations: Vec<(String, String, usize, usize)> = Vec::new();

    if let Some(t) = r.cur() {
        if t.token_type == TokenType::Punctuation && t.value == PUNC_SEMI {
            r.advance();
            // Empty body, return early
            let ep_def = EndpointDefinition {
                name: ep_name,
                url: base_url,
                headers: ep_headers,
                headers_var: ep_headers_var,
                slot_bindings: ep_slot_bindings.clone(),
                qs: ep_qs,
                auth: ctx.auth,
                auth_location: ctx.auth_location,
                timeout: ctx.timeout,
                variables: endpoint_variables,
                has_requests: false,
                is_template: true,
                source_path: Some(r.file_path.to_string_lossy().to_string()),
                related_files,
                line: ep_line,
                character: ep_character,
                declaration_line: ep_declaration_line,
                declaration_end_line: ep_declaration_end_line,
            };
            return Ok((children, ep_def, required_locations));
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
            let (var, _, _) = parse_variable_declaration(r)?;
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

            let (mut req, req_vars, req_locs) =
                parse_request_with_context(r, &endpoint_variables, Some(&ep_name), &all_requests)?;
            for loc in req_locs {
                required_locations.push(loc);
            }
            req.slot_bindings.extend(ep_slot_bindings.iter().cloned());
            if req.url.is_empty() {
                req.url = base_url.clone();
            } else if req.url.starts_with('?') || req.url.starts_with('#') {
                req.url = format!("{}{}", base_url, req.url);
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
                    let (base_part, fragment) = match req.url.split_once('#') {
                        Some((base, frag)) => (base.to_string(), format!("#{frag}")),
                        None => (req.url.clone(), String::new()),
                    };
                    let separator = if base_part.contains('?') { "&" } else { "?" };
                    req.url = format!("{base_part}{separator}{qs}{fragment}");
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
                    req.auth_location = ctx.auth_location.clone();
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
        if let Some(t) = r.cur() {
            return Err(r.create_error(format!("Unexpected token '{}'", t.value), t.span.clone()));
        }
        let span = if r.tokens.is_empty() {
            0..0
        } else {
            let last = r.tokens.last().unwrap();
            last.span.end..last.span.end
        };
        return Err(r.create_error("Expected '}'".into(), span));
    }

    let ep_def = EndpointDefinition {
        name: ep_name,
        url: base_url,
        headers: ep_headers,
        headers_var: ep_headers_var,
        slot_bindings: ep_slot_bindings.clone(),
        qs: ep_qs,
        auth: ctx.auth,
        auth_location: ctx.auth_location,
        timeout: ctx.timeout,
        variables: endpoint_variables,
        has_requests: !children.is_empty(),
        is_template: false,
        source_path: Some(r.file_path.to_string_lossy().to_string()),
        related_files,
        line: ep_line,
        character: ep_character,
        declaration_line: ep_declaration_line,
        declaration_end_line: ep_declaration_end_line,
    };

    Ok((children, ep_def, required_locations))
}
