use super::{
    error::SyntaxError,
    keywords::ALL_KEYWORDS,
    token::{Token, TokenType},
};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref KEYWORD_REGEX: Regex = {
        let pattern = format!("^({})\\b", ALL_KEYWORDS.join("|"));
        Regex::new(&pattern).unwrap()
    };
    static ref TOKEN_PATTERNS: Vec<(Regex, TokenType)> = vec![
        (Regex::new(r"^(\r\n|\r|\n)").unwrap(), TokenType::Newline),
        (Regex::new(r"^[ \t]+").unwrap(), TokenType::Whitespace),
        (Regex::new(r"^//.*").unwrap(), TokenType::Comment),
        (Regex::new(r"^/\*[\s\S]*?\*/").unwrap(), TokenType::Comment),
        (KEYWORD_REGEX.clone(), TokenType::Keyword),
        (
            Regex::new(r#"^"([^"\\]|\\.)*""#).unwrap(),
            TokenType::String
        ),
        (Regex::new(r"^'([^'\\]|\\.)*'").unwrap(), TokenType::String),
        (
            Regex::new(r"^[0-9]+(\.[0-9]+)?").unwrap(),
            TokenType::Number
        ),
        (
            Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_-]*").unwrap(),
            TokenType::Identifier
        ),
        (
            Regex::new(r"^(\+|-|\*|/|%|=|==|!=|<|>|<=|>=|&&|\|\||!|&|\||\^|<<|>>)").unwrap(),
            TokenType::Operator
        ),
        (
            Regex::new(r"^[{}()\[\];,.:?#]").unwrap(),
            TokenType::Punctuation
        ),
    ];
    static ref UNCLOSED_STRING_PATTERN: Regex = Regex::new(r#"^"([^"\n\\]|\\.)*"#).unwrap();
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, SyntaxError> {
    let mut tokens = Vec::new();
    let mut position = 0;
    let mut remaining = input;
    while !remaining.is_empty() {
        let mut matched = false;
        for (pattern, token_type) in TOKEN_PATTERNS.iter() {
            if let Some(mat) = pattern.find(remaining) {
                if mat.start() == 0 {
                    let val = mat.as_str().to_string();
                    let len = val.len();
                    tokens.push(Token {
                        token_type: token_type.clone(),
                        value: val,
                        span: position..position + len,
                    });
                    position += len;
                    remaining = &remaining[len..];
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            // Check for unclosed string
            if let Some(mat) = UNCLOSED_STRING_PATTERN.find(remaining) {
                if mat.start() == 0 {
                    let (line, column) = get_line_col(input, position);
                    return Err(SyntaxError::new(
                        "Unclosed string literal".into(),
                        line,
                        column,
                        position..position + mat.len(),
                    ));
                }
            }

            let ch = remaining.chars().next().unwrap();
            let len = ch.len_utf8();
            tokens.push(Token {
                token_type: TokenType::Punctuation,
                value: ch.to_string(),
                span: position..position + len,
            });
            position += len;
            remaining = &remaining[len..];
        }
    }
    Ok(tokens)
}

fn get_line_col(input: &str, pos: usize) -> (usize, usize) {
    let prefix = &input[..pos];
    let line = prefix.matches('\n').count() + 1;
    let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    // Be careful with unicode here, but column usually counts chars or bytes.
    // Editors use char offset or UTF-16 offset.
    // For now simple byte offset from BOL is fine or char count.
    // Let's do char count for column.
    let column = prefix[last_line_start..].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unclosed_string() {
        let input = "\"jose\n";
        let result = tokenize(input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().message, "Unclosed string literal");
    }
}
