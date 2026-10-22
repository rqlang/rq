use crate::error::RqError;
use crate::logger::Logger;
use crate::syntax::fs::Fs;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn clean_path_str(s: &str) -> &str {
    #[cfg(windows)]
    {
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return stripped;
        }
    }
    s
}

pub fn clean_path(path: &Path) -> String {
    let s = path.display().to_string();
    clean_path_str(&s).to_string()
}

const SKIPPED_DIRECTORIES: &[&str] = &["node_modules", "target", "dist", "out"];

pub fn is_skipped_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.') || SKIPPED_DIRECTORIES.contains(&name))
}

pub const MAX_WALKED_ENTRIES: usize = 50_000;

pub fn walk_rq_files(fs: &dyn Fs, root: &Path) -> Result<Vec<PathBuf>, RqError> {
    walk_rq_files_up_to(fs, root, MAX_WALKED_ENTRIES)
}

fn walk_rq_files_up_to(fs: &dyn Fs, root: &Path, limit: usize) -> Result<Vec<PathBuf>, RqError> {
    let mut walk = RqFileWalk {
        fs,
        root,
        limit,
        walked: 0,
        paths: Vec::new(),
        visited: HashSet::new(),
    };
    if fs.is_dir(root) {
        walk.visit(root)?;
    }
    Ok(walk.paths)
}

pub fn resolve_under_workspace(logical: &Path, workspace: Option<&Path>) -> PathBuf {
    let Some(workspace) = workspace else {
        return logical.to_path_buf();
    };
    let root = workspace_root(workspace);
    if logical.is_absolute() {
        if logical.starts_with(root) {
            return logical.to_path_buf();
        }
        return match logical.file_name() {
            Some(name) => root.join(name),
            None => root.to_path_buf(),
        };
    }
    root.join(logical)
}

fn workspace_root(workspace: &Path) -> &Path {
    if workspace.is_file() {
        workspace.parent().unwrap_or(workspace)
    } else {
        workspace
    }
}

struct RqFileWalk<'a> {
    fs: &'a dyn Fs,
    root: &'a Path,
    limit: usize,
    walked: usize,
    paths: Vec<PathBuf>,
    visited: HashSet<PathBuf>,
}

impl RqFileWalk<'_> {
    fn visit(&mut self, dir: &Path) -> Result<(), RqError> {
        if !self.mark_visited(dir) {
            return Ok(());
        }
        let entries = match self.fs.read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if dir == self.root => return Err(RqError::Generic(e)),
            Err(e) => {
                Logger::debug(&format!(
                    "* Skipping unreadable directory {}: {e}",
                    clean_path(dir)
                ));
                return Ok(());
            }
        };
        self.walked += entries.len();
        if self.walked > self.limit {
            return Err(RqError::Generic(format!(
                "Too many files under {}: rq stops looking for .rq files after {} entries. Point it at the folder that holds them.",
                clean_path(self.root),
                self.limit
            )));
        }
        for path in entries {
            if self.fs.is_dir(&path) {
                if is_skipped_directory(&path) {
                    Logger::debug(&format!("* Skipping directory {}", clean_path(&path)));
                    continue;
                }
                self.visit(&path)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                self.paths.push(path);
            }
        }
        Ok(())
    }

    fn mark_visited(&mut self, dir: &Path) -> bool {
        let key = self
            .fs
            .canonicalize(dir)
            .unwrap_or_else(|_| dir.to_path_buf());
        if self.visited.insert(key) {
            return true;
        }
        Logger::debug(&format!(
            "* Skipping already visited directory {}",
            clean_path(dir)
        ));
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_skipped_directory, resolve_under_workspace, walk_rq_files, walk_rq_files_up_to,
    };
    use crate::native::NativeFs;
    use std::path::{Path, PathBuf};

    fn workspace_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for file in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "").expect("write");
        }
        dir
    }

    fn walked_names(root: &Path) -> Vec<String> {
        let mut names: Vec<String> = walk_rq_files(&NativeFs, root)
            .expect("walk")
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .expect("inside root")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn walk_finds_rq_files_in_nested_directories() {
        let dir = workspace_with(&["a.rq", "api/b.rq", "api/deep/c.rq"]);
        assert_eq!(
            walked_names(dir.path()),
            ["a.rq", "api/b.rq", "api/deep/c.rq"]
        );
    }

    #[test]
    fn walk_ignores_files_that_are_not_rq() {
        let dir = workspace_with(&["a.rq", "notes.txt", "data.json"]);
        assert_eq!(walked_names(dir.path()), ["a.rq"]);
    }

    #[test]
    fn walk_skips_dependency_and_build_directories() {
        let dir = workspace_with(&["a.rq", "node_modules/pkg/b.rq", "target/c.rq"]);
        assert_eq!(walked_names(dir.path()), ["a.rq"]);
    }

    #[test]
    fn walk_of_a_path_that_is_not_a_directory_finds_nothing() {
        let dir = workspace_with(&["a.rq"]);
        assert!(walked_names(&dir.path().join("a.rq")).is_empty());
    }

    #[test]
    fn skips_dependency_and_build_directories() {
        for name in ["node_modules", "target", "dist", "out"] {
            assert!(
                is_skipped_directory(Path::new("/workspace").join(name).as_path()),
                "`{name}` must be skipped"
            );
        }
    }

    #[test]
    fn walk_stops_with_an_error_past_the_entry_limit() {
        let dir = workspace_with(&["a.rq", "b.txt", "api/c.rq"]);
        let target = walk_rq_files_up_to(&NativeFs, dir.path(), 3);
        assert!(target
            .expect_err("over the limit")
            .to_string()
            .contains("Too many files"));
    }

    #[test]
    fn walk_counts_every_entry_up_to_the_limit() {
        let dir = workspace_with(&["a.rq", "b.txt", "api/c.rq"]);
        let target = walk_rq_files_up_to(&NativeFs, dir.path(), 4);
        assert_eq!(target.expect("within the limit").len(), 2);
    }

    #[test]
    fn skips_hidden_directories() {
        for name in [".git", ".venv", ".cache"] {
            assert!(
                is_skipped_directory(Path::new("/workspace").join(name).as_path()),
                "`{name}` must be skipped"
            );
        }
    }

    #[test]
    fn walk_still_enters_a_hidden_root() {
        let dir = workspace_with(&[".config/a.rq"]);
        assert_eq!(walked_names(&dir.path().join(".config")), ["a.rq"]);
    }

    #[test]
    fn does_not_skip_an_ordinary_directory() {
        assert!(!is_skipped_directory(Path::new("/workspace/api")));
    }

    #[test]
    fn does_not_skip_a_directory_merely_containing_a_skipped_name() {
        assert!(!is_skipped_directory(Path::new("/workspace/target_api")));
    }

    #[test]
    fn keeps_the_logical_path_when_there_is_no_workspace() {
        let target = resolve_under_workspace(Path::new("api/users.rq"), None);
        assert_eq!(target, PathBuf::from("api/users.rq"));
    }

    #[test]
    fn preserves_the_subdirectory_of_a_relative_path() {
        let target =
            resolve_under_workspace(Path::new("api/users.rq"), Some(Path::new("/workspace")));
        assert_eq!(target, PathBuf::from("/workspace/api/users.rq"));
    }

    #[test]
    fn keeps_an_absolute_path_already_inside_the_workspace() {
        let target = resolve_under_workspace(
            Path::new("/workspace/api/users.rq"),
            Some(Path::new("/workspace")),
        );
        assert_eq!(target, PathBuf::from("/workspace/api/users.rq"));
    }

    #[test]
    fn pulls_an_absolute_path_outside_the_workspace_back_under_it() {
        let target = resolve_under_workspace(
            Path::new("/elsewhere/api/users.rq"),
            Some(Path::new("/workspace")),
        );
        assert_eq!(target, PathBuf::from("/workspace/users.rq"));
    }

    #[test]
    fn takes_the_parent_when_the_workspace_is_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace_file = dir.path().join("existing.rq");
        std::fs::write(&workspace_file, "").expect("write");
        let target = resolve_under_workspace(Path::new("users.rq"), Some(&workspace_file));
        assert_eq!(target, dir.path().join("users.rq"));
    }
}
