use std::ops::Range;

use super::utils::unescape_string;
use crate::syntax::{
    error::SyntaxError,
    http_method::HttpMethod,
    keywords::{
        KW_ENV, KW_RQ, PUNC_DOLLAR, PUNC_DOT, PUNC_LBRACKET, PUNC_LPAREN, PUNC_RBRACKET,
        PUNC_RPAREN,
    },
    parse_result::AuthLocation,
    reader::{expect, TokenReader},
    token::{Token, TokenType},
    ui_color::UiColor,
};

pub const ATTR_UI_COLOR: &str = "ui.color";

pub struct RequiredVariable {
    pub name: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Default)]
pub struct AttributeContext {
    pub method: Option<HttpMethod>,
    pub auth: Option<String>,
    pub auth_location: Option<AuthLocation>,
    pub timeout: Option<String>,
    pub required_variables: Vec<RequiredVariable>,
    pub color: Option<UiColor>,
}

impl AttributeContext {
    pub fn set_method(&mut self, method: HttpMethod) -> Result<(), String> {
        if self.method.is_some() {
            return Err("Duplicate attribute 'method'".to_string());
        }
        self.method = Some(method);
        Ok(())
    }
    pub fn set_auth(&mut self, auth: String, location: AuthLocation) -> Result<(), String> {
        if self.auth.is_some() {
            return Err("Duplicate attribute 'auth'".to_string());
        }
        self.auth = Some(auth);
        self.auth_location = Some(location);
        Ok(())
    }
    pub fn set_timeout(&mut self, timeout: String) -> Result<(), String> {
        if self.timeout.is_some() {
            return Err("Duplicate attribute 'timeout'".to_string());
        }
        self.timeout = Some(timeout);
        Ok(())
    }
    pub fn set_color(&mut self, color: UiColor) -> Result<(), String> {
        if self.color.is_some() {
            return Err(format!("Duplicate attribute '{ATTR_UI_COLOR}'"));
        }
        self.color = Some(color);
        Ok(())
    }
    pub fn add_required_variable(&mut self, name: String, line: usize, character: usize) {
        self.required_variables.push(RequiredVariable {
            name,
            line,
            character,
        });
    }
}

pub trait AttributeParser {
    fn name(&self) -> &str;
    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError>;
}

const ATTRIBUTE_TARGETS: &[(&str, &str)] = &[
    ("method", KW_RQ),
    ("required", KW_RQ),
    ("timeout", "rq or ep"),
    ("auth", "rq or ep"),
    (ATTR_UI_COLOR, KW_ENV),
];

pub fn parse_attributes(
    r: &mut TokenReader,
    parsers: &[&dyn AttributeParser],
    statement: &str,
    ctx: &mut AttributeContext,
) -> Result<(), SyntaxError> {
    loop {
        r.skip_ignorable();
        let Some(t) = r.cur() else { break };
        if t.token_type != TokenType::Punctuation || t.value != PUNC_LBRACKET {
            break;
        }
        let Some((name, span)) = peek_attribute_name(r) else {
            break;
        };
        if let Some(parser) = parsers.iter().find(|p| p.name() == name) {
            parser.parse(r, ctx)?;
            continue;
        }
        let message = match ATTRIBUTE_TARGETS.iter().find(|(known, _)| *known == name) {
            Some((_, targets)) => format!(
                "Attribute '{name}' is not supported on {statement} statements; use it on {targets} statements instead"
            ),
            None => format!("Unknown attribute: '{name}'"),
        };
        return Err(r.create_error(message, span));
    }
    Ok(())
}

fn peek_attribute_name(r: &TokenReader) -> Option<(String, Range<usize>)> {
    let mut offset = 1;
    while let Some(next) = r.peek(offset) {
        if !matches!(
            next.token_type,
            TokenType::Whitespace | TokenType::Newline | TokenType::Comment
        ) {
            break;
        }
        offset += 1;
    }
    let first = r.peek(offset).filter(|t| is_name_token(t))?;
    let mut name = first.value.clone();
    let mut span = first.span.clone();
    while let (Some(dot), Some(part)) = (r.peek(offset + 1), r.peek(offset + 2)) {
        if dot.token_type != TokenType::Punctuation || dot.value != PUNC_DOT || !is_name_token(part)
        {
            break;
        }
        name.push_str(PUNC_DOT);
        name.push_str(&part.value);
        span.end = part.span.end;
        offset += 2;
    }
    Some((name, span))
}

fn is_name_token(t: &Token) -> bool {
    t.token_type == TokenType::Identifier || t.token_type == TokenType::Keyword
}

pub struct MethodAttributeParser;
impl AttributeParser for MethodAttributeParser {
    fn name(&self) -> &str {
        "method"
    }

    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError> {
        let start_token = r.cur().cloned().ok_or_else(|| {
            r.create_error("Unexpected EOF".into(), r.source.len()..r.source.len())
        })?;
        r.advance();

        r.skip_ignorable();
        let _ = expect(
            r,
            |t| {
                (t.token_type == TokenType::Identifier || t.token_type == TokenType::Keyword)
                    && t.value == "method"
            },
            "Expected 'method'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
            "Expected '('",
        )?;
        r.advance();

        r.skip_ignorable();
        let method_tok = expect(
            r,
            |t| t.token_type == TokenType::Identifier,
            "Expected HTTP method",
        )?;
        let method = HttpMethod::from_str(&method_tok.value)
            .ok_or_else(|| r.create_error("Invalid HTTP method".into(), method_tok.span.clone()))?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
            "Expected ')'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACKET,
            "Expected ']'",
        )?;
        r.advance();

        ctx.set_method(method)
            .map_err(|msg| r.create_error_with_file(msg, start_token.span.clone()))?;
        Ok(())
    }
}

pub struct AuthAttributeParser;
impl AttributeParser for AuthAttributeParser {
    fn name(&self) -> &str {
        "auth"
    }

    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError> {
        let start_token = r.cur().cloned().ok_or_else(|| {
            r.create_error("Unexpected EOF".into(), r.source.len()..r.source.len())
        })?;
        r.advance();

        r.skip_ignorable();
        let _ = expect(
            r,
            |t| {
                (t.token_type == TokenType::Identifier || t.token_type == TokenType::Keyword)
                    && t.value == "auth"
            },
            "Expected 'auth'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
            "Expected '('",
        )?;
        r.advance();

        r.skip_ignorable();
        let auth_name_tok = expect(
            r,
            |t| t.token_type == TokenType::String || t.token_type == TokenType::Identifier,
            "Expected string literal or identifier",
        )?;
        let auth_name = if auth_name_tok.token_type == TokenType::Identifier {
            format!("{{{{{}}}}}", auth_name_tok.value)
        } else {
            unescape_string(&auth_name_tok.value[1..auth_name_tok.value.len() - 1])
        };
        let (auth_line_1, auth_col_1) = r.get_line_col(auth_name_tok.span.start);
        let auth_location = AuthLocation {
            file: r.file_path.to_string_lossy().to_string(),
            line: auth_line_1.saturating_sub(1),
            character: auth_col_1.saturating_sub(1),
        };
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
            "Expected ')'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACKET,
            "Expected ']'",
        )?;
        r.advance();

        ctx.set_auth(auth_name, auth_location)
            .map_err(|msg| r.create_error_with_file(msg, start_token.span.clone()))?;
        Ok(())
    }
}

pub struct TimeoutAttributeParser;
impl AttributeParser for TimeoutAttributeParser {
    fn name(&self) -> &str {
        "timeout"
    }

    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError> {
        let start_token = r.cur().cloned().ok_or_else(|| {
            r.create_error("Unexpected EOF".into(), r.source.len()..r.source.len())
        })?;
        r.advance(); // consume '['

        r.skip_ignorable();
        let _ = expect(
            r,
            |t| {
                (t.token_type == TokenType::Identifier || t.token_type == TokenType::Keyword)
                    && t.value == "timeout"
            },
            "Expected 'timeout'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
            "Expected '('",
        )?;
        r.advance();

        r.skip_ignorable();

        let val_token = r.cur().cloned().ok_or_else(|| {
            r.create_error("Unexpected EOF".into(), r.source.len()..r.source.len())
        })?;

        let timeout_str = if val_token.token_type == TokenType::Number {
            r.advance();
            val_token.value
        } else if val_token.token_type == TokenType::Punctuation && val_token.value == PUNC_DOLLAR {
            r.advance(); // consume $
            let ident = expect(
                r,
                |t| t.token_type == TokenType::Identifier,
                "Expected variable name",
            )?
            .clone();
            r.advance(); // consume ident
            format!("{{{{{}}}}}", ident.value)
        } else {
            return Err(r.create_error(
                "Expected number or variable for timeout".into(),
                val_token.span,
            ));
        };

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
            "Expected ')'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACKET,
            "Expected ']'",
        )?;
        r.advance();

        ctx.set_timeout(timeout_str)
            .map_err(|msg| r.create_error_with_file(msg, start_token.span.clone()))?;
        Ok(())
    }
}

pub struct RequiredAttributeParser;
impl AttributeParser for RequiredAttributeParser {
    fn name(&self) -> &str {
        "required"
    }

    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError> {
        r.advance(); // consume '['

        r.skip_ignorable();
        let _ = expect(
            r,
            |t| {
                (t.token_type == TokenType::Identifier || t.token_type == TokenType::Keyword)
                    && t.value == "required"
            },
            "Expected 'required'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
            "Expected '('",
        )?;
        r.advance();

        r.skip_ignorable();
        let name_tok = expect(
            r,
            |t| t.token_type == TokenType::Identifier,
            "Expected variable name",
        )?
        .clone();
        let (line_1, col_1) = r.get_line_col(name_tok.span.start);
        let line = line_1.saturating_sub(1);
        let character = col_1.saturating_sub(1);
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
            "Expected ')'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACKET,
            "Expected ']'",
        )?;
        r.advance();

        ctx.add_required_variable(name_tok.value, line, character);
        Ok(())
    }
}

pub struct UiColorAttributeParser;
impl AttributeParser for UiColorAttributeParser {
    fn name(&self) -> &str {
        ATTR_UI_COLOR
    }

    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError> {
        let start_token = r.cur().cloned().ok_or_else(|| {
            r.create_error("Unexpected EOF".into(), r.source.len()..r.source.len())
        })?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Identifier && t.value == "ui",
            format!("Expected '{ATTR_UI_COLOR}'"),
        )?;
        r.advance();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_DOT,
            format!("Expected '{ATTR_UI_COLOR}'"),
        )?;
        r.advance();
        expect(
            r,
            |t| t.token_type == TokenType::Identifier && t.value == "color",
            format!("Expected '{ATTR_UI_COLOR}'"),
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_LPAREN,
            "Expected '('",
        )?;
        r.advance();

        r.skip_ignorable();
        let color_tok = expect(
            r,
            |t| t.token_type == TokenType::Identifier,
            format!("Expected color name, one of: {}", UiColor::names()),
        )?;
        let color = UiColor::from_name(&color_tok.value).ok_or_else(|| {
            r.create_error(
                format!(
                    "Unknown color '{}'; expected one of: {}",
                    color_tok.value,
                    UiColor::names()
                ),
                color_tok.span.clone(),
            )
        })?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RPAREN,
            "Expected ')'",
        )?;
        r.advance();

        r.skip_ignorable();
        expect(
            r,
            |t| t.token_type == TokenType::Punctuation && t.value == PUNC_RBRACKET,
            "Expected ']'",
        )?;
        r.advance();

        ctx.set_color(color)
            .map_err(|msg| r.create_error_with_file(msg, start_token.span.clone()))?;
        Ok(())
    }
}
