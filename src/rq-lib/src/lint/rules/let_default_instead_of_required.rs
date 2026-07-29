use crate::lint::{LintContext, LintDiagnostic, LintRule};
use crate::syntax::variable_context::VariableValue;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "let_default_instead_of_required"
    }

    fn description(&self) -> &'static str {
        "A `let` whose value is a primitive string and is used as a URL interpolation is a \
         runtime value with a hardcoded default. Use `[required(name)]` on the consuming \
         request instead."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for var in &ctx.rq_file.file_variables {
            let VariableValue::String(value) = &var.value else {
                continue;
            };
            if looks_like_json(value) {
                continue;
            }
            if !is_used_in_request_url(&var.name, ctx) {
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
                rule: "let_default_instead_of_required",
                message: format!(
                    "`let {name}` holds a hardcoded default that is interpolated into a request URL. \
                     This is a runtime value masquerading as a constant — declare it with `[required({name})]` \
                     on the consuming request and remove this `let`.",
                    name = var.name
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Remove `let {name} = …;` and add `[required({name})]` to each request that \
                     interpolates `{{{{{name}}}}}` in its URL.",
                    name = var.name
                )),
            });
        }
    }
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn is_used_in_request_url(name: &str, ctx: &LintContext) -> bool {
    let pattern_tight = format!("{{{{{name}}}}}");
    let pattern_spaced = format!("{{{{ {name} }}}}");
    ctx.rq_file.requests.iter().any(|r| {
        let url = r.request.raw_url.trim();
        if url == pattern_tight || url == pattern_spaced {
            return false;
        }
        url.contains(&pattern_tight) || url.contains(&pattern_spaced)
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_let_used_as_path_param() {
        let src = "let user_id = \"1\";\nrq get_user(\"http://x/users/{{user_id}}\");\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "let_default_instead_of_required"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_let_not_used_in_url() {
        let src = "let base_url = \"http://x\";\nrq foo(base_url);\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "let_default_instead_of_required"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_json_let() {
        let src = "let payload = \"{\\\"k\\\":1}\";\nrq foo(\"http://x\", body: payload);\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .all(|d| d.rule != "let_default_instead_of_required"));
    }
}
