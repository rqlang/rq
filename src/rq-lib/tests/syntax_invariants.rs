use rq_lib::syntax::{analysis::analyze, tokenize, Fs, RqFile};
use std::path::{Path, PathBuf};

struct NoFs;

impl Fs for NoFs {
    fn read(&self, _path: &Path) -> Result<String, String> {
        Err("no filesystem in syntax invariant tests".into())
    }

    fn resolve_path(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
        Ok(base.join(relative))
    }

    fn exists(&self, _path: &Path) -> bool {
        false
    }

    fn is_file(&self, _path: &Path) -> bool {
        false
    }

    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn read_dir(&self, _dir: &Path) -> Result<Vec<PathBuf>, String> {
        Ok(Vec::new())
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        Ok(path.to_path_buf())
    }
}

const ADVERSARIAL_SOURCES: &[&str] = &[
    "",
    " ",
    "\n",
    "\r\n",
    "\0",
    "\"",
    "'",
    "\"unclosed",
    "\"caf\u{00e9}",
    "let a = \"\u{1f600}\";",
    "rq",
    "rq ",
    "rq (",
    "rq a(",
    "ep",
    "ep a {",
    "env",
    "auth",
    "let",
    "let a",
    "let a =",
    "import",
    "import \"",
    "$[",
    "${",
    "${a",
    "$[\"a\":",
    "//",
    "/*",
    "/* a",
    "*/",
    "let \u{00f1} = 1;",
    "rq \u{4f60}\u{597d}();",
    "\u{feff}rq a();",
    "let a = '\u{00e9}\\'';",
    "rq a(url: \"\u{00e9}\u{00e9}\u{00e9}\"",
    "[method(",
    "[method(POST)]",
    "}",
    ")",
    "]",
    ";;;",
    "let a = \"line\nline\";",
    "ep a() { rq b(); ",
    "rq a(url: 1.2.3);",
    "?#:.,",
];

#[test]
fn every_repository_fixture_holds_the_syntax_invariants() {
    let files = collect_rq_files(&repository_root());
    assert!(files.len() > 200, "the fixture corpus was not found");
    for file in files {
        let label = file.display().to_string();
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("the fixture {label} must be readable UTF-8: {error}"));
        assert_invariants(&source, &label);
    }
}

#[test]
fn adversarial_sources_hold_the_syntax_invariants() {
    for source in ADVERSARIAL_SOURCES {
        assert_invariants(source, &format!("{source:?}"));
    }
}

fn assert_invariants(source: &str, label: &str) {
    match tokenize(source) {
        Ok(tokens) => {
            let mut cursor = 0;
            for token in &tokens {
                assert_eq!(
                    token.span.start, cursor,
                    "gap or overlap between token spans in {label}"
                );
                assert!(
                    source.is_char_boundary(token.span.start)
                        && source.is_char_boundary(token.span.end),
                    "token span off a char boundary in {label}"
                );
                assert_eq!(
                    &source[token.span.clone()],
                    token.value,
                    "token value does not match its span in {label}"
                );
                cursor = token.span.end;
            }
            assert_eq!(
                cursor,
                source.len(),
                "tokens do not cover the whole input in {label}"
            );
            if let Err(error) = analyze(&tokens, PathBuf::from("invariants.rq"), source, &NoFs) {
                assert_error_position(&error, source, label);
            }
        }
        Err(error) => assert_error_position(&error, source, label),
    }
    RqFile::from_content_lenient(PathBuf::from("invariants.rq"), source, &NoFs);
}

fn assert_error_position(error: &rq_lib::syntax::error::SyntaxError, source: &str, label: &str) {
    assert!(
        error.span.start <= error.span.end,
        "inverted error span in {label}: {:?} ({})",
        error.span,
        error.message
    );
    assert!(
        error.span.end <= source.len(),
        "error span past the end of the input in {label}: {:?} of {} bytes ({})",
        error.span,
        source.len(),
        error.message
    );
    assert!(
        source.is_char_boundary(error.span.start) && source.is_char_boundary(error.span.end),
        "error span off a char boundary in {label}: {:?} ({})",
        error.span,
        error.message
    );
    assert!(
        error.line >= 1 && error.column >= 1,
        "error position is not one-based in {label}: {}:{} ({})",
        error.line,
        error.column,
        error.message
    );
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root must be reachable from the crate directory")
}

fn collect_rq_files(dir: &Path) -> Vec<PathBuf> {
    let ignored = ["node_modules", "target", "out", ".git", "wasm"];
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if ignored.contains(&name.as_str()) {
                continue;
            }
            found.extend(collect_rq_files(&path));
        } else if name.ends_with(".rq") {
            found.push(path);
        }
    }
    found
}
