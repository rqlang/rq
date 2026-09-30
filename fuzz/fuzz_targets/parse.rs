#![no_main]

use libfuzzer_sys::fuzz_target;
use rq_lib::syntax::{analysis::analyze, tokenize, Fs, RqFile};
use std::path::{Path, PathBuf};

struct NoFs;

impl Fs for NoFs {
    fn read(&self, _path: &Path) -> Result<String, String> {
        Err("no filesystem while fuzzing".into())
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

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let path = PathBuf::from("fuzz.rq");

    if let Ok(tokens) = tokenize(source) {
        if let Err(error) = analyze(&tokens, path.clone(), source, &NoFs) {
            assert!(
                error.span.start <= error.span.end,
                "inverted error span on {source:?}"
            );
            assert!(
                error.span.end <= source.len(),
                "error span past the end of the input on {source:?}"
            );
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end),
                "error span off a char boundary on {source:?}"
            );
            assert!(error.line >= 1 && error.column >= 1, "zero-based position");
        }
    }

    RqFile::from_content_lenient(path, source, &NoFs);
});
