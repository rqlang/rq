use crate::lint::{LintContext, LintDiagnostic, LintRule};
use crate::syntax::variable_context::VariableValue;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "let_default_instead_of_required"
    }

    fn description(&self) -> &'static str {
        "A `let` holding a hardcoded resource identifier that is interpolated as the trailing \
         path segment of a request URL is runtime input with a default. Use `[required(name)]` \
         and a bare identifier in URL position instead."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for var in &ctx.rq_file.file_variables {
            let VariableValue::String(value) = &var.value else {
                continue;
            };
            if looks_like_json(value) {
                continue;
            }
            if !looks_like_resource_identifier(&var.name) {
                continue;
            }
            if !is_used_as_trailing_path_parameter(&var.name, ctx) {
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

fn looks_like_resource_identifier(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "id"
        || lower == "uuid"
        || lower.ends_with("_id")
        || lower.ends_with("_uuid")
        || lower.ends_with("_guid")
}

fn is_used_as_trailing_path_parameter(name: &str, ctx: &LintContext) -> bool {
    ctx.rq_file.requests.iter().any(|r| {
        let url = r.request.raw_url.trim();
        let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');
        let Some(last_segment) = path.rsplit('/').next() else {
            return false;
        };
        if last_segment == path {
            return false;
        }
        is_interpolation_of(last_segment, name)
    })
}

fn is_interpolation_of(segment: &str, name: &str) -> bool {
    segment
        .strip_prefix("{{")
        .and_then(|inner| inner.strip_suffix("}}"))
        .is_some_and(|inner| inner.trim() == name)
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

    fn flags_rule(src: &str) -> bool {
        lint(src, None, None)
            .diagnostics
            .iter()
            .any(|d| d.rule == "let_default_instead_of_required")
    }

    #[test]
    fn does_not_flag_a_constant_in_a_middle_path_segment() {
        assert!(
            !flags_rule(
                "let api_version = \"v1\";\nrq list(\"https://x/{{api_version}}/users\");\n"
            ),
            "an api version is a constant, not caller-supplied input"
        );
    }

    #[test]
    fn does_not_flag_a_base_url_prefix() {
        assert!(
            !flags_rule("let base_url = \"http://x\";\nrq list(\"{{base_url}}/users\");\n"),
            "the documented base URL pattern must stay clean"
        );
    }

    #[test]
    fn does_not_flag_a_trailing_segment_that_is_not_an_identifier() {
        assert!(
            !flags_rule("let resource = \"users\";\nrq list(\"http://x/{{resource}}\");\n"),
            "a collection name in URL position is not a resource identifier"
        );
    }

    #[test]
    fn does_not_flag_an_identifier_used_outside_the_trailing_segment() {
        assert!(!flags_rule(
            "let user_id = \"1\";\nrq list(\"http://x/users/{{user_id}}/orders\");\n"
        ));
    }

    #[test]
    fn flags_a_path_param_followed_by_a_query_string() {
        assert!(flags_rule(
            "let user_id = \"1\";\nrq get_user(\"http://x/users/{{user_id}}?v=1\");\n"
        ));
    }

    #[test]
    fn flags_a_spaced_interpolation() {
        assert!(flags_rule(
            "let user_id = \"1\";\nrq get_user(\"http://x/users/{{ user_id }}\");\n"
        ));
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
