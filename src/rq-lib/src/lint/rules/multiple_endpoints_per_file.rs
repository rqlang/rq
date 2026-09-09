use crate::lint::{local_endpoints, LintContext, LintDiagnostic, LintRule};
use std::path::Path;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "multiple_endpoints_per_file"
    }

    fn description(&self) -> &'static str {
        "A file declaring more than one `ep` should be split so each endpoint lives in its \
         own file named after it."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        let declared = local_endpoints(ctx);
        if declared.len() < 2 {
            return;
        }
        let stem = file_stem(ctx.display_path);
        let keeps_current_file = stem
            .as_deref()
            .is_some_and(|s| declared.iter().any(|e| e.name == s));

        for endpoint in &declared {
            if keeps_current_file && Some(endpoint.name.as_str()) == stem.as_deref() {
                continue;
            }
            out.push(LintDiagnostic {
                severity: "error",
                rule: "multiple_endpoints_per_file",
                message: format!(
                    "This file declares {count} endpoints. Keep one `ep` per file, named after \
                     the endpoint, so each domain is easy to find and files stay small — move \
                     `ep {name}` into its own `{name}.rq` and `import` any shared `env`, `auth` \
                     or `let` definitions from a shared file.",
                    count = declared.len(),
                    name = endpoint.name,
                ),
                line: endpoint.line + 1,
                column: endpoint.character + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Move `ep {name}` into a new file `{name}.rq`.",
                    name = endpoint.name
                )),
            });
        }
    }
}

fn file_stem(display_path: &str) -> Option<String> {
    Path::new(display_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;
    use crate::lint::LintDiagnostic;

    fn diagnostics_for(src: &str, path: Option<&str>) -> Vec<LintDiagnostic> {
        lint(src, path, None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "multiple_endpoints_per_file")
            .collect()
    }

    const TWO_EPS: &str = "ep users(\"http://x/users\") {\n    rq list();\n}\n\n\
                           ep widgets(\"http://x/widgets\") {\n    rq list();\n}\n";

    #[test]
    fn does_not_flag_single_endpoint() {
        let src = "ep users(\"http://x/users\") {\n    rq list();\n}\n";
        assert!(diagnostics_for(src, Some("users.rq")).is_empty());
    }

    #[test]
    fn flags_every_endpoint_when_none_matches_the_file_name() {
        let target = diagnostics_for(TWO_EPS, Some("api.rq"));
        assert_eq!(target.len(), 2);
        assert!(target[0].message.contains("declares 2 endpoints"));
    }

    #[test]
    fn leaves_the_endpoint_matching_the_file_name_in_place() {
        let target = diagnostics_for(TWO_EPS, Some("users.rq"));
        assert_eq!(target.len(), 1);
        assert!(
            target[0].suggested_fix.as_deref()
                == Some("Move `ep widgets` into a new file `widgets.rq`.")
        );
    }

    #[test]
    fn reports_the_endpoint_declaration_position() {
        let target = diagnostics_for(TWO_EPS, Some("users.rq"));
        assert_eq!(target[0].line, 5);
        assert_eq!(target[0].column, 4);
    }

    #[test]
    fn does_not_flag_a_base_endpoint_used_for_extension() {
        let src = "ep base(\"http://x\");\nep users<base>(\"/users\") {\n    rq list();\n}\n";
        assert!(diagnostics_for(src, Some("users.rq")).is_empty());
    }

    #[test]
    fn does_not_count_endpoints_pulled_in_by_import() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("shared.rq"),
            "ep shared_ep(\"http://x/shared\") {\n    rq list();\n}\n",
        )
        .expect("write shared");
        let draft = dir.path().join("users.rq");
        let src = "import \"shared\";\n\nep users(\"http://x/users\") {\n    rq list();\n}\n";
        let target = lint(src, draft.to_str(), None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "multiple_endpoints_per_file")
            .count();
        assert_eq!(target, 0);
    }
}
