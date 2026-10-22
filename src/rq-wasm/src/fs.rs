use rq_lib::syntax::fs::Fs;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct WasmFs {
    files: HashMap<String, String>,
    root: Option<String>,
    disk: Box<dyn Fs>,
}

impl WasmFs {
    pub fn new(files: HashMap<String, String>) -> Self {
        Self {
            files,
            root: None,
            disk: disk(),
        }
    }

    pub fn with_root(files: HashMap<String, String>, root: &Path) -> Self {
        let root = Self::normalize(root);
        Self {
            files,
            root: Some(root),
            disk: disk(),
        }
    }

    fn normalize(path: &Path) -> String {
        let raw = path.to_string_lossy();
        let s = raw.replace('\\', "/");
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

    fn directory_prefix(key: &str) -> String {
        if key.ends_with('/') {
            key.to_string()
        } else {
            format!("{key}/")
        }
    }

    fn holds_directory(&self, key: &str) -> bool {
        let prefix = Self::directory_prefix(key);
        self.files.keys().any(|k| k.starts_with(&prefix)) || self.root.as_deref() == Some(key)
    }

    fn draft_entries(&self, key: &str) -> Vec<String> {
        let prefix = Self::directory_prefix(key);
        self.files
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('/').next())
            .filter(|immediate| !immediate.is_empty())
            .map(|immediate| format!("{prefix}{immediate}"))
            .collect()
    }
}

impl Fs for WasmFs {
    fn read(&self, path: &Path) -> Result<String, String> {
        let key = Self::normalize(path);
        match self.files.get(&key) {
            Some(content) => Ok(content.clone()),
            None => self.disk.read(Path::new(&key)),
        }
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
        self.files.contains_key(&key)
            || self.holds_directory(&key)
            || self.disk.exists(Path::new(&key))
    }

    fn is_file(&self, path: &Path) -> bool {
        let key = Self::normalize(path);
        self.files.contains_key(&key) || self.disk.is_file(Path::new(&key))
    }

    fn is_dir(&self, path: &Path) -> bool {
        let key = Self::normalize(path);
        if self.files.contains_key(&key) {
            return false;
        }
        self.holds_directory(&key) || self.disk.is_dir(Path::new(&key))
    }

    fn read_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, String> {
        let key = Self::normalize(dir);
        let mut entries = self.draft_entries(&key);
        match self.disk.read_dir(Path::new(&key)) {
            Ok(found) => entries.extend(found.iter().map(|p| Self::normalize(p))),
            Err(e) if !self.holds_directory(&key) => return Err(e),
            Err(_) => {}
        }
        let mut seen = HashSet::new();
        Ok(entries
            .into_iter()
            .filter(|entry| seen.insert(entry.clone()))
            .map(PathBuf::from)
            .collect())
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        Ok(PathBuf::from(Self::normalize(path)))
    }
}

#[cfg(target_arch = "wasm32")]
fn disk() -> Box<dyn Fs> {
    Box::new(node::NodeFs)
}

#[cfg(not(target_arch = "wasm32"))]
fn disk() -> Box<dyn Fs> {
    Box::new(rq_lib::native::NativeFs)
}

#[cfg(target_arch = "wasm32")]
mod node {
    use rq_lib::syntax::fs::Fs;
    use std::path::{Path, PathBuf};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    #[wasm_bindgen(module = "fs")]
    extern "C" {
        type Stats;

        #[wasm_bindgen(catch, js_name = readFileSync)]
        fn read_file_sync(path: &str, encoding: &str) -> Result<String, JsValue>;

        #[wasm_bindgen(catch, js_name = readdirSync)]
        fn readdir_sync(path: &str) -> Result<js_sys::Array, JsValue>;

        #[wasm_bindgen(catch, js_name = statSync)]
        fn stat_sync(path: &str) -> Result<Stats, JsValue>;

        #[wasm_bindgen(method, js_name = isDirectory)]
        fn is_directory(this: &Stats) -> bool;

        #[wasm_bindgen(method, js_name = isFile)]
        fn is_regular_file(this: &Stats) -> bool;
    }

    pub struct NodeFs;

    impl Fs for NodeFs {
        fn read(&self, path: &Path) -> Result<String, String> {
            read_file_sync(&path.to_string_lossy(), "utf8").map_err(message)
        }

        fn resolve_path(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
            Ok(base.join(relative))
        }

        fn exists(&self, path: &Path) -> bool {
            stat_sync(&path.to_string_lossy()).is_ok()
        }

        fn is_file(&self, path: &Path) -> bool {
            stat_sync(&path.to_string_lossy()).is_ok_and(|stats| stats.is_regular_file())
        }

        fn is_dir(&self, path: &Path) -> bool {
            stat_sync(&path.to_string_lossy()).is_ok_and(|stats| stats.is_directory())
        }

        fn read_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, String> {
            let names = readdir_sync(&dir.to_string_lossy()).map_err(message)?;
            Ok(names
                .iter()
                .filter_map(|name| name.as_string())
                .map(|name| dir.join(name))
                .collect())
        }

        fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
            Ok(path.to_path_buf())
        }
    }

    fn message(error: JsValue) -> String {
        match error.dyn_ref::<js_sys::Error>() {
            Some(error) => String::from(error.message()),
            None => format!("{error:?}"),
        }
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

    fn make_rooted_fs(entries: &[(&str, &str)], root: &str) -> WasmFs {
        WasmFs::with_root(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            Path::new(root),
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
        let target = make_fs(&[("/project/sub/a.rq", ""), ("/project/sub/b.rq", "")]);
        let entries = target.read_dir(Path::new("/project")).unwrap();
        assert_eq!(entries, vec![PathBuf::from("/project/sub")]);
    }

    #[test]
    fn read_dir_returns_both_files_and_subdirs() {
        let target = make_fs(&[("/project/root.rq", ""), ("/project/sub/a.rq", "")]);
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
        let target = make_fs(&[("/project/a/x.rq", ""), ("/project/b/y.rq", "")]);
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
    fn is_dir_true_for_root_when_no_files_are_loaded() {
        let target = make_rooted_fs(&[], "/project");
        assert!(target.is_dir(Path::new("/project")));
    }

    #[test]
    fn exists_true_for_root_when_no_files_are_loaded() {
        let target = make_rooted_fs(&[], "/project");
        assert!(target.exists(Path::new("/project")));
    }

    #[test]
    fn read_dir_on_empty_root_returns_no_entries() {
        let target = make_rooted_fs(&[], "/project");
        assert!(target.read_dir(Path::new("/project")).unwrap().is_empty());
    }

    #[test]
    fn is_dir_false_for_unknown_path_when_root_is_set() {
        let target = make_rooted_fs(&[], "/project");
        assert!(!target.is_dir(Path::new("/elsewhere")));
    }

    #[test]
    fn is_dir_false_for_missing_subdirectory_of_root() {
        let target = make_rooted_fs(&[], "/project");
        assert!(!target.is_dir(Path::new("/project/sub")));
    }

    #[test]
    fn is_dir_false_when_root_points_at_a_loaded_file() {
        let target = make_rooted_fs(&[("/project/foo.rq", "")], "/project/foo.rq");
        assert!(!target.is_dir(Path::new("/project/foo.rq")));
    }

    #[test]
    fn is_dir_true_for_root_given_with_a_trailing_slash() {
        let target = make_rooted_fs(&[], "/project/");
        assert!(target.is_dir(Path::new("/project")));
    }

    #[test]
    fn is_dir_still_true_for_ancestor_when_root_is_set() {
        let target = make_rooted_fs(&[("/project/sub/foo.rq", "")], "/project");
        assert!(target.is_dir(Path::new("/project/sub")));
    }

    fn disk_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (file, content) in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        dir
    }

    fn key(dir: &tempfile::TempDir, file: &str) -> String {
        WasmFs::normalize(&dir.path().join(file))
    }

    #[test]
    fn read_falls_back_to_a_file_on_disk() {
        let dir = disk_with(&[("data.json", "{}")]);
        let target = make_fs(&[]);
        assert_eq!(
            target.read(Path::new(&key(&dir, "data.json"))).unwrap(),
            "{}"
        );
    }

    #[test]
    fn read_prefers_the_draft_over_the_saved_file() {
        let dir = disk_with(&[("a.rq", "saved")]);
        let target = make_fs(&[(&key(&dir, "a.rq"), "draft")]);
        assert_eq!(target.read(Path::new(&key(&dir, "a.rq"))).unwrap(), "draft");
    }

    #[test]
    fn is_dir_true_for_a_directory_only_on_disk() {
        let dir = disk_with(&[("api/a.rq", "")]);
        let target = make_fs(&[]);
        assert!(target.is_dir(Path::new(&key(&dir, "api"))));
    }

    #[test]
    fn read_dir_merges_disk_entries_with_unsaved_drafts() {
        let dir = disk_with(&[("a.rq", "")]);
        let target = make_fs(&[(&key(&dir, "b.rq"), "")]);
        let mut entries = target
            .read_dir(Path::new(&WasmFs::normalize(dir.path())))
            .unwrap();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                PathBuf::from(key(&dir, "a.rq")),
                PathBuf::from(key(&dir, "b.rq"))
            ]
        );
    }

    #[test]
    fn read_dir_lists_a_saved_file_with_a_draft_once() {
        let dir = disk_with(&[("a.rq", "saved")]);
        let target = make_fs(&[(&key(&dir, "a.rq"), "draft")]);
        let entries = target
            .read_dir(Path::new(&WasmFs::normalize(dir.path())))
            .unwrap();
        assert_eq!(entries, vec![PathBuf::from(key(&dir, "a.rq"))]);
    }

    #[test]
    fn read_dir_of_a_missing_directory_is_an_error() {
        let dir = disk_with(&[]);
        let target = make_fs(&[]);
        assert!(target.read_dir(Path::new(&key(&dir, "missing"))).is_err());
    }

    #[test]
    fn walk_finds_saved_files_and_drafts_but_not_other_files() {
        let dir = disk_with(&[("a.rq", ""), ("notes.txt", ""), ("api/b.rq", "")]);
        let target = make_fs(&[(&key(&dir, "api/c.rq"), "")]);
        let mut found: Vec<String> =
            rq_lib::paths::walk_rq_files(&target, Path::new(&WasmFs::normalize(dir.path())))
                .unwrap()
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                key(&dir, "a.rq"),
                key(&dir, "api/b.rq"),
                key(&dir, "api/c.rq")
            ]
        );
    }

    #[test]
    fn walk_skips_the_folders_a_saved_rqignore_lists() {
        let dir = disk_with(&[(".rqignore", "vendor\n"), ("a.rq", ""), ("vendor/b.rq", "")]);
        let target = make_fs(&[]);
        let found =
            rq_lib::paths::walk_rq_files(&target, Path::new(&WasmFs::normalize(dir.path())))
                .unwrap();
        assert_eq!(found, vec![PathBuf::from(key(&dir, "a.rq"))]);
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
