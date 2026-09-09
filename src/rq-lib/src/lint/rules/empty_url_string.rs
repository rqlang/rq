use crate::lint::{line_col, LintContext, LintDiagnostic, LintRule};
use crate::syntax::token::{Token, TokenType};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "empty_url_string"
    }

    fn description(&self) -> &'static str {
        "A request whose URL argument is an empty string contributes nothing. \
         Omit the argument instead of passing \"\"."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (name, offset) in empty_url_requests(ctx.source) {
            let (line, column) = line_col(ctx.source, offset);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "empty_url_string",
                message: format!(
                    "Request `{name}` passes an empty URL string. An empty string adds nothing to \
                     the URL — inside an `ep` the endpoint already supplies it. Write `rq {name}();` \
                     instead."
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!("Replace `rq {name}(\"\")` with `rq {name}()`.")),
            });
        }
    }
}

fn empty_url_requests(source: &str) -> Vec<(String, usize)> {
    let Ok(tokens) = crate::syntax::tokenize(source) else {
        return Vec::new();
    };
    let significant: Vec<Token> = tokens
        .into_iter()
        .filter(|t| {
            !matches!(
                t.token_type,
                TokenType::Whitespace | TokenType::Newline | TokenType::Comment
            )
        })
        .collect();

    let mut found = Vec::new();
    for window in significant.windows(4) {
        let [keyword, name, open, url] = window else {
            continue;
        };
        if keyword.token_type != TokenType::Keyword || keyword.value != "rq" {
            continue;
        }
        if name.token_type != TokenType::Identifier {
            continue;
        }
        if open.token_type != TokenType::Punctuation || open.value != "(" {
            continue;
        }
        if url.token_type != TokenType::String || url.value != "\"\"" {
            continue;
        }
        found.push((name.value.clone(), url.span.start));
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn rules_for(src: &str) -> Vec<String> {
        lint(src, None, None)
            .diagnostics
            .into_iter()
            .map(|d| d.rule.to_string())
            .collect()
    }

    #[test]
    fn flags_empty_url_string_inside_endpoint() {
        let src = "ep widgets(\"http://x/widgets\") {\n    rq list(\"\");\n}\n";
        let target = lint(src, Some("draft.rq"), None);
        let diagnostic = target
            .diagnostics
            .iter()
            .find(|d| d.rule == "empty_url_string")
            .expect("expected empty_url_string diagnostic");
        assert_eq!(diagnostic.line, 2);
        assert!(diagnostic.message.contains("rq list();"));
        assert!(diagnostic.suggested_fix.is_some());
    }

    #[test]
    fn does_not_flag_request_with_no_url_argument() {
        let src = "ep widgets(\"http://x/widgets\") {\n    rq list();\n}\n";
        assert!(!rules_for(src).contains(&"empty_url_string".to_string()));
    }

    #[test]
    fn does_not_flag_request_with_real_url() {
        let src = "rq list(\"http://x/widgets\");\n";
        assert!(!rules_for(src).contains(&"empty_url_string".to_string()));
    }

    #[test]
    fn does_not_flag_empty_string_in_a_later_argument() {
        let src = "rq post_thing(\"http://x/things\", body: \"\");\n";
        assert!(!rules_for(src).contains(&"empty_url_string".to_string()));
    }
}
