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
        line_col_at(&self.source, pos)
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

pub fn line_col_at(source: &str, pos: usize) -> (usize, usize) {
    let Some(prefix) = source.get(..pos) else {
        return (1, 1);
    };
    let line = prefix.matches('\n').count() + 1;
    let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = prefix
        .get(last_line_start..)
        .map_or(0, |l| l.chars().map(column_width).sum::<usize>())
        + 1;
    (line, column)
}

pub fn offset_at(source: &str, line: usize, column: usize) -> Option<usize> {
    let line_start = match line.checked_sub(1)? {
        0 => 0,
        previous => source.match_indices('\n').nth(previous - 1)?.0 + 1,
    };
    let line_text = source.get(line_start..)?.split('\n').next()?;
    let mut remaining = column.checked_sub(1)?;
    for (index, ch) in line_text.char_indices() {
        if remaining < column_width(ch) {
            return Some(line_start + index);
        }
        remaining -= column_width(ch);
    }
    Some(line_start + line_text.len())
}

#[cfg(feature = "utf16-columns")]
fn column_width(ch: char) -> usize {
    ch.len_utf16()
}

#[cfg(not(feature = "utf16-columns"))]
fn column_width(_: char) -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::{line_col_at, offset_at};

    #[test]
    fn line_col_at_counts_multi_byte_characters_as_one_column() {
        assert_eq!(line_col_at("é\naé b", 7), (2, 4));
    }

    #[test]
    fn line_col_at_inside_a_multi_byte_character_falls_back_to_the_start() {
        assert_eq!(line_col_at("é", 1), (1, 1));
    }

    #[cfg(feature = "utf16-columns")]
    #[test]
    fn line_col_at_counts_a_character_outside_the_bmp_as_two_columns() {
        assert_eq!(line_col_at("a\n😀x", 6), (2, 3));
    }

    #[cfg(feature = "utf16-columns")]
    #[test]
    fn offset_at_counts_a_character_outside_the_bmp_as_two_columns() {
        assert_eq!(offset_at("a\n😀x", 2, 3), Some(6));
    }

    #[cfg(feature = "utf16-columns")]
    #[test]
    fn offset_at_inside_a_surrogate_pair_falls_back_to_the_character_start() {
        assert_eq!(offset_at("😀x", 1, 2), Some(0));
    }

    #[cfg(not(feature = "utf16-columns"))]
    #[test]
    fn line_col_at_counts_a_character_outside_the_bmp_as_one_column() {
        assert_eq!(line_col_at("a\n😀x", 6), (2, 2));
    }

    #[cfg(not(feature = "utf16-columns"))]
    #[test]
    fn offset_at_counts_a_character_outside_the_bmp_as_one_column() {
        assert_eq!(offset_at("a\n😀x", 2, 2), Some(6));
    }

    #[test]
    fn offset_at_past_the_end_of_a_line_stays_on_that_line() {
        assert_eq!(offset_at("ab\ncd", 1, 10), Some(2));
    }

    #[test]
    fn offset_at_a_missing_line_is_none() {
        assert_eq!(offset_at("ab", 3, 1), None);
    }

    #[test]
    fn offset_at_inverts_line_col_at() {
        let source = "é😀\n[😀] x";
        let target: Vec<Option<usize>> = source
            .char_indices()
            .map(|(offset, _)| {
                let (line, column) = line_col_at(source, offset);
                offset_at(source, line, column)
            })
            .collect();
        let expected: Vec<Option<usize>> = source
            .char_indices()
            .map(|(offset, _)| Some(offset))
            .collect();
        assert_eq!(target, expected);
    }
}
