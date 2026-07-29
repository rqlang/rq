use crate::lint::{LintContext, LintDiagnostic, LintRule};
use crate::syntax::parse_result::Request;
use std::collections::HashMap;

pub struct Rule;

#[derive(Clone, Copy)]
enum Origin<'a> {
    Draft,
    Workspace { file: &'a str },
}

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "top_level_rq_should_be_ep"
    }

    fn description(&self) -> &'static str {
        "Two or more top-level `rq` statements sharing a URL prefix should be grouped \
         under a single `ep` block. Includes requests from other files in the workspace \
         when a workspace path is provided."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        let mut groups: HashMap<String, Vec<(&Request, Origin)>> = HashMap::new();

        for r in &ctx.rq_file.requests {
            let request = &r.request;
            if request.endpoint.is_some() {
                continue;
            }
            let prefix = url_prefix(&request.raw_url);
            if prefix.is_empty() {
                continue;
            }
            groups
                .entry(prefix)
                .or_default()
                .push((request, Origin::Draft));
        }

        for request in ctx.workspace_requests {
            if request.endpoint.is_some() {
                continue;
            }
            let prefix = url_prefix(&request.raw_url);
            if prefix.is_empty() {
                continue;
            }
            let file = request.source_path.as_deref().unwrap_or("<workspace>");
            groups
                .entry(prefix)
                .or_default()
                .push((request, Origin::Workspace { file }));
        }

        for (prefix, members) in groups {
            if members.len() < 2 {
                continue;
            }
            let workspace_sibling = members.iter().find_map(|(req, origin)| match origin {
                Origin::Workspace { file } => Some((bare_request_name(&req.name), *file)),
                Origin::Draft => None,
            });
            for (req, origin) in &members {
                let Origin::Draft = origin else {
                    continue;
                };
                let bare = bare_request_name(&req.name);
                let message = match workspace_sibling {
                    Some((sibling_name, sibling_file)) if sibling_name != bare => format!(
                        "Top-level request `{bare}` shares URL prefix `{prefix}` with existing \
                         request `{sibling_name}` in `{sibling_file}`. Refactor both into a shared \
                         `ep` (the existing request should move out of `{sibling_file}` or this \
                         draft should include the refactor): `ep <noun>(\"{prefix}\") {{ rq … }}`."
                    ),
                    Some((_, sibling_file)) => format!(
                        "Top-level request `{bare}` shares URL prefix `{prefix}` with an existing \
                         request in `{sibling_file}`. Refactor into a shared `ep`: \
                         `ep <noun>(\"{prefix}\") {{ rq … }}`."
                    ),
                    None => format!(
                        "Top-level request `{bare}` shares URL prefix `{prefix}` with another \
                         top-level request. Group these under a shared `ep` so the base URL \
                         lives in one place: `ep <noun>(\"{prefix}\") {{ rq … }}`."
                    ),
                };
                out.push(LintDiagnostic {
                    severity: "error",
                    rule: "top_level_rq_should_be_ep",
                    message,
                    line: req.line + 1,
                    column: req.character + 1,
                    file: Some(ctx.display_path.to_string()),
                    suggested_fix: Some(format!(
                        "Wrap the sibling requests inside `ep <noun>(\"{prefix}\") {{ … }}` \
                         and rename them to verb-only forms (`list`, `get`, `post`, …)."
                    )),
                });
            }
        }
    }
}

fn url_prefix(url: &str) -> String {
    let stripped = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    let parts: Vec<&str> = stripped.split('/').collect();
    if parts.len() >= 4 && parts[0].ends_with(':') && parts[1].is_empty() {
        return parts[..4].join("/");
    }
    if parts.len() >= 2 {
        return parts[..2].join("/");
    }
    String::new()
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_two_top_level_rq_sharing_prefix() {
        let src = "rq list_users(\"http://x/users\");\nrq get_user(\"http://x/users/1\");\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "top_level_rq_should_be_ep"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_single_top_level_rq() {
        let src = "rq solo(\"http://x/users/1\");\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .all(|d| d.rule != "top_level_rq_should_be_ep"));
    }

    #[test]
    fn does_not_flag_two_unrelated_top_level_rq() {
        let src = "rq users(\"http://x/users\");\nrq widgets(\"http://x/widgets\");\n";
        let target = lint(src, None, None);
        let n = target
            .diagnostics
            .iter()
            .filter(|d| d.rule == "top_level_rq_should_be_ep")
            .count();
        assert_eq!(n, 0, "got: {:?}", target.diagnostics);
    }

    #[test]
    fn does_not_flag_requests_already_in_ep() {
        let src = "ep users(\"http://x/users\") {\n    rq list();\n    rq get(\"/1\");\n}\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .all(|d| d.rule != "top_level_rq_should_be_ep"));
    }

    #[test]
    fn flags_draft_conflicting_with_workspace_request() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("existing.rq"),
            "rq get_widgets(\"http://x/widgets\");\n",
        )
        .expect("write existing");

        let draft = "rq get_widget(\"http://x/widgets/1\");\n";
        let target = lint(draft, Some("draft.rq"), Some(tmp.path()));
        let diag = target
            .diagnostics
            .iter()
            .find(|d| d.rule == "top_level_rq_should_be_ep");
        assert!(
            diag.is_some(),
            "expected workspace-aware diagnostic, got: {:?}",
            target.diagnostics
        );
        let diag = diag.unwrap();
        assert!(
            diag.message.contains("existing.rq"),
            "diagnostic should mention the workspace file: {}",
            diag.message
        );
        assert!(diag.message.contains("get_widgets"));
    }

    #[test]
    fn deduplicates_workspace_requests_pulled_in_via_imports() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("shared.rq"),
            "rq shared_widget(\"http://x/widgets/shared\");\n",
        )
        .expect("write shared");
        std::fs::write(
            tmp.path().join("users.rq"),
            "import \"shared\";\nrq users_widget(\"http://x/widgets/users\");\n",
        )
        .expect("write users");

        let draft = "rq draft_widget(\"http://x/widgets/draft\");\n";
        let target = lint(draft, Some("draft.rq"), Some(tmp.path()));
        let diags: Vec<_> = target
            .diagnostics
            .iter()
            .filter(|d| d.rule == "top_level_rq_should_be_ep")
            .collect();
        assert_eq!(
            diags.len(),
            1,
            "draft request should fire the rule exactly once even though shared_widget is reachable \
             via both shared.rq and users.rq's import. got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_double_count_same_named_request_in_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("existing.rq"),
            "rq get_widget(\"http://x/widgets/1\");\n",
        )
        .expect("write existing");

        let draft = "rq get_widget(\"http://x/widgets/2\");\n";
        let target = lint(draft, Some("draft.rq"), Some(tmp.path()));
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "top_level_rq_should_be_ep"),
            "should treat as update-in-place, not duplicate. got: {:?}",
            target.diagnostics
        );
    }
}
