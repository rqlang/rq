use crate::lint::{
    endpoint_children, merged_endpoint_qs, query_params, query_params_on_every_request,
    request_own_headers, LintContext, LintDiagnostic, LintRule,
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
                    let fix = fix_for(endpoint, &children, request, &key, &value);
                    out.push(diagnostic(ctx, endpoint, request, &key, &value, fix));
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

enum Fix {
    AlreadyOnEndpoint { in_own_url: bool },
    AlreadyInUrl,
    HoistToEndpoint { merged_qs: String },
    WriteIntoUrl,
}

fn fix_for(
    endpoint: &EndpointDefinition,
    children: &[&Request],
    request: &Request,
    key: &str,
    value: &str,
) -> Fix {
    let pair_matches = |(k, v): &(String, String)| k.eq_ignore_ascii_case(key) && v == value;
    let in_own_url = query_params(&request.raw_url).iter().any(pair_matches);
    let on_endpoint = endpoint
        .qs
        .as_deref()
        .is_some_and(|qs| query_params(&format!("?{qs}")).iter().any(pair_matches));
    if on_endpoint {
        return Fix::AlreadyOnEndpoint { in_own_url };
    }
    let siblings: Vec<&Request> = children
        .iter()
        .copied()
        .filter(|child| !std::ptr::eq(*child, request))
        .collect();
    let on_every_sibling = !siblings.is_empty()
        && query_params_on_every_request(&siblings)
            .iter()
            .any(pair_matches);
    if on_every_sibling {
        Fix::HoistToEndpoint {
            merged_qs: merged_endpoint_qs(endpoint, key, value),
        }
    } else if in_own_url {
        Fix::AlreadyInUrl
    } else {
        Fix::WriteIntoUrl
    }
}

fn diagnostic(
    ctx: &LintContext,
    endpoint: &EndpointDefinition,
    request: &Request,
    key: &str,
    value: &str,
    fix: Fix,
) -> LintDiagnostic {
    let bare = bare_request_name(&request.name);
    let ep_name = &endpoint.name;
    let (advice, suggested_fix) = advice(&fix, bare, ep_name, key, value);
    LintDiagnostic {
        severity: "error",
        rule: "query_param_as_header",
        message: format!(
            "`rq {bare}` passes `$[\"{key}\": \"{value}\"]` in its second positional argument. \
             For `rq` the positional arguments are `url`, `headers`, `body` — there is no `qs` \
             parameter — so rqlang sends this as the HTTP header `{key}: {value}` and the query \
             string `?{key}={value}` never reaches the server. `{key}` is used as a real query \
             parameter elsewhere under `ep {ep_name}`, so this was meant to be one. {advice}"
        ),
        line: request.line + 1,
        column: request.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(suggested_fix),
    }
}

fn advice(fix: &Fix, bare: &str, ep_name: &str, key: &str, value: &str) -> (String, String) {
    let drop_map = format!("delete the `$[\"{key}\": \"{value}\"]` argument from `rq {bare}`");
    match fix {
        Fix::AlreadyOnEndpoint { in_own_url: false } => (
            format!(
                "`ep {ep_name}` already sends `{key}={value}` through its `qs`, so remove the map \
                 from `rq {bare}`."
            ),
            format!("Delete the `$[\"{key}\": \"{value}\"]` argument from `rq {bare}`."),
        ),
        Fix::AlreadyOnEndpoint { in_own_url: true } => (
            format!(
                "`ep {ep_name}` already sends `{key}={value}` through its `qs`, so remove the map \
                 and drop `?{key}={value}` from the URL of `rq {bare}`."
            ),
            format!("Drop `?{key}={value}` from the URL of `rq {bare}` and {drop_map}."),
        ),
        Fix::AlreadyInUrl => (
            format!(
                "The URL of `rq {bare}` already sends `{key}={value}`, so remove the map from \
                 `rq {bare}`."
            ),
            format!("Delete the `$[\"{key}\": \"{value}\"]` argument from `rq {bare}`."),
        ),
        Fix::HoistToEndpoint { merged_qs } => (
            format!(
                "Every other request under `ep {ep_name}` sends it, and only `ep` takes `qs`: \
                 declare `ep {ep_name}(..., qs: \"{merged_qs}\")` once, drop `?{key}={value}` \
                 from the URL of every request under `ep {ep_name}` and remove the map from \
                 `rq {bare}`."
            ),
            format!(
                "Set `qs: \"{merged_qs}\"` on `ep {ep_name}(...)`, drop `?{key}={value}` from the \
                 URL of every request under `ep {ep_name}` and {drop_map}."
            ),
        ),
        Fix::WriteIntoUrl => (
            format!(
                "Not every request under `ep {ep_name}` sends it, so it does not belong in the \
                 endpoint's `qs`: write `?{key}={value}` into the URL string of `rq {bare}` (a path \
                 parameter `id` becomes `\"/{{{{id}}}}?{key}={value}\"`) and remove the map."
            ),
            format!("Write `?{key}={value}` into the URL of `rq {bare}` and {drop_map}."),
        ),
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
    fn suggests_the_request_url_when_a_sibling_does_not_send_the_parameter() {
        let src = "ep jobs(\"http://x/jobs\") {\n    \
                   rq list(\"?top=1\");\n\n    \
                   rq post(body: ${});\n\n    \
                   [required(job_id)]\n    \
                   rq get(job_id, $[\"top\": \"1\"]);\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(
            !target[0].message.contains("qs: \"top=1\""),
            "hoisting would add the parameter to `post`: {}",
            target[0].message
        );
        assert!(target[0].message.contains("\"/{{id}}?top=1\""));
    }

    #[test]
    fn does_not_suggest_adding_a_qs_the_endpoint_already_declares() {
        let src = "ep users(\"http://x/users\", qs: \"v=1\") {\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"v\": \"1\"]);\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0].message.contains("already sends `v=1`"),
            "got: {}",
            target[0].message
        );
    }

    #[test]
    fn tells_a_request_whose_url_carries_the_pair_to_drop_it_when_hoisting() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq post(\"?v=1\", $[\"v\": \"1\"], body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0].message.contains("from the URL of every request"),
            "got: {}",
            target[0].message
        );
    }

    #[test]
    fn merges_the_suggested_qs_with_the_existing_endpoint_qs() {
        let src = "ep users(\"http://x/users\", qs: \"page=1\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   [required(user_id)]\n    \
                   rq get(user_id, $[\"v\": \"1\"]);\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0].message.contains("qs: \"page=1&v=1\""),
            "got: {}",
            target[0].message
        );
    }

    #[test]
    fn only_asks_to_remove_the_map_when_the_request_url_already_sends_the_pair() {
        let src = "ep users(\"http://x/users\") {\n    \
                   rq list(\"?v=1\");\n\n    \
                   rq other();\n\n    \
                   rq post(\"?v=1\", $[\"v\": \"1\"], body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target[0]
                .message
                .contains("already sends `v=1`, so remove the map"),
            "got: {}",
            target[0].message
        );
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
