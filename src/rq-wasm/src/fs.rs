use rq_lib::syntax::fs::Fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct WasmFs {
    files: HashMap<String, String>,
}

impl WasmFs {
    pub fn new(files: HashMap<String, String>) -> Self {
        Self { files }
    }

    fn normalize(path: &Path) -> String {
        let s = path.to_string_lossy();
        let mut parts: Vec<&str> = Vec::new();
        for component in s.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                other => parts.push(other),
            }
        }
        if s.starts_with('/') {
            format!("/{}", parts.join("/"))
        } else {
            parts.join("/")
        }
    }
}

impl Fs for WasmFs {
    fn read(&self, path: &Path) -> Result<String, String> {
        let key = Self::normalize(path);
        self.files
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("file not found in WasmFs: {key}"))
    }

    fn resolve_path(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
        let dir = if self.is_dir(base) {
            base.to_path_buf()
        } else {
            base.parent().unwrap_or(base).to_path_buf()
        };
        let joined = dir.join(relative);
        let normalized = Self::normalize(&joined);
        Ok(PathBuf::from(normalized))
    }

    fn exists(&self, path: &Path) -> bool {
        let key = Self::normalize(path);
        self.files.contains_key(&key) || self.is_dir(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        let key = Self::normalize(path);
        self.files.contains_key(&key)
    }

    fn is_dir(&self, path: &Path) -> bool {
        let key = Self::normalize(path);
        let prefix = if key.ends_with('/') {
            key.clone()
        } else {
            format!("{key}/")
        };
        self.files.keys().any(|k| k.starts_with(&prefix))
    }

    fn read_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, String> {
        let key = Self::normalize(dir);
        let prefix = if key.ends_with('/') {
            key.clone()
        } else {
            format!("{key}/")
        };
        let mut seen = std::collections::HashSet::new();
        let mut entries: Vec<PathBuf> = Vec::new();
        for k in self.files.keys() {
            if let Some(rest) = k.strip_prefix(&prefix) {
                let immediate = rest.split('/').next().unwrap_or("");
                if !immediate.is_empty() && seen.insert(immediate.to_string()) {
                    entries.push(PathBuf::from(format!("{prefix}{immediate}")));
                }
            }
        }
        Ok(entries)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        Ok(PathBuf::from(Self::normalize(path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rq_lib::syntax::fs::Fs;

    fn make_fs(entries: &[(&str, &str)]) -> WasmFs {
        WasmFs::new(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn read_returns_file_content() {
        let target = make_fs(&[("/project/foo.rq", "hello")]);
        assert_eq!(target.read(Path::new("/project/foo.rq")).unwrap(), "hello");
    }

    #[test]
    fn read_missing_file_returns_error() {
        let target = make_fs(&[]);
        assert!(target.read(Path::new("/project/missing.rq")).is_err());
    }

    #[test]
    fn is_file_true_for_known_path() {
        let target = make_fs(&[("/project/foo.rq", "")]);
        assert!(target.is_file(Path::new("/project/foo.rq")));
    }

    #[test]
    fn is_file_false_for_directory_path() {
        let target = make_fs(&[("/project/sub/foo.rq", "")]);
        assert!(!target.is_file(Path::new("/project/sub")));
    }

    #[test]
    fn is_dir_true_for_direct_parent() {
        let target = make_fs(&[("/project/sub/foo.rq", "")]);
        assert!(target.is_dir(Path::new("/project/sub")));
    }

    #[test]
    fn is_dir_true_for_ancestor() {
        let target = make_fs(&[("/project/sub/foo.rq", "")]);
        assert!(target.is_dir(Path::new("/project")));
    }

    #[test]
    fn is_dir_false_for_file_path() {
        let target = make_fs(&[("/project/foo.rq", "")]);
        assert!(!target.is_dir(Path::new("/project/foo.rq")));
    }

    #[test]
    fn is_dir_false_for_unknown_path() {
        let target = make_fs(&[]);
        assert!(!target.is_dir(Path::new("/project/missing")));
    }

    #[test]
    fn exists_true_for_known_file() {
        let target = make_fs(&[("/project/foo.rq", "")]);
        assert!(target.exists(Path::new("/project/foo.rq")));
    }

    #[test]
    fn exists_true_for_implied_directory() {
        let target = make_fs(&[("/project/sub/foo.rq", "")]);
        assert!(target.exists(Path::new("/project/sub")));
        assert!(target.exists(Path::new("/project")));
    }

    #[test]
    fn exists_false_for_unknown_path() {
        let target = make_fs(&[]);
        assert!(!target.exists(Path::new("/project/missing.rq")));
    }

    #[test]
    fn read_dir_returns_direct_files() {
        let target = make_fs(&[("/project/a.rq", ""), ("/project/b.rq", "")]);
        let mut entries = target.read_dir(Path::new("/project")).unwrap();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                PathBuf::from("/project/a.rq"),
                PathBuf::from("/project/b.rq"),
            ]
        );
    }

    #[test]
    fn read_dir_returns_subdirectory_as_single_entry() {
        let target = make_fs(&[
            ("/project/sub/a.rq", ""),
            ("/project/sub/b.rq", ""),
        ]);
        let entries = target.read_dir(Path::new("/project")).unwrap();
        assert_eq!(entries, vec![PathBuf::from("/project/sub")]);
    }

    #[test]
    fn read_dir_returns_both_files_and_subdirs() {
        let target = make_fs(&[
            ("/project/root.rq", ""),
            ("/project/sub/a.rq", ""),
        ]);
        let mut entries = target.read_dir(Path::new("/project")).unwrap();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                PathBuf::from("/project/root.rq"),
                PathBuf::from("/project/sub"),
            ]
        );
    }

    #[test]
    fn read_dir_deduplicates_subdir_with_many_files() {
        let target = make_fs(&[
            ("/project/sub/a.rq", ""),
            ("/project/sub/b.rq", ""),
            ("/project/sub/c.rq", ""),
        ]);
        let entries = target.read_dir(Path::new("/project")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], PathBuf::from("/project/sub"));
    }

    #[test]
    fn read_dir_does_not_include_entries_from_sibling_dirs() {
        let target = make_fs(&[
            ("/project/a/x.rq", ""),
            ("/project/b/y.rq", ""),
        ]);
        let entries_a = target.read_dir(Path::new("/project/a")).unwrap();
        assert_eq!(entries_a, vec![PathBuf::from("/project/a/x.rq")]);
    }

    #[test]
    fn read_dir_subdir_entry_is_recognized_as_dir() {
        let target = make_fs(&[("/project/sub/foo.rq", "")]);
        let entries = target.read_dir(Path::new("/project")).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(target.is_dir(&entries[0]));
        assert!(!target.is_file(&entries[0]));
    }

    #[test]
    fn recursive_traversal_finds_all_rq_files_in_nested_dirs() {
        let target = make_fs(&[
            ("/project/root.rq", ""),
            ("/project/open_api/a.rq", ""),
            ("/project/open_api/b.rq", ""),
            ("/project/other/c.rq", ""),
            ("/project/other/deep/d.rq", ""),
        ]);
        let found = collect_rq_recursive(&target, Path::new("/project"));
        assert_eq!(found.len(), 5);
    }

    fn collect_rq_recursive(fs: &WasmFs, dir: &Path) -> Vec<PathBuf> {
        let mut result = Vec::new();
        if let Ok(entries) = fs.read_dir(dir) {
            for entry in entries {
                if fs.is_dir(&entry) {
                    result.extend(collect_rq_recursive(fs, &entry));
                } else if entry.extension().and_then(|s| s.to_str()) == Some("rq") {
                    result.push(entry);
                }
            }
        }
        result
    }

    #[test]
    fn resolve_path_from_file_uses_parent_dir() {
        let target = make_fs(&[("/project/foo.rq", "")]);
        let resolved = target
            .resolve_path(Path::new("/project/foo.rq"), "bar.rq")
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/project/bar.rq"));
    }

    #[test]
    fn resolve_path_from_dir_uses_dir_directly() {
        let target = make_fs(&[("/project/foo.rq", "")]);
        let resolved = target
            .resolve_path(Path::new("/project"), "bar.rq")
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/project/bar.rq"));
    }

    #[test]
    fn canonicalize_normalizes_path() {
        let target = make_fs(&[]);
        let result = target
            .canonicalize(Path::new("/project/../project/foo.rq"))
            .unwrap();
        assert_eq!(result, PathBuf::from("/project/foo.rq"));
    }
}
