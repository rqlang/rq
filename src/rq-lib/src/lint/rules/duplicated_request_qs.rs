use crate::lint::{
    endpoint_children, query_params, query_params_on_every_request, LintContext, LintDiagnostic,
    LintRule,
};
use crate::syntax::parse_result::{EndpointDefinition, Request};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "duplicated_request_qs"
    }

    fn description(&self) -> &'static str {
        "The same query parameter written into every request of an endpoint belongs on the \
         endpoint. `ep` takes a `qs` parameter that applies to every child request."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (endpoint, children) in endpoint_children(ctx) {
            if children.len() < 2 {
                continue;
            }
            for (key, value) in query_params_on_every_request(&children) {
                for request in &children {
                    out.push(diagnostic(ctx, endpoint, request, &key, &value));
                }
            }
        }
    }
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
    let (message, suggested_fix) = if endpoint_declares(endpoint, key, value) {
        already_on_endpoint(bare, ep_name, key, value)
    } else {
        hoist_to_endpoint(bare, ep_name, &merged_qs(endpoint, key, value), key, value)
    };
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_request_qs",
        message,
        line: request.line + 1,
        column: request.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(suggested_fix),
    }
}

fn hoist_to_endpoint(
    bare: &str,
    ep_name: &str,
    merged: &str,
    key: &str,
    value: &str,
) -> (String, String) {
    (
        format!(
            "`rq {bare}` writes the query parameter `{key}={value}` into its own URL, and so does \
             every other request under `ep {ep_name}`. An `ep` takes a `qs` parameter that is \
             appended to every child request, including the ones whose URL is a path parameter — \
             declare it once as `ep {ep_name}(..., qs: \"{merged}\")` and drop `?{key}={value}` \
             from each request."
        ),
        format!(
            "Add `qs: \"{merged}\"` to `ep {ep_name}(...)` and remove `?{key}={value}` from \
             `rq {bare}`."
        ),
    )
}

fn already_on_endpoint(bare: &str, ep_name: &str, key: &str, value: &str) -> (String, String) {
    (
        format!(
            "`rq {bare}` writes the query parameter `{key}={value}` into its own URL, but \
             `ep {ep_name}` already declares it in `qs`, so the request sends it twice. Drop \
             `?{key}={value}` from each request under `ep {ep_name}`."
        ),
        format!("Remove `?{key}={value}` from `rq {bare}`."),
    )
}

fn endpoint_declares(endpoint: &EndpointDefinition, key: &str, value: &str) -> bool {
    endpoint.qs.as_deref().is_some_and(|qs| {
        query_params(&format!("?{qs}")).contains(&(key.to_string(), value.to_string()))
    })
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
    fn flags_a_query_parameter_carried_by_every_one_of_three_siblings() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq list_admins(\"?v=1&role=admin\");\n\n    \
                   rq post(\"?v=1\", body: ${});\n}\n";
        assert_eq!(diagnostics_for(src).len(), 3);
    }

    #[test]
    fn flags_a_repeated_pair_once_per_request() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1&v=1\");\n\n    \
                   rq post(\"?v=1\", body: ${});\n}\n";
        assert_eq!(diagnostics_for(src).len(), 2);
    }

    #[test]
    fn reports_a_query_parameter_the_endpoint_already_declares_as_sent_twice() {
        let src = "ep users(\"http://x/users\", qs: \"v=1\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq post(\"?v=1\", body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0].message.contains("sends it twice"),
            "got: {}",
            target[0].message
        );
    }

    #[test]
    fn does_not_flag_the_only_request_of_an_endpoint() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?page=2\");\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "a lone request's query string is its own data"
        );
    }

    #[test]
    fn does_not_flag_a_query_parameter_missing_from_a_sibling() {
        let src = "ep jobs(\"http://x/jobs\") {\n    \
                   rq list(\"?top=1\");\n\n    \
                   rq list_by_status(\"?top=1&status=failed\");\n\n    \
                   [required(job_id)]\n    \
                   rq get(job_id);\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "hoisting onto the ep would add the parameter to a request that does not send it"
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
