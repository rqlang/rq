use crate::lint::{LintContext, LintDiagnostic, LintRule};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "duplicated_noun_in_ep"
    }

    fn description(&self) -> &'static str {
        "Request name inside an `ep` block duplicates the endpoint's noun. \
         The endpoint name supplies the noun — name the request after the verb alone."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for req_with_vars in &ctx.rq_file.requests {
            let request = &req_with_vars.request;
            let Some(ep_name) = &request.endpoint else {
                continue;
            };
            let bare_name = bare_request_name(&request.name);
            if !name_contains_noun(bare_name, ep_name) {
                continue;
            }
            out.push(LintDiagnostic {
                severity: "error",
                rule: "duplicated_noun_in_ep",
                message: format!(
                    "Request `{bare_name}` inside endpoint `{ep_name}` repeats the endpoint's noun. \
                     Name the request after the verb alone (`list`, `get`, `post`, `put`, \
                     `patch`, `delete`) — the endpoint name already supplies the noun."
                ),
                line: request.line + 1,
                column: request.character + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Rename `{bare_name}` to the verb alone (e.g. `list` for collection GETs, \
                     `get` for single-resource GETs, `post`/`put`/`patch`/`delete` for writes)."
                )),
            });
        }
    }
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

fn name_contains_noun(request_name: &str, ep_name: &str) -> bool {
    let req = request_name.to_lowercase();
    let ep = ep_name.to_lowercase();
    if req.contains(&ep) {
        return true;
    }
    if let Some(singular) = singular_form(&ep) {
        if req.contains(&singular) {
            return true;
        }
    }
    false
}

fn singular_form(name: &str) -> Option<String> {
    if let Some(stripped) = name.strip_suffix("ies") {
        return Some(format!("{stripped}y"));
    }
    if let Some(stripped) = name.strip_suffix("es") {
        return Some(stripped.to_string());
    }
    if let Some(stripped) = name.strip_suffix('s') {
        return Some(stripped.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::lint;

    #[test]
    fn flags_duplicated_plural_noun() {
        let src = "ep widgets(\"http://x/widgets\") { rq get_widgets(); }\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "duplicated_noun_in_ep"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn flags_duplicated_singular_noun() {
        let src = "ep widgets(\"http://x/widgets\") { rq get_widget(\"/{{id}}\"); }\n";
        let target = lint(src, None, None);
        assert!(target
            .diagnostics
            .iter()
            .any(|d| d.rule == "duplicated_noun_in_ep"));
    }

    #[test]
    fn does_not_flag_verb_only_name() {
        let src = "ep widgets(\"http://x/widgets\") { rq list(); }\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "duplicated_noun_in_ep"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn singular_handles_ies_es_s() {
        assert_eq!(singular_form("categories"), Some("category".to_string()));
        assert_eq!(singular_form("widgets"), Some("widget".to_string()));
        assert_eq!(singular_form("user"), None);
    }

    #[test]
    fn bare_request_name_strips_ep_prefix() {
        assert_eq!(bare_request_name("users/get_user"), "get_user");
        assert_eq!(bare_request_name("standalone"), "standalone");
    }
}
