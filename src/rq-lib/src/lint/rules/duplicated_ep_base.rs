use crate::lint::{
    endpoint_extensions, local_endpoints, EndpointSummary, LintContext, LintDiagnostic, LintRule,
};
use crate::syntax::parse_result::EndpointDefinition;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "duplicated_ep_base"
    }

    fn description(&self) -> &'static str {
        "Endpoints repeating the same base URL or auth provider should extend a shared \
         template endpoint instead of duplicating it."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        let extensions = endpoint_extensions(ctx.source);
        let locals = local_endpoints(ctx);
        let candidates = comparable_endpoints(ctx, &locals, &extensions);
        let templates = available_templates(ctx);

        for endpoint in &locals {
            if extensions.iter().any(|e| e.child == endpoint.name) {
                continue;
            }
            let diagnostic = match matching_template(&templates, &endpoint.url) {
                Some(template) => extend_existing_diagnostic(ctx, endpoint, template),
                None => {
                    let shared: Vec<String> = candidates
                        .iter()
                        .filter(|peer| peer.name != endpoint.name)
                        .filter_map(|peer| describe_overlap(endpoint, peer))
                        .collect();
                    if shared.is_empty() {
                        continue;
                    }
                    extract_template_diagnostic(ctx, endpoint, &shared)
                }
            };
            out.push(diagnostic);
        }
    }
}

fn extend_existing_diagnostic(
    ctx: &LintContext,
    endpoint: &EndpointDefinition,
    template: &EndpointSummary,
) -> LintDiagnostic {
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_ep_base",
        message: format!(
            "A template endpoint `{base}` with base URL `{base_url}` already exists in {file}, \
             but `ep {name}` repeats that base URL instead of extending it. Extend the existing \
             template: `ep {name}<{base}>(\"{suffix}\")`. Do not declare a second template.",
            base = template.name,
            base_url = template.url,
            file = template.file,
            name = endpoint.name,
            suffix = url_suffix(&endpoint.url, &template.url),
        ),
        line: endpoint.line + 1,
        column: endpoint.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Rewrite as `ep {name}<{base}>(\"{suffix}\")` and `import` the file declaring `{base}`.",
            name = endpoint.name,
            base = template.name,
            suffix = url_suffix(&endpoint.url, &template.url),
        )),
    }
}

fn extract_template_diagnostic(
    ctx: &LintContext,
    endpoint: &EndpointDefinition,
    shared: &[String],
) -> LintDiagnostic {
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_ep_base",
        message: format!(
            "`ep {name}` duplicates configuration that already exists elsewhere: {shared}. \
             Extract the shared parts into a template endpoint in a shared file — \
             `ep base(url: \"<common base>\");` — and extend it here with \
             `ep {name}<base>(\"/{name}\")`, so the base URL, headers and auth are \
             declared once.",
            name = endpoint.name,
            shared = shared.join("; "),
        ),
        line: endpoint.line + 1,
        column: endpoint.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Declare `ep base(...)` in a shared file, `import` it, and rewrite this as \
             `ep {name}<base>(\"/{name}\")`.",
            name = endpoint.name
        )),
    }
}

fn available_templates(ctx: &LintContext) -> Vec<EndpointSummary> {
    let mut templates: Vec<EndpointSummary> = ctx
        .rq_file
        .endpoints
        .values()
        .filter(|e| e.is_template)
        .map(|e| EndpointSummary {
            name: e.name.clone(),
            url: e.url.clone(),
            auth: e.auth.clone(),
            own_auth: e.auth.clone(),
            qs: e.qs.clone(),
            file: e.source_path.clone().unwrap_or_default(),
            extends: None,
            is_template: true,
            line: e.line,
            character: e.character,
        })
        .collect();
    for template in ctx.workspace_endpoints.iter().filter(|e| e.is_template) {
        if templates.iter().any(|t| t.name == template.name) {
            continue;
        }
        templates.push(template.clone());
    }
    templates
}

fn matching_template<'a>(
    templates: &'a [EndpointSummary],
    url: &str,
) -> Option<&'a EndpointSummary> {
    templates
        .iter()
        .filter(|t| !t.url.is_empty() && t.url != url)
        .find(|t| {
            url.strip_prefix(t.url.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

fn url_suffix(url: &str, base_url: &str) -> String {
    url.strip_prefix(base_url).unwrap_or(url).to_string()
}

fn comparable_endpoints(
    ctx: &LintContext,
    locals: &[&EndpointDefinition],
    extensions: &[crate::lint::EndpointExtension],
) -> Vec<EndpointSummary> {
    let mut candidates: Vec<EndpointSummary> = locals
        .iter()
        .filter(|e| !extensions.iter().any(|x| x.child == e.name))
        .map(|e| EndpointSummary {
            name: e.name.clone(),
            url: e.url.clone(),
            auth: e.auth.clone(),
            own_auth: e.auth.clone(),
            qs: e.qs.clone(),
            file: ctx.display_path.to_string(),
            extends: None,
            is_template: false,
            line: e.line,
            character: e.character,
        })
        .collect();
    candidates.extend(
        ctx.workspace_endpoints
            .iter()
            .filter(|e| e.extends.is_none() && !e.is_template)
            .cloned(),
    );
    candidates
}

fn describe_overlap(endpoint: &EndpointDefinition, peer: &EndpointSummary) -> Option<String> {
    let prefix = common_url_prefix(&endpoint.url, &peer.url)?;
    let mut reasons = vec![format!("base URL `{prefix}`")];
    if let (Some(auth), Some(peer_auth)) = (&endpoint.auth, &peer.auth) {
        if auth == peer_auth {
            reasons.push(format!("auth provider `{auth}`"));
        }
    }
    Some(format!(
        "shares {} with `ep {}` ({})",
        reasons.join(" and "),
        peer.name,
        peer.file
    ))
}

fn common_url_prefix(left: &str, right: &str) -> Option<String> {
    let (left_scheme, left_rest) = split_scheme(left);
    let (right_scheme, right_rest) = split_scheme(right);
    if left_scheme != right_scheme {
        return None;
    }
    let left_segments: Vec<&str> = left_rest.split('/').collect();
    let right_segments: Vec<&str> = right_rest.split('/').collect();

    let mut common: Vec<&str> = Vec::new();
    for (a, b) in left_segments.iter().zip(right_segments.iter()) {
        if a != b {
            break;
        }
        common.push(a);
    }
    if common.iter().all(|s| s.is_empty()) {
        return None;
    }
    if common.len() >= left_segments.len() || common.len() >= right_segments.len() {
        return None;
    }
    Some(format!("{left_scheme}{}", common.join("/")))
}

fn split_scheme(url: &str) -> (&str, &str) {
    for scheme in ["http://", "https://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            return (scheme, rest);
        }
    }
    ("", url)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn diagnostics_for(src: &str, path: Option<&str>) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, path, None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_base")
            .collect()
    }

    #[test]
    fn flags_two_endpoints_sharing_an_interpolated_base() {
        let src = "ep users(\"{{base_url}}/users\") {\n    rq list();\n}\n\n\
                   ep widgets(\"{{base_url}}/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert_eq!(target.len(), 2);
        assert!(target[0].message.contains("base URL `{{base_url}}`"));
    }

    #[test]
    fn flags_two_endpoints_sharing_a_host() {
        let src = "ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n\n\
                   ep widgets(\"http://localhost:8080/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert!(target[0]
            .message
            .contains("base URL `http://localhost:8080`"));
    }

    #[test]
    fn does_not_flag_two_endpoints_whose_schemes_differ() {
        let src = "ep users(\"http://api.example/users\") {\n    rq list();\n}\n\n\
                   ep widgets(\"https://api.example/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert!(
            target.is_empty(),
            "a shared template would change one endpoint's scheme: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_a_schemeless_url_against_an_absolute_one() {
        let src = "ep users(\"http://api.example/users\") {\n    rq list();\n}\n\n\
                   ep widgets(\"api.example/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert!(target.is_empty(), "got: {target:?}");
    }

    #[test]
    fn does_not_flag_shared_auth_when_base_urls_differ() {
        let src = "[auth(\"tok\")]\nep users(\"http://a/users\") {\n    rq list();\n}\n\n\
                   [auth(\"tok\")]\nep widgets(\"http://b/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert!(
            target.is_empty(),
            "a template carries a url, so unrelated hosts cannot share one: {target:?}"
        );
    }

    #[test]
    fn reports_auth_alongside_a_shared_base_url() {
        let src = "[auth(\"tok\")]\nep users(\"{{base_url}}/users\") {\n    rq list();\n}\n\n\
                   [auth(\"tok\")]\nep widgets(\"{{base_url}}/widgets\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert_eq!(target.len(), 2);
        assert!(target[0].message.contains("base URL `{{base_url}}`"));
        assert!(target[0].message.contains("auth provider `tok`"));
    }

    #[test]
    fn tells_a_third_endpoint_to_extend_the_existing_template() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("shared.rq"),
            "env local {\n    base_url: \"http://localhost:8080\",\n}\n\n\
             ep base(url: \"{{base_url}}\");\n",
        )
        .expect("write shared");
        std::fs::write(
            dir.path().join("users.rq"),
            "import \"shared\";\n\nep users<base>(\"/users\") {\n    rq list();\n}\n",
        )
        .expect("write users");
        let draft = dir.path().join("orders.rq");
        let src = "import \"shared\";\n\nep orders(\"{{base_url}}/orders\") {\n    rq list();\n}\n";
        let target: Vec<_> = lint(src, draft.to_str(), Some(dir.path()))
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_base")
            .collect();
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(
            target[0].message.contains("already exists"),
            "should point at the existing template: {}",
            target[0].message
        );
        assert_eq!(
            target[0].suggested_fix.as_deref(),
            Some(
                "Rewrite as `ep orders<base>(\"/orders\")` and `import` the file declaring `base`."
            )
        );
    }

    #[test]
    fn finds_an_existing_template_that_the_draft_does_not_yet_import() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("shared.rq"),
            "ep base(url: \"http://localhost:8080\");\n",
        )
        .expect("write shared");
        let draft = dir.path().join("orders.rq");
        let src = "ep orders(\"http://localhost:8080/orders\") {\n    rq list();\n}\n";
        let target: Vec<_> = lint(src, draft.to_str(), Some(dir.path()))
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_base")
            .collect();
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("already exists"));
    }

    #[test]
    fn does_not_flag_unrelated_endpoints() {
        let src = "ep users(\"http://a/users\") {\n    rq list();\n}\n\n\
                   ep widgets(\"http://b/widgets\") {\n    rq list();\n}\n";
        assert!(diagnostics_for(src, Some("api.rq")).is_empty());
    }

    #[test]
    fn does_not_flag_a_single_endpoint() {
        let src = "ep users(\"{{base_url}}/users\") {\n    rq list();\n}\n";
        assert!(diagnostics_for(src, Some("users.rq")).is_empty());
    }

    #[test]
    fn does_not_flag_endpoints_that_already_extend_a_base() {
        let src = "ep base(url: \"{{base_url}}\");\n\n\
                   ep users<base>(\"/users\") {\n    rq list();\n}\n\n\
                   ep widgets<base>(\"/widgets\") {\n    rq list();\n}\n";
        assert!(
            diagnostics_for(src, Some("api.rq")).is_empty(),
            "the extracted form must not be re-flagged"
        );
    }

    #[test]
    fn does_not_flag_nested_paths_under_the_same_endpoint() {
        let src = "ep users(\"{{base_url}}/users\") {\n    rq list();\n}\n\n\
                   ep user_admins(\"{{base_url}}/users/admins\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src, Some("api.rq"));
        assert!(
            target.is_empty(),
            "one URL being a prefix of the other is not sibling duplication: {target:?}"
        );
    }

    #[test]
    fn flags_a_draft_sharing_a_base_with_another_workspace_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("widgets.rq"),
            "ep widgets(\"{{base_url}}/widgets\") {\n    rq list();\n}\n",
        )
        .expect("write widgets");
        let draft = dir.path().join("users.rq");
        let src = "ep users(\"{{base_url}}/users\") {\n    rq list();\n}\n";
        let target: Vec<_> = lint(src, draft.to_str(), Some(dir.path()))
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_base")
            .collect();
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("widgets.rq"));
    }
}
