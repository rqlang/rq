use crate::lint::{line_col, LintContext, LintDiagnostic, LintRule};
use crate::syntax::token::TokenType;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "single_brace_interpolation"
    }

    fn description(&self) -> &'static str {
        "rqlang interpolates with double braces. A single-braced `{name}` inside a string is \
         sent literally and silently produces a wrong request."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (name, offset) in single_brace_occurrences(ctx.source) {
            let single = ["{", &name, "}"].concat();
            let double = ["{{", &name, "}}"].concat();
            let (line, column) = line_col(ctx.source, offset);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "single_brace_interpolation",
                message: format!(
                    "`{single}` uses single braces, which rqlang does not treat as interpolation. \
                     The text `{single}` is sent literally, so the request silently targets the \
                     wrong URL. Use `{double}`, or — for a URL path parameter — pass `{name}` as a \
                     bare identifier with the `[required({name})]` attribute."
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Replace `{single}` with `{double}`, or declare `[required({name})]` and pass \
                     `{name}` as a bare identifier."
                )),
            });
        }
    }
}

fn single_brace_occurrences(source: &str) -> Vec<(String, usize)> {
    let Ok(tokens) = crate::syntax::tokenize(source) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for token in tokens {
        if token.token_type != TokenType::String {
            continue;
        }
        let bytes = token.value.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'{' {
                index += 1;
                continue;
            }
            if bytes.get(index + 1) == Some(&b'{') {
                index += 2;
                continue;
            }
            if index > 0 && bytes[index - 1] == b'$' {
                index += 1;
                continue;
            }
            match read_identifier(bytes, index + 1) {
                Some((name, end)) if bytes.get(end) == Some(&b'}') => {
                    found.push((name, token.span.start + index));
                    index = end + 1;
                }
                _ => index += 1,
            }
        }
    }
    found
}

fn read_identifier(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let first = bytes.get(start)?;
    if !first.is_ascii_alphabetic() && *first != b'_' {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    Some((
        String::from_utf8_lossy(&bytes[start..end]).into_owned(),
        end,
    ))
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn has_rule(src: &str) -> bool {
        lint(src, None, None)
            .diagnostics
            .iter()
            .any(|d| d.rule == "single_brace_interpolation")
    }

    #[test]
    fn flags_single_brace_in_url() {
        let src = "ep widgets(\"http://x/widgets\") {\n    rq get(\"/{widget_id}\");\n}\n";
        let target = lint(src, Some("draft.rq"), None);
        let diagnostic = target
            .diagnostics
            .iter()
            .find(|d| d.rule == "single_brace_interpolation")
            .expect("expected single_brace_interpolation diagnostic");
        assert_eq!(diagnostic.line, 2);
        assert_eq!(diagnostic.column, 14);
        assert!(diagnostic.message.contains("{{widget_id}}"));
        assert!(diagnostic
            .suggested_fix
            .as_ref()
            .expect("fix")
            .contains("[required(widget_id)]"));
    }

    #[test]
    fn does_not_flag_double_brace_interpolation() {
        let src = "let widget_id = \"1\";\nrq get(\"http://x/widgets/{{widget_id}}\");\n";
        assert!(!has_rule(src));
    }

    #[test]
    fn does_not_flag_json_literal_body() {
        let src = "rq post_thing(\"http://x/things\", body: ${\"name\": \"alice\"});\n";
        assert!(!has_rule(src));
    }

    #[test]
    fn does_not_flag_braces_in_a_comment() {
        let src = "// see {widget_id} in the docs\nrq get(\"http://x/widgets/1\");\n";
        assert!(!has_rule(src));
    }

    #[test]
    fn does_not_flag_empty_braces() {
        let src = "rq get(\"http://x/widgets/{}\");\n";
        assert!(!has_rule(src));
    }

    #[test]
    fn flags_single_brace_in_a_header_value() {
        let src = "rq get(\"http://x/widgets\", headers: [\"authorization: Bearer {token}\"]);\n";
        assert!(has_rule(src));
    }

    #[test]
    fn flags_each_occurrence_in_one_string() {
        let src = "rq get(\"http://x/{tenant}/widgets/{widget_id}\");\n";
        let count = lint(src, None, None)
            .diagnostics
            .iter()
            .filter(|d| d.rule == "single_brace_interpolation")
            .count();
        assert_eq!(count, 2);
    }
}
