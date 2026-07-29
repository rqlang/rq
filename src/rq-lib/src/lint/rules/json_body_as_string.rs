use crate::lint::{LintContext, LintDiagnostic, LintRule};
use crate::syntax::variable_context::VariableValue;

pub struct Rule;

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
    for (idx, line_str) in ctx.source.lines().enumerate() {
        let Some(col) = find_inline_json_string_body(line_str) else {
            continue;
        };
        out.push(LintDiagnostic {
            severity: "error",
            rule: "json_body_as_string",
            message: "Inline body is a quoted string that looks like JSON. \
                      At runtime this is sent as a string, not JSON, and will break receiving APIs. \
                      Use a `${ ... }` literal or `io.read_file(\"…\")`."
                .into(),
            line: idx + 1,
            column: col,
            file: Some(ctx.display_path.to_string()),
            suggested_fix: Some("Replace `body: \"{...}\"` with `body: ${...}`".into()),
        });
    }
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn find_inline_json_string_body(line: &str) -> Option<usize> {
    let after_body = line.find("body:").map(|i| i + "body:".len())?;
    let rest = &line[after_body..];
    let trim_start = rest.len() - rest.trim_start().len();
    let trimmed = &rest[trim_start..];
    let mut chars = trimmed.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let next = chars.next()?;
    if next != '{' && next != '[' {
        return None;
    }
    Some(after_body + trim_start + 1)
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
