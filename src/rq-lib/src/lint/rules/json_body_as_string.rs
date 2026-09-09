use crate::lint::{line_col, significant_tokens, LintContext, LintDiagnostic, LintRule};
use crate::syntax::token::TokenType;
use crate::syntax::variable_context::VariableValue;

pub struct Rule;

const BODY_ARGUMENT: &str = "body";

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "json_body_as_string"
    }

    fn description(&self) -> &'static str {
        "A quoted string containing JSON is a string body, not a JSON body. \
         Use a `${...}` literal or `io.read_file(\"...\")`."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        check_let_with_json_string(ctx, out);
        check_inline_string_body(ctx, out);
    }
}

fn check_let_with_json_string(ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
    for var in &ctx.rq_file.file_variables {
        let VariableValue::String(s) = &var.value else {
            continue;
        };
        if !looks_like_json(s) {
            continue;
        }
        let (line, column) = ctx
            .rq_file
            .let_variable_locations
            .get(&var.name)
            .map(|(_, l, c)| (l + 1, c + 1))
            .unwrap_or((1, 1));
        out.push(LintDiagnostic {
            severity: "error",
            rule: "json_body_as_string",
            message: format!(
                "`let {}` holds a quoted string that looks like JSON. \
                 At runtime this is sent as a string, not JSON, and will break receiving APIs. \
                 Use a JSON literal `${{ ... }}` instead, or `io.read_file(\"…\")` to load from a fixture file.",
                var.name
            ),
            line,
            column,
            file: Some(ctx.display_path.to_string()),
            suggested_fix: Some(format!("let {} = ${{ … JSON object … }};", var.name)),
        });
    }
}

fn check_inline_string_body(ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
    for offset in json_string_bodies(ctx.source) {
        let (line, column) = line_col(ctx.source, offset);
        out.push(LintDiagnostic {
            severity: "error",
            rule: "json_body_as_string",
            message: "Inline body is a quoted string that looks like JSON. \
                      At runtime this is sent as a string, not JSON, and will break receiving APIs. \
                      Use a `${ ... }` literal or `io.read_file(\"…\")`."
                .into(),
            line,
            column,
            file: Some(ctx.display_path.to_string()),
            suggested_fix: Some("Replace `body: \"{...}\"` with `body: ${...}`".into()),
        });
    }
}

fn json_string_bodies(source: &str) -> Vec<usize> {
    let significant = significant_tokens(source);
    let mut found = Vec::new();
    for window in significant.windows(3) {
        let [key, colon, value] = window else {
            continue;
        };
        if key.token_type != TokenType::Identifier || key.value != BODY_ARGUMENT {
            continue;
        }
        if colon.token_type != TokenType::Punctuation || colon.value != ":" {
            continue;
        }
        if value.token_type != TokenType::String {
            continue;
        }
        let Some(content) = string_content(&value.value) else {
            continue;
        };
        if !looks_like_json(content) {
            continue;
        }
        found.push(value.span.start);
    }
    found
}

fn string_content(raw: &str) -> Option<&str> {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return None;
    }
    Some(&raw[1..raw.len() - 1])
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_let_with_json_string() {
        let src = "let payload = \"{\\\"name\\\":\\\"x\\\"}\";\n";
        let target = lint(src, Some("draft.rq"), None);
        assert!(!target.ok);
        assert_eq!(target.diagnostics.len(), 1);
        assert_eq!(target.diagnostics[0].rule, "json_body_as_string");
        assert_eq!(target.diagnostics[0].line, 1);
    }

    #[test]
    fn does_not_flag_let_with_plain_string() {
        let src = "let user_id = \"1\";\n";
        let target = lint(src, None, None);
        assert!(target.ok, "diagnostics: {:?}", target.diagnostics);
    }

    #[test]
    fn flags_inline_string_body_looking_like_json() {
        let src = "rq foo(\"http://x\", body: \"{}\");\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .any(|d| d.rule == "json_body_as_string"));
    }

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, None, None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "json_body_as_string")
            .collect()
    }

    #[test]
    fn does_not_flag_a_body_mentioned_in_a_comment() {
        let src = "// example: body: \"{}\"\nrq foo(\"http://x\", body: ${});\n";
        let target = diagnostics_for(src);
        assert!(target.is_empty(), "got: {target:?}");
    }

    #[test]
    fn flags_a_body_on_the_line_after_the_argument_name() {
        let src = "rq foo(\n    \"http://x\",\n    body:\n        \"{\\\"a\\\":1}\",\n);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 4);
        assert_eq!(target[0].column, 9);
    }

    #[test]
    fn does_not_flag_a_body_key_in_a_map() {
        let src = "rq foo(\"http://x\", headers: $[\"body\": \"{}\"]);\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "a quoted map key is not the body argument: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_json_literal_body() {
        let src = "rq foo(\"http://x\", body: ${});\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "json_body_as_string"),
            "got: {:?}",
            target.diagnostics
        );
    }
}
