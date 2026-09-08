use crate::lint::{endpoint_children, query_params, LintContext, LintDiagnostic, LintRule};
use crate::syntax::parse_result::{EndpointDefinition, Request};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "duplicated_request_qs"
    }

    fn description(&self) -> &'static str {
        "The same query parameter written into two or more sibling requests belongs on the \
         endpoint. `ep` takes a `qs` parameter that applies to every child request."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (endpoint, children) in endpoint_children(ctx) {
            for (request, key, value) in shared_query_params(&children) {
                out.push(diagnostic(ctx, endpoint, request, &key, &value));
            }
        }
    }
}

fn shared_query_params<'a>(children: &[&'a Request]) -> Vec<(&'a Request, String, String)> {
    let mut found = Vec::new();
    for request in children {
        for (key, value) in query_params(&request.raw_url) {
            let carriers = children
                .iter()
                .filter(|peer| {
                    query_params(&peer.raw_url)
                        .iter()
                        .any(|(peer_key, peer_value)| peer_key == &key && peer_value == &value)
                })
                .count();
            if carriers < 2 {
                continue;
            }
            found.push((*request, key, value));
        }
    }
    found
}

fn diagnostic(
    ctx: &LintContext,
    endpoint: &EndpointDefinition,
    request: &Request,
    key: &str,
    value: &str,
) -> LintDiagnostic {
    let bare = bare_request_name(&request.name);
    let ep_name = &endpoint.name;
    let merged = merged_qs(endpoint, key, value);
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_request_qs",
        message: format!(
            "`rq {bare}` writes the query parameter `{key}={value}` into its own URL, and so does \
             at least one sibling under `ep {ep_name}`. An `ep` takes a `qs` parameter that is \
             appended to every child request, including the ones whose URL is a path parameter — \
             declare it once as `ep {ep_name}(..., qs: \"{merged}\")` and drop `?{key}={value}` \
             from each request."
        ),
        line: request.line + 1,
        column: request.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Add `qs: \"{merged}\"` to `ep {ep_name}(...)` and remove `?{key}={value}` from \
             `rq {bare}`."
        )),
    }
}

fn merged_qs(endpoint: &EndpointDefinition, key: &str, value: &str) -> String {
    let pair = format!("{key}={value}");
    match endpoint.qs.as_deref().filter(|qs| !qs.is_empty()) {
        Some(existing) if !existing.split('&').any(|part| part.trim() == pair) => {
            format!("{existing}&{pair}")
        }
        Some(existing) => existing.to_string(),
        None => pair,
    }
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, Some("users.rq"), None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_request_qs")
            .collect()
    }

    #[test]
    fn flags_a_query_parameter_repeated_across_siblings() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq post(\"?v=1\", body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 2, "got: {target:?}");
        assert!(target[0].message.contains("qs: \"v=1\""));
        assert!(target[0].message.contains("path parameter"));
    }

    #[test]
    fn suggests_merging_into_an_existing_endpoint_qs() {
        let src = "ep users(\"http://x/users\", qs: \"page=1\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq post(\"?v=1\", body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0].message.contains("qs: \"page=1&v=1\""),
            "should merge with the existing qs: {}",
            target[0].message
        );
    }

    #[test]
    fn does_not_flag_a_query_parameter_used_by_a_single_request() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?page=2\");\n\n    \
                   rq post(body: ${});\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "a query string on one child is that request's own data, not endpoint config"
        );
    }

    #[test]
    fn does_not_flag_the_same_key_with_different_values() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?page=1\");\n\n    \
                   rq list_page_two(\"?page=2\");\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "differing values are per-request data, not shared endpoint config"
        );
    }

    #[test]
    fn does_not_flag_a_top_level_request() {
        let src = "rq search_users(\"http://x/users?v=1\");\nrq search_widgets(\"http://x/widgets?v=1\");\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "top-level requests have no endpoint to hoist onto"
        );
    }

    #[test]
    fn does_not_flag_an_endpoint_that_already_declares_the_qs() {
        let src = "ep users(\"http://x/users\", qs: \"v=1\") {\n    \
                   rq list();\n\n    \
                   rq post(body: ${});\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
