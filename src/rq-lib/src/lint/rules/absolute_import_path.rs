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
            if !is_absolute_import(path) {
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

fn is_absolute_import(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || has_drive_prefix(path)
}

fn has_drive_prefix(path: &str) -> bool {
    let mut chars = path.chars();
    let Some(drive) = chars.next() else {
        return false;
    };
    drive.is_ascii_alphabetic() && chars.next() == Some(':')
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

    fn flags(src: &str) -> bool {
        lint(src, None, None)
            .diagnostics
            .iter()
            .any(|d| d.rule == "absolute_import_path")
    }

    #[test]
    fn flags_a_windows_drive_path() {
        assert!(flags("import \"C:\\\\shared\";\n"), "backslash drive path");
        assert!(flags("import \"C:/shared\";\n"), "forward slash drive path");
    }

    #[test]
    fn flags_a_lowercase_windows_drive_path() {
        assert!(flags("import \"c:\\\\shared\";\n"));
    }

    #[test]
    fn flags_a_windows_rooted_path() {
        assert!(flags("import \"\\\\shared\";\n"));
    }

    #[test]
    fn flags_a_windows_unc_path() {
        assert!(flags("import \"\\\\\\\\server\\\\share\\\\shared\";\n"));
    }

    #[test]
    fn does_not_flag_a_relative_path_containing_a_colon() {
        assert!(
            !flags("import \"shared:v2\";\n"),
            "a colon away from the drive position is not an absolute path"
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
