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
            let bare = bare_request_name(&request.name);
            let Some(verb) = detect_write_verb(&request.method, bare) else {
                continue;
            };
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

fn detect_write_verb(method: &HttpMethod, bare_name: &str) -> Option<&'static str> {
    if matches!(method, HttpMethod::POST) {
        return Some("POST");
    }
    if matches!(method, HttpMethod::PUT) {
        return Some("PUT");
    }
    if matches!(method, HttpMethod::PATCH) {
        return Some("PATCH");
    }
    let lower = bare_name.to_lowercase();
    if lower.starts_with("post_") || lower == "post" {
        return Some("POST");
    }
    if lower.starts_with("put_") || lower == "put" {
        return Some("PUT");
    }
    if lower.starts_with("patch_") || lower == "patch" {
        return Some("PATCH");
    }
    None
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_post_without_body() {
        let src = "rq post_user(\"http://x/users\");\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "missing_body_on_write"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_post_with_body() {
        let src = "rq post_user(\"http://x/users\", body: ${});\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "missing_body_on_write"),
            "got: {:?}",
            target.diagnostics
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
