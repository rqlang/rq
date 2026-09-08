use crate::lint::{
    endpoint_auth_attributes, endpoint_extensions, local_endpoints, query_params, EndpointSummary,
    LintContext, LintDiagnostic, LintRule,
};

pub struct Rule;

struct Extending {
    name: String,
    base: String,
    file: String,
    own_qs: Vec<(String, String)>,
    own_auth: Option<String>,
    line: usize,
    character: usize,
}

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "duplicated_ep_config"
    }

    fn description(&self) -> &'static str {
        "Configuration that a template endpoint already carries, or that two of its children \
         declare identically, belongs on the template. A child re-declaring an inherited `qs` \
         sends the parameter twice."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        let templates = available_templates(ctx);
        let peers = extending_endpoints(ctx, &templates);
        for endpoint in peers.iter().filter(|e| e.file == ctx.display_path) {
            let Some(template) = templates.iter().find(|t| t.name == endpoint.base) else {
                continue;
            };
            check_query_string(ctx, endpoint, template, &peers, out);
            check_auth(ctx, endpoint, template, &peers, out);
        }
    }
}

fn check_query_string(
    ctx: &LintContext,
    endpoint: &Extending,
    template: &EndpointSummary,
    peers: &[Extending],
    out: &mut Vec<LintDiagnostic>,
) {
    let inherited = template_params(template);
    for (key, value) in &endpoint.own_qs {
        if let Some((_, inherited_value)) = inherited.iter().find(|(k, _)| k == key) {
            out.push(redundant_qs_diagnostic(
                ctx,
                endpoint,
                template,
                key,
                value,
                inherited_value,
            ));
            continue;
        }
        let sharing = siblings_declaring(peers, endpoint, |peer| {
            peer.own_qs.iter().any(|(k, v)| k == key && v == value)
        });
        if sharing.is_empty() {
            continue;
        }
        out.push(hoist_diagnostic(
            ctx,
            endpoint,
            template,
            "query string",
            &format!("qs: \"{key}={value}\""),
            &sharing,
        ));
    }
}

fn check_auth(
    ctx: &LintContext,
    endpoint: &Extending,
    template: &EndpointSummary,
    peers: &[Extending],
    out: &mut Vec<LintDiagnostic>,
) {
    let Some(provider) = &endpoint.own_auth else {
        return;
    };
    if template.auth.as_deref() == Some(provider.as_str()) {
        out.push(redundant_auth_diagnostic(ctx, endpoint, template, provider));
        return;
    }
    let sharing = siblings_declaring(peers, endpoint, |peer| {
        peer.own_auth.as_deref() == Some(provider.as_str())
    });
    if sharing.is_empty() {
        return;
    }
    out.push(hoist_diagnostic(
        ctx,
        endpoint,
        template,
        "auth provider",
        &format!("[auth(\"{provider}\")]"),
        &sharing,
    ));
}

fn siblings_declaring<'a>(
    peers: &'a [Extending],
    endpoint: &Extending,
    declares: impl Fn(&Extending) -> bool,
) -> Vec<&'a Extending> {
    peers
        .iter()
        .filter(|peer| peer.name != endpoint.name && peer.base == endpoint.base)
        .filter(|peer| declares(peer))
        .collect()
}

fn redundant_qs_diagnostic(
    ctx: &LintContext,
    endpoint: &Extending,
    template: &EndpointSummary,
    key: &str,
    value: &str,
    inherited: &str,
) -> LintDiagnostic {
    let name = &endpoint.name;
    let base = &endpoint.base;
    let location = template_location(template);
    let message = if value == inherited {
        format!(
            "`ep {name}` declares `qs: \"{key}={value}\"`, but it already inherits `{key}={value}` \
             from the template `{base}`{location}. A template's query string is prepended to its \
             child's rather than replaced, so the request goes out as `?{key}={value}&{key}={value}` \
             — the parameter is sent twice. Remove `qs: \"{key}={value}\"` from `ep {name}`."
        )
    } else {
        format!(
            "`ep {name}` declares `qs: \"{key}={value}\"`, but the template `{base}`{location} \
             already declares `{key}={inherited}`. A child's query string is appended to the \
             template's rather than overriding it, so the request goes out as \
             `?{key}={inherited}&{key}={value}` with `{key}` sent twice and the server free to \
             pick either. rqlang has no per-child override for a query parameter: either drop \
             `{key}` from the template and declare it on each child that needs it, or drop it here."
        )
    };
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_ep_config",
        message,
        line: endpoint.line + 1,
        column: endpoint.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(if value == inherited {
            format!("Remove `{key}={value}` from the `qs` of `ep {name}` — `ep {base}` already supplies it.")
        } else {
            format!(
                "Remove `{key}` from `ep {base}`{location} and declare it per child, or drop \
                 `{key}={value}` from `ep {name}`."
            )
        }),
    }
}

fn redundant_auth_diagnostic(
    ctx: &LintContext,
    endpoint: &Extending,
    template: &EndpointSummary,
    provider: &str,
) -> LintDiagnostic {
    let name = &endpoint.name;
    let base = &endpoint.base;
    let location = template_location(template);
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_ep_config",
        message: format!(
            "`ep {name}` declares `[auth(\"{provider}\")]`, but it already inherits that same \
             provider from the template `{base}`{location}. The attribute changes nothing — a \
             child only needs its own `[auth(...)]` when it authenticates differently from the \
             template. Remove it from `ep {name}`."
        ),
        line: endpoint.line + 1,
        column: endpoint.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Delete the `[auth(\"{provider}\")]` attribute above `ep {name}` — `ep {base}` \
             already applies it."
        )),
    }
}

fn hoist_diagnostic(
    ctx: &LintContext,
    endpoint: &Extending,
    template: &EndpointSummary,
    label: &str,
    declaration: &str,
    sharing: &[&Extending],
) -> LintDiagnostic {
    let name = &endpoint.name;
    let base = &endpoint.base;
    let peers = describe_peers(sharing);
    let location = template_location(template);
    LintDiagnostic {
        severity: "error",
        rule: "duplicated_ep_config",
        message: format!(
            "`ep {name}` declares the {label} `{declaration}`, and so {peers}. Both extend the \
             same template `{base}`, and carrying configuration shared by its children is exactly \
             what a template endpoint is for. Move `{declaration}` onto `ep {base}`{location} and \
             drop it from each child — every endpoint extending `{base}` then inherits it, \
             including ones added later. Keep it on a child only when that child needs a \
             different value from its siblings."
        ),
        line: endpoint.line + 1,
        column: endpoint.character + 1,
        file: Some(ctx.display_path.to_string()),
        suggested_fix: Some(format!(
            "Add `{declaration}` to `ep {base}`{location} and remove it from `ep {name}`."
        )),
    }
}

fn describe_peers(sharing: &[&Extending]) -> String {
    let described: Vec<String> = sharing
        .iter()
        .map(|peer| format!("`ep {}` ({})", peer.name, peer.file))
        .collect();
    match described.len() {
        1 => format!("does {}", described[0]),
        _ => format!("do {}", described.join(", ")),
    }
}

fn template_location(template: &EndpointSummary) -> String {
    if template.file.is_empty() {
        return String::new();
    }
    format!(" in {}", template.file)
}

fn template_params(template: &EndpointSummary) -> Vec<(String, String)> {
    match template.qs.as_deref().filter(|qs| !qs.is_empty()) {
        Some(qs) => query_params(&format!("?{qs}")),
        None => Vec::new(),
    }
}

fn extending_endpoints(ctx: &LintContext, templates: &[EndpointSummary]) -> Vec<Extending> {
    let extensions = endpoint_extensions(ctx.source);
    let auth_attributes = endpoint_auth_attributes(ctx.source);
    let mut found: Vec<Extending> = local_endpoints(ctx)
        .into_iter()
        .filter_map(|endpoint| {
            let extension = extensions.iter().find(|e| e.child == endpoint.name)?;
            let template = templates.iter().find(|t| t.name == extension.base)?;
            Some(Extending {
                name: endpoint.name.clone(),
                base: extension.base.clone(),
                file: ctx.display_path.to_string(),
                own_qs: own_qs(endpoint.qs.as_deref(), template.qs.as_deref()),
                own_auth: auth_attributes
                    .iter()
                    .find(|a| a.endpoint == endpoint.name)
                    .map(|a| a.provider.clone()),
                line: endpoint.line,
                character: endpoint.character,
            })
        })
        .collect();

    for endpoint in ctx.workspace_endpoints {
        let Some(base) = &endpoint.extends else {
            continue;
        };
        let Some(template) = templates.iter().find(|t| &t.name == base) else {
            continue;
        };
        found.push(Extending {
            name: endpoint.name.clone(),
            base: base.clone(),
            file: endpoint.file.clone(),
            own_qs: own_qs(endpoint.qs.as_deref(), template.qs.as_deref()),
            own_auth: endpoint.own_auth.clone(),
            line: endpoint.line,
            character: endpoint.character,
        });
    }
    found
}

fn own_qs(child: Option<&str>, template: Option<&str>) -> Vec<(String, String)> {
    let Some(child) = child.filter(|qs| !qs.is_empty()) else {
        return Vec::new();
    };
    let Some(inherited) = template.filter(|qs| !qs.is_empty()) else {
        return query_params(&format!("?{child}"));
    };
    if child == inherited {
        return Vec::new();
    }
    match child.strip_prefix(&format!("{inherited}&")) {
        Some(rest) => query_params(&format!("?{rest}")),
        None => Vec::new(),
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

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    const SHARED: &str = "env local {\n    base_url: \"http://localhost:8080\",\n}\n\n\
                          ep base(url: \"{{base_url}}\");\n";

    fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content).expect("write");
        }
        dir
    }

    fn diagnostics_in(dir: &tempfile::TempDir, draft: &str) -> Vec<crate::lint::LintDiagnostic> {
        let path = dir.path().join(draft);
        let src = std::fs::read_to_string(&path).expect("read draft");
        lint(&src, path.to_str(), Some(dir.path()))
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_config")
            .collect()
    }

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, Some("api.rq"), None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "duplicated_ep_config")
            .collect()
    }

    #[test]
    fn flags_the_same_qs_on_two_endpoints_extending_one_template() {
        let dir = workspace(&[
            ("shared.rq", SHARED),
            (
                "widgets.rq",
                "import \"shared\";\n\nep widgets<base>(\"/widgets\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
            (
                "users.rq",
                "import \"shared\";\n\nep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
        ]);
        let target = diagnostics_in(&dir, "users.rq");
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("qs: \"v=1\""));
        assert!(target[0].message.contains("`ep widgets`"));
        assert!(target[0].message.contains("shared.rq"));
    }

    #[test]
    fn flags_the_same_auth_on_two_endpoints_extending_one_template() {
        let dir = workspace(&[
            ("shared.rq", SHARED),
            (
                "widgets.rq",
                "import \"shared\";\n\n[auth(\"token_auth\")]\nep widgets<base>(\"/widgets\") {\n    rq list();\n}\n",
            ),
            (
                "users.rq",
                "import \"shared\";\n\n[auth(\"token_auth\")]\nep users<base>(\"/users\") {\n    rq list();\n}\n",
            ),
        ]);
        let target = diagnostics_in(&dir, "users.rq");
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("[auth(\"token_auth\")]"));
    }

    #[test]
    fn flags_a_qs_the_child_already_inherits_from_the_template() {
        let src = "ep base(url: \"http://x\", qs: \"v=1\");\n\n\
                   ep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(
            target[0].message.contains("?v=1&v=1"),
            "must name the doubled parameter: {}",
            target[0].message
        );
        assert!(target[0].message.contains("sent twice"));
    }

    #[test]
    fn flags_a_child_qs_that_conflicts_with_the_inherited_value() {
        let src = "ep base(url: \"http://x\", qs: \"v=1\");\n\n\
                   ep users<base>(\"/users\", qs: \"v=2\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(
            target[0].message.contains("?v=1&v=2"),
            "must show that appending does not override: {}",
            target[0].message
        );
        assert!(target[0].message.contains("no per-child override"));
    }

    #[test]
    fn flags_an_auth_attribute_the_child_already_inherits() {
        let src = "[auth(\"token_auth\")]\nep base(url: \"http://x\");\n\n\
                   [auth(\"token_auth\")]\nep users<base>(\"/users\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("already inherits that same"));
        assert!(target[0]
            .suggested_fix
            .as_ref()
            .expect("fix")
            .contains("Delete the `[auth(\"token_auth\")]` attribute"));
    }

    #[test]
    fn does_not_flag_a_child_overriding_the_template_auth() {
        let src = "[auth(\"token_auth\")]\nep base(url: \"http://x\");\n\n\
                   [auth(\"admin_auth\")]\nep users<base>(\"/users\") {\n    rq list();\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "a child authenticating differently is the documented override"
        );
    }

    #[test]
    fn does_not_confuse_a_request_auth_attribute_with_the_endpoint_one() {
        let src = "[auth(\"token_auth\")]\nep base(url: \"http://x\");\n\n\
                   ep users<base>(\"/users\") {\n    [auth(\"other\")]\n    rq list();\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "the attribute belongs to the request, not the endpoint"
        );
    }

    #[test]
    fn reports_only_the_inherited_parameter_of_a_multi_param_qs() {
        let src = "ep base(url: \"http://x\", qs: \"v=1\");\n\n\
                   ep users<base>(\"/users\", qs: \"v=1&page=2\") {\n    rq list();\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("v=1"));
        assert!(
            !target[0].message.contains("page"),
            "page=2 is the child's own parameter: {}",
            target[0].message
        );
    }

    #[test]
    fn reports_qs_and_auth_separately_when_both_are_duplicated() {
        let dir = workspace(&[
            ("shared.rq", SHARED),
            (
                "widgets.rq",
                "import \"shared\";\n\n[auth(\"token_auth\")]\nep widgets<base>(\"/widgets\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
            (
                "users.rq",
                "import \"shared\";\n\n[auth(\"token_auth\")]\nep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
        ]);
        assert_eq!(diagnostics_in(&dir, "users.rq").len(), 2);
    }

    #[test]
    fn flags_two_endpoints_extending_the_same_template_in_one_file() {
        let src = "ep base(url: \"http://x\");\n\n\
                   ep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n\n\
                   ep widgets<base>(\"/widgets\", qs: \"v=1\") {\n    rq list();\n}\n";
        assert_eq!(diagnostics_for(src).len(), 2);
    }

    #[test]
    fn does_not_flag_differing_query_strings() {
        let dir = workspace(&[
            ("shared.rq", SHARED),
            (
                "widgets.rq",
                "import \"shared\";\n\nep widgets<base>(\"/widgets\", qs: \"v=2\") {\n    rq list();\n}\n",
            ),
            (
                "users.rq",
                "import \"shared\";\n\nep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
        ]);
        assert!(diagnostics_in(&dir, "users.rq").is_empty());
    }

    #[test]
    fn does_not_flag_the_fixed_shape_where_the_template_carries_everything() {
        let dir = workspace(&[
            (
                "shared.rq",
                "env local {\n    base_url: \"http://localhost:8080\",\n}\n\n\
                 [auth(\"token_auth\")]\nep base(url: \"{{base_url}}\", qs: \"v=1\");\n",
            ),
            (
                "widgets.rq",
                "import \"shared\";\n\nep widgets<base>(\"/widgets\") {\n    rq list();\n}\n",
            ),
            (
                "users.rq",
                "import \"shared\";\n\nep users<base>(\"/users\") {\n    rq list();\n}\n",
            ),
        ]);
        assert!(diagnostics_in(&dir, "users.rq").is_empty());
    }

    #[test]
    fn does_not_flag_an_only_child_of_the_template() {
        let dir = workspace(&[
            ("shared.rq", SHARED),
            (
                "users.rq",
                "import \"shared\";\n\nep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n",
            ),
        ]);
        assert!(diagnostics_in(&dir, "users.rq").is_empty());
    }

    #[test]
    fn does_not_flag_endpoints_extending_different_templates() {
        let src = "ep public_base(url: \"http://x\");\n\
                   ep private_base(url: \"http://y\");\n\n\
                   ep users<public_base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n\n\
                   ep widgets<private_base>(\"/widgets\", qs: \"v=1\") {\n    rq list();\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
