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
    let request_segments = segments(request_name);
    let ep_segments = segments(ep_name);
    if ep_segments.is_empty() || request_segments.len() < ep_segments.len() {
        return false;
    }
    request_segments.windows(ep_segments.len()).any(|window| {
        window
            .iter()
            .zip(ep_segments.iter())
            .all(|(request, ep)| same_noun(request, ep))
    })
}

fn segments(name: &str) -> Vec<String> {
    name.split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_lowercase())
        .collect()
}

fn same_noun(left: &str, right: &str) -> bool {
    left == right
        || singular_form(left).as_deref() == Some(right)
        || singular_form(right).as_deref() == Some(left)
}

fn singular_form(name: &str) -> Option<String> {
    if let Some(stripped) = name.strip_suffix("ies") {
        return Some(format!("{stripped}y"));
    }
    if let Some(stripped) = name.strip_suffix("es") {
        if ends_with_sibilant(stripped) {
            return Some(stripped.to_string());
        }
    }
    if let Some(stripped) = name.strip_suffix('s') {
        return Some(stripped.to_string());
    }
    None
}

fn ends_with_sibilant(stem: &str) -> bool {
    stem.ends_with('s')
        || stem.ends_with('x')
        || stem.ends_with('z')
        || stem.ends_with("ch")
        || stem.ends_with("sh")
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

    fn flags_noun(src: &str) -> bool {
        lint(src, None, None)
            .diagnostics
            .iter()
            .any(|d| d.rule == "duplicated_noun_in_ep")
    }

    #[test]
    fn does_not_flag_a_name_that_merely_contains_the_noun_as_a_substring() {
        assert!(
            !flags_noun("ep users(\"http://x/users\") { rq get_abusers(); }\n"),
            "`abusers` is not the endpoint noun"
        );
        assert!(
            !flags_noun("ep orders(\"http://x/orders\") { rq get_preorders(); }\n"),
            "`preorders` is not the endpoint noun"
        );
    }

    #[test]
    fn flags_a_multi_segment_endpoint_noun() {
        assert!(flags_noun(
            "ep user_profiles(\"http://x/user-profiles\") { rq get_user_profile(); }\n"
        ));
    }

    #[test]
    fn does_not_flag_a_partial_match_of_a_multi_segment_noun() {
        assert!(
            !flags_noun("ep user_profiles(\"http://x/user-profiles\") { rq get_user(); }\n"),
            "only part of the endpoint noun is repeated"
        );
    }

    #[test]
    fn flags_a_plural_request_under_a_singular_endpoint() {
        assert!(flags_noun(
            "ep user(\"http://x/user\") { rq get_users(); }\n"
        ));
    }

    #[test]
    fn flags_a_noun_in_the_middle_of_a_request_name() {
        assert!(flags_noun(
            "ep widgets(\"http://x/widgets\") { rq get_widget_by_id(); }\n"
        ));
    }

    #[test]
    fn singular_handles_ies_es_s() {
        assert_eq!(singular_form("categories"), Some("category".to_string()));
        assert_eq!(singular_form("widgets"), Some("widget".to_string()));
        assert_eq!(singular_form("user"), None);
    }

    #[test]
    fn singular_strips_only_the_s_when_es_is_not_the_plural_marker() {
        assert_eq!(singular_form("profiles"), Some("profile".to_string()));
        assert_eq!(singular_form("notes"), Some("note".to_string()));
    }

    #[test]
    fn singular_strips_es_after_a_sibilant_stem() {
        assert_eq!(singular_form("addresses"), Some("address".to_string()));
        assert_eq!(singular_form("boxes"), Some("box".to_string()));
        assert_eq!(singular_form("matches"), Some("match".to_string()));
    }

    #[test]
    fn bare_request_name_strips_ep_prefix() {
        assert_eq!(bare_request_name("users/get_user"), "get_user");
        assert_eq!(bare_request_name("standalone"), "standalone");
    }
}
