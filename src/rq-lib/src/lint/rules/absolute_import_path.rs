use crate::lint::{line_col, significant_tokens, LintContext, LintDiagnostic, LintRule};
use crate::syntax::keywords::KW_IMPORT;
use crate::syntax::token::TokenType;

pub struct Rule;

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "absolute_import_path"
    }

    fn description(&self) -> &'static str {
        "Import paths must be relative to the importing file, never absolute."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (offset, path) in import_paths(ctx.source) {
            if !is_absolute_import(path) {
                continue;
            }
            let (line, column) = line_col(ctx.source, offset);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "absolute_import_path",
                message: format!(
                    "Import path `{path}` is absolute. Use a relative path (e.g. `\"shared\"`, \
                     `\"common/envs\"`, `\"../shared\"`); absolute paths make the project \
                     non-portable across machines."
                ),
                line,
                column,
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

fn import_paths(source: &str) -> Vec<(usize, &str)> {
    let tokens = significant_tokens(source);
    let mut found = Vec::new();
    for window in tokens.windows(2) {
        let [keyword, value] = window else {
            continue;
        };
        if keyword.token_type != TokenType::Keyword || keyword.value != KW_IMPORT {
            continue;
        }
        if value.token_type != TokenType::String || value.value.len() < 2 {
            continue;
        }
        let start = value.span.start + 1;
        let end = value.span.start + value.value.len() - 1;
        found.push((start, &source[start..end]));
    }
    found
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

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, None, None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "absolute_import_path")
            .collect()
    }

    #[test]
    fn does_not_flag_an_import_inside_a_block_comment() {
        let src = "/*\nimport \"/tmp/shared\";\n*/\nrq foo(\"http://x\");\n";
        let target = diagnostics_for(src);
        assert!(target.is_empty(), "got: {target:?}");
    }

    #[test]
    fn does_not_flag_an_import_inside_a_line_comment() {
        let src = "// import \"/tmp/shared\";\nrq foo(\"http://x\");\n";
        let target = diagnostics_for(src);
        assert!(target.is_empty(), "got: {target:?}");
    }

    #[test]
    fn flags_a_real_import_next_to_a_commented_out_one() {
        let src = "/* import \"/old/shared\"; */\nimport \"/new/shared\";\nrq foo(\"http://x\");\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("/new/shared"));
        assert_eq!(target[0].line, 2);
        assert_eq!(target[0].column, 9);
    }

    #[test]
    fn flags_a_single_quoted_absolute_import() {
        assert_eq!(diagnostics_for("import '/tmp/shared';\n").len(), 1);
    }

    #[test]
    fn does_not_flag_an_identifier_import() {
        assert!(diagnostics_for("import shared;\n").is_empty());
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
