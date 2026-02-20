use crate::syntax::{
    error::SyntaxError,
    http_method::HttpMethod,
    keywords::{PUNC_DOLLAR, PUNC_LBRACKET, PUNC_LPAREN, PUNC_RBRACKET, PUNC_RPAREN},
    reader::{expect, TokenReader},
    token::TokenType,
};

#[derive(Default)]
pub struct AttributeContext {
    pub method: Option<HttpMethod>,
    pub auth: Option<String>,
    pub timeout: Option<String>,
}

impl AttributeContext {
    pub fn set_method(&mut self, method: HttpMethod) -> Result<(), String> {
        if self.method.is_some() {
            return Err("Duplicate attribute 'method'".to_string());
        }
        self.method = Some(method);
        Ok(())
    }
    pub fn set_auth(&mut self, auth: String) -> Result<(), String> {
        if self.auth.is_some() {
            return Err("Duplicate attribute 'auth'".to_string());
        }
        self.auth = Some(auth);
        Ok(())
    }
    pub fn set_timeout(&mut self, timeout: String) -> Result<(), String> {
        if self.timeout.is_some() {
            return Err("Duplicate attribute 'timeout'".to_string());
        }
        self.timeout = Some(timeout);
        Ok(())
    }
}

pub trait AttributeParser {
    fn name(&self) -> &str;
    fn parse(&self, r: &mut TokenReader, ctx: &mut AttributeContext) -> Result<(), SyntaxError>;
}

pub fn parse_attributes(
    r: &mut TokenReader,
    parsers: &[&dyn AttributeParser],
    ctx: &mut AttributeContext,
) -> Result<(), SyntaxError> {
    loop {
        r.skip_ignorable();
        if let Some(t) = r.cur() {
            if t.token_type == TokenType::Punctuation && t.value == PUNC_LBRACKET {
                let mut offset = 1;
                let mut attr_name = None;

                while let Some(next) = r.peek(offset) {
                    if matches!(
                        next.token_type,
                        TokenType::Whitespace | TokenType::Newline | TokenType::Comment
                    ) {
                        offset += 1;
                        continue;
                    }
                    if next.token_type == TokenType::Identifier
                        || next.token_type == TokenType::Keyword
                    {
                        attr_name = Some(next.value.clone());
                    }
                    break;
                }

                if let Some(name) = attr_name {
                    if let Some(parser) = parsers.iter().find(|p| p.name() == name) {
                        parser.parse(r, ctx)?;
                        continue;
                    } else {
                        return Err(
                            r.create_error(format!("Unknown attribute: '{name}'"), t.span.clone())
                        );
                    }
                } else {
                    break;
                }
            }
        }
        break;
    }
    Ok(())
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
            |t| t.token_type == TokenType::String,
            "Expected string literal",
        )?;
        let auth_name = auth_name_tok
            .value
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
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

        ctx.set_auth(auth_name)
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
