use super::{
    error::SyntaxError,
    token::{Token, TokenType},
};
use std::path::PathBuf;

#[derive(Clone)]
pub struct TokenReader {
    pub tokens: Vec<Token>,
    pub idx: usize,
    pub file_path: PathBuf,
    pub source: String,
}
impl TokenReader {
    pub fn new(tokens: Vec<Token>, file_path: PathBuf, source: String) -> Self {
        Self {
            tokens,
            idx: 0,
            file_path,
            source,
        }
    }
    pub fn cur(&self) -> Option<&Token> {
        self.tokens.get(self.idx)
    }
    pub fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.idx + offset)
    }
    pub fn advance(&mut self) {
        if self.idx < self.tokens.len() {
            self.idx += 1;
        }
    }
    pub fn is_end(&self) -> bool {
        self.idx >= self.tokens.len()
    }
    pub fn create_error(&self, message: String, span: std::ops::Range<usize>) -> SyntaxError {
        // Default to including the file path.
        self.create_error_with_file(message, span)
    }

    pub fn create_error_no_file(
        &self,
        message: String,
        span: std::ops::Range<usize>,
    ) -> SyntaxError {
        self.create_error_with_file(message, span)
    }

    pub fn create_error_with_file(
        &self,
        message: String,
        span: std::ops::Range<usize>,
    ) -> SyntaxError {
        let (line, column) = self.get_line_col(span.start);
        SyntaxError::with_file(
            message,
            line,
            column,
            span,
            self.file_path.to_string_lossy().to_string(),
        )
    }

    pub fn get_line_col(&self, pos: usize) -> (usize, usize) {
        if pos > self.source.len() {
            return (1, 1);
        }
        let prefix = &self.source[..pos];
        let line = prefix.matches('\n').count() + 1;
        let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let column = prefix[last_line_start..].chars().count() + 1;
        (line, column)
    }

    pub fn skip_ignorable(&mut self) {
        loop {
            let mut progressed = false;
            if let Some(t) = self.cur() {
                match t.token_type {
                    TokenType::Whitespace | TokenType::Newline | TokenType::Comment => {
                        self.advance();
                        progressed = true;
                    }
                    _ => {}
                }
            }
            if !progressed {
                break;
            }
        }
    }
    pub fn is_keyword(&self, kw: &str) -> bool {
        self.cur()
            .map(|t| t.token_type == TokenType::Keyword && t.value == kw)
            .unwrap_or(false)
    }
}

pub fn make_error<S: Into<String>>(r: &TokenReader, t: &Token, msg: S) -> SyntaxError {
    let msg = msg.into();

    // Attempt to improve error location for "missing separator" cases
    // by backtracking to the earliest Newline after the last valid token.
    let mut best_span = t.span.clone();

    // We assume t is the current token at r.idx.
    if r.idx < r.tokens.len() && r.tokens[r.idx].span == t.span {
        let mut i = r.idx;
        let mut candidate_span = None;
        while i > 0 {
            i -= 1;
            let tok = &r.tokens[i];
            match tok.token_type {
                TokenType::Newline => {
                    candidate_span = Some(tok.span.clone());
                }
                TokenType::Whitespace | TokenType::Comment => {
                    // Continue scanning backwards
                }
                _ => {
                    // Found a meaningful token, stop scanning
                    break;
                }
            }
        }
        if let Some(s) = candidate_span {
            best_span = s;
            return r.create_error_with_file(msg, best_span);
        }
    }

    r.create_error(msg, best_span)
}

pub fn expect<F, S>(r: &mut TokenReader, pred: F, msg: S) -> Result<Token, SyntaxError>
where
    F: Fn(&Token) -> bool,
    S: Into<String>,
{
    let msg = msg.into();
    if let Some(t) = r.cur() {
        if pred(t) {
            return Ok(t.clone());
        }
        return Err(make_error(r, t, msg));
    }

    // EOF case
    let span = if r.tokens.is_empty() {
        0..0
    } else {
        // Attempt to backtrack for EOF too
        let len = r.tokens.len();
        let mut best_span = None;
        let mut i = len;
        while i > 0 {
            i -= 1;
            let tok = &r.tokens[i];
            match tok.token_type {
                TokenType::Newline => {
                    // If we end with specific tokens, maybe the error is "Missing X"
                    // and we want to point to the end of the previous line.
                    best_span = Some(tok.span.clone());
                }
                TokenType::Whitespace | TokenType::Comment => {
                    // Continue scanning backwards
                }
                _ => {
                    // Found a meaningful token.
                    // If we found a Newline before this, use that Newline.
                    // If not, use the end of this token?
                    // Actually, if we found a Newline, we prefer that over "end of token" for "Missing ;"
                    // But for "Unexpected EOF", end of token is better.
                    // The "Missing ;" error specifically benefits from the Newline location.
                    break;
                }
            }
        }

        if let Some(s) = best_span {
            s
        } else {
            let last = r.tokens.last().unwrap();
            last.span.end..last.span.end
        }
    };
    Err(r.create_error_with_file(msg, span))
}
