use crate::lint::{LintContext, LintDiagnostic, LintRule};

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "absolute_import_path"
    }

    fn description(&self) -> &'static str {
        "Import paths must be relative to the importing file, never absolute."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (idx, line_str) in ctx.source.lines().enumerate() {
            let Some((col, path)) = find_import_path(line_str) else {
                continue;
            };
            if !path.starts_with('/') {
                continue;
            }
            out.push(LintDiagnostic {
                severity: "error",
                rule: "absolute_import_path",
                message: format!(
                    "Import path `{path}` is absolute. Use a relative path (e.g. `\"shared\"`, \
                     `\"common/envs\"`, `\"../shared\"`); absolute paths make the project \
                     non-portable across machines."
                ),
                line: idx + 1,
                column: col + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Replace `\"{path}\"` with a path relative to the importing file."
                )),
            });
        }
    }
}

fn find_import_path(line: &str) -> Option<(usize, &str)> {
    let trimmed_start = line.len() - line.trim_start().len();
    let rest = &line[trimmed_start..];
    let after_import = rest.strip_prefix("import")?.trim_start();
    let consumed = rest.len() - after_import.len();
    let path_start = trimmed_start + consumed;
    let inner = after_import.strip_prefix('"')?;
    let end = inner.find('"')?;
    let path = &inner[..end];
    Some((path_start + 1, path))
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    #[test]
    fn flags_absolute_import() {
        let src = "import \"/Users/me/shared\";\nrq foo(\"http://x\");\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .any(|d| d.rule == "absolute_import_path"),
            "got: {:?}",
            target.diagnostics
        );
    }

    #[test]
    fn does_not_flag_relative_import() {
        let src = "import \"shared\";\nimport \"../shared\";\nimport \"common/envs\";\nrq foo(\"http://x\");\n";
        let target = lint(src, None, None);
        assert!(
            target
                .diagnostics
                .iter()
                .all(|d| d.rule != "absolute_import_path"),
            "got: {:?}",
            target.diagnostics
        );
    }
}
