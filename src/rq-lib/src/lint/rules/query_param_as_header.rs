use crate::lint::{
    endpoint_children, query_params, request_own_headers, LintContext, LintDiagnostic, LintRule,
};
use crate::syntax::parse_result::{EndpointDefinition, Request};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "query_param_as_header"
    }

    fn description(&self) -> &'static str {
        "`rq` has no `qs` parameter — its second positional argument is the header map. A query \
         parameter passed there is sent as an HTTP header and never reaches the query string."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (endpoint, children) in endpoint_children(ctx) {
            let known = known_query_keys(endpoint, &children);
            for request in &children {
                for (key, value) in request_own_headers(request, endpoint) {
                    if !known.iter().any(|k| k.eq_ignore_ascii_case(&key)) {
                        continue;
                    }
                    out.push(diagnostic(ctx, endpoint, request, &key, &value));
                }
            }
        }
    }
}

fn known_query_keys(endpoint: &EndpointDefinition, children: &[&Request]) -> Vec<String> {
    let mut keys: Vec<String> = children
        .iter()
        .flat_map(|request| query_params(&request.raw_url))
        .map(|(key, _)| key)
        .collect();
    if let Some(qs) = endpoint.qs.as_deref() {
        keys.extend(query_params(&format!("?{qs}")).into_iter().map(|(k, _)| k));
    }
    keys
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
    LintDiagnostic {
        severity: "error",
        rule: "query_param_as_header",
        message: format!(
            "`rq {bare}` passes `$[\"{key}\": \"{value}\"]` in its second positional argument. \
             For `rq` the positional arguments are `url`, `headers`, `body` — there is no `qs` \
             parameter — so rqlang sends this as the HTTP header `{key}: {value}` and the query \
             string `?{key}={value}` never reaches the server. `{key}` is used as a real query \
             parameter elsewhere under `ep {ep_name}`, so this was meant to be one. Only `ep` \
             takes `qs`: declare `ep {ep_name}(..., qs: \"{key}={value}\")` once and remove the \
             map from `rq {bare}`."
        ),
        line: request.line + 1,
        column: request.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Add `qs: \"{key}={value}\"` to `ep {ep_name}(...)` and delete the \
             `$[\"{key}\": \"{value}\"]` argument from `rq {bare}`."
        )),
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
            .filter(|d| d.rule == "query_param_as_header")
            .collect()
    }

    #[test]
    fn flags_a_query_param_map_when_a_sibling_uses_the_key_as_a_query_string() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"v\": \"1\"]);\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 5);
        assert!(target[0].message.contains("HTTP header `v: 1`"));
        assert!(target[0].message.contains("never reaches the server"));
        assert!(target[0].message.contains("qs: \"v=1\""));
    }

    #[test]
    fn flags_a_map_duplicating_a_query_param_the_endpoint_already_declares() {
        let src = "ep users(\"http://x/users\", qs: \"v=1\") {\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"v\": \"1\"]);\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
    }

    #[test]
    fn does_not_flag_a_genuine_header() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"X-Trace-Id\": \"abc\"]);\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_headers_inherited_from_the_endpoint() {
        let src = "ep users(\"http://x/users\", headers: $[\"v\": \"1\"]) {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq post(body: ${});\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "the endpoint's own headers are not the request's mistake"
        );
    }

    #[test]
    fn does_not_flag_when_no_sibling_treats_the_key_as_a_query_param() {
        let src = "ep users(\"http://x/users\") {\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"v\": \"1\"]);\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "with no corroborating evidence the map is taken at face value as a header"
        );
    }
}
