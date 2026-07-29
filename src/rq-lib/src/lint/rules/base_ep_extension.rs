use crate::lint::{LintContext, LintDiagnostic, LintRule};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "base_ep_extension"
    }

    fn description(&self) -> &'static str {
        "Endpoints using the `ep child<base>(…)` extension form duplicate the multi-file \
         split pattern at parse level. Inline the URL directly into the consuming `ep`."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (idx, line_str) in ctx.source.lines().enumerate() {
            let trimmed = line_str.trim_start();
            if !trimmed.starts_with("ep ") {
                continue;
            }
            let Some(angle_col) = find_extension_angle(trimmed) else {
                continue;
            };
            let leading = line_str.len() - trimmed.len();
            out.push(LintDiagnostic {
                severity: "error",
                rule: "base_ep_extension",
                message: "Endpoint uses the `ep child<base>(…)` extension form. \
                          Inline the URL directly into the consuming `ep` instead — \
                          extension chains add indirection without value for most cases."
                    .into(),
                line: idx + 1,
                column: leading + angle_col + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(
                    "Rewrite as `ep child(\"<full URL>\") { ... }` without the `<base>` chain."
                        .into(),
                ),
            });
        }
    }
}

fn find_extension_angle(line_after_trim: &str) -> Option<usize> {
    let after_ep = &line_after_trim[3..];
    let mut chars = after_ep.char_indices();
    while let Some((i, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if !is_ident_char(ch) {
            return None;
        }
        let name_start = i;
        let mut last_ident_end = i + ch.len_utf8();
        for (j, c) in chars.by_ref() {
            if is_ident_char(c) {
                last_ident_end = j + c.len_utf8();
                continue;
            }
            if c == '<' {
                return Some(3 + name_start + (last_ident_end - name_start));
            }
            return None;
        }
        return None;
    }
    None
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_ep_with_base_extension() {
        let src = "ep base(\"http://x\");\nep users<base>(\"/users\") {\n    rq list();\n}\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "base_ep_extension"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_inline_ep() {
        let src = "ep users(\"http://x/users\") {\n    rq list();\n}\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "base_ep_extension"),
            "got: {:?}",
            target.diagnostics
        );
    }
}
