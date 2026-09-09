use crate::lint::{endpoint_extensions, line_col, LintContext, LintDiagnostic, LintRule};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "base_ep_extension"
    }

    fn description(&self) -> &'static str {
        "An `ep child<base>(…)` chain whose base is used by a single endpoint adds indirection \
         without value. Inline the URL. A base shared by two or more endpoints is the point of \
         the extension form and is left alone."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        let extensions = endpoint_extensions(ctx.source);
        for extension in &extensions {
            let consumers = count_consumers(ctx, &extensions, &extension.base);
            if consumers >= 2 {
                continue;
            }
            let (line, column) = line_col(ctx.source, extension.offset);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "base_ep_extension",
                message: format!(
                    "`ep {child}` extends `{base}`, but `{base}` is used by only this one \
                     endpoint. A one-consumer extension chain adds indirection without value — \
                     inline the URL directly into `ep {child}`. Keep the chain only once a \
                     second endpoint extends the same base.",
                    child = extension.child,
                    base = extension.base,
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Rewrite as `ep {child}(\"<full URL>\") {{ ... }}` without the `<{base}>` chain.",
                    child = extension.child,
                    base = extension.base,
                )),
            });
        }
    }
}

fn count_consumers(
    ctx: &LintContext,
    extensions: &[crate::lint::EndpointExtension],
    base: &str,
) -> usize {
    let local = extensions.iter().filter(|e| e.base == base).count();
    let workspace = ctx
        .workspace_endpoints
        .iter()
        .filter(|e| e.extends.as_deref() == Some(base))
        .count();
    local + workspace
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_ep_with_base_extension_used_once() {
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
    fn does_not_flag_a_base_shared_by_two_endpoints() {
        let src = "ep base(\"http://x\");\n\
                   ep users<base>(\"/users\") {\n    rq list();\n}\n\
                   ep widgets<base>(\"/widgets\") {\n    rq list();\n}\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "base_ep_extension"),
            "a base with two consumers is the point of the extension form: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_a_base_whose_second_consumer_is_another_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("widgets.rq"),
            "ep base(\"http://x\");\nep widgets<base>(\"/widgets\") {\n    rq list();\n}\n",
        )
        .expect("write widgets");
        let draft = dir.path().join("users.rq");
        let src = "ep base(\"http://x\");\nep users<base>(\"/users\") {\n    rq list();\n}\n";
        let target = lint(src, draft.to_str(), Some(dir.path()));
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "base_ep_extension"),
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
