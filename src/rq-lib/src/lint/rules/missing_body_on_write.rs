use crate::lint::{LintContext, LintDiagnostic, LintRule};
use crate::syntax::http_method::HttpMethod;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "missing_body_on_write"
    }

    fn description(&self) -> &'static str {
        "POST, PUT, and PATCH requests should include a body."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for req_with_vars in &ctx.rq_file.requests {
            let request = &req_with_vars.request;
            let Some(verb) = write_verb(&request.method) else {
                continue;
            };
            let bare = bare_request_name(&request.name);
            if request.body.is_some() {
                continue;
            }
            out.push(LintDiagnostic {
                severity: "error",
                rule: "missing_body_on_write",
                message: format!(
                    "Request `{bare}` is a {verb} but has no `body:` argument. \
                     Write actions should carry a body — add `body: io.read_file(\"<entity>-<verb>.json\")` \
                     or an inline JSON literal `body: ${{ ... }}`."
                ),
                line: request.line + 1,
                column: request.character + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Add `body: io.read_file(\"<entity>-{verb_lower}.json\")` to `rq {bare}(...)`.",
                    verb_lower = verb.to_lowercase()
                )),
            });
        }
    }
}

fn write_verb(method: &HttpMethod) -> Option<&'static str> {
    matches!(
        method,
        HttpMethod::POST | HttpMethod::PUT | HttpMethod::PATCH
    )
    .then(|| method.as_str())
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn flags_missing_body(src: &str) -> bool {
        lint(src, None, None)
            .diagnostics
            .iter()
            .any(|d| d.rule == "missing_body_on_write")
    }

    #[test]
    fn flags_a_post_named_after_the_verb_without_body() {
        assert!(flags_missing_body(
            "ep users(\"http://x/users\") {\n    rq post();\n}\n"
        ));
    }

    #[test]
    fn flags_a_post_declared_by_attribute_without_body() {
        assert!(flags_missing_body(
            "[method(POST)]\nrq post_user(\"http://x/users\");\n"
        ));
    }

    #[test]
    fn does_not_flag_post_with_body() {
        assert!(!flags_missing_body(
            "[method(POST)]\nrq post_user(\"http://x/users\", body: ${});\n"
        ));
    }

    #[test]
    fn does_not_flag_a_get_whose_name_merely_starts_with_a_write_verb() {
        assert!(
            !flags_missing_body("rq post_user(\"http://x/users\");\n"),
            "`post_user` is not a recognised method name, so the request runs as a GET"
        );
    }

    #[test]
    fn does_not_flag_a_request_explicitly_declared_as_get() {
        assert!(
            !flags_missing_body("[method(GET)]\nrq post_user(\"http://x/users\");\n"),
            "an explicit method attribute must win over the request name"
        );
    }

    #[test]
    fn does_not_flag_get_without_body() {
        let src = "rq get_user(\"http://x/users/1\");\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .all(|d| d.rule != "missing_body_on_write"));
    }

    #[test]
    fn does_not_flag_delete_without_body() {
        let src = "rq delete_user(\"http://x/users/1\");\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .all(|d| d.rule != "missing_body_on_write"));
    }
}
