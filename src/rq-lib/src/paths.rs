use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::RqError;
use crate::logger::Logger;
use crate::syntax::fs::Fs;

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

pub const MAX_WALKED_ENTRIES: usize = 50_000;
pub const IGNORE_FILE: &str = ".rqignore";

pub fn walk_rq_files(fs: &dyn Fs, root: &Path) -> Result<Vec<PathBuf>, RqError> {
    walk_rq_files_up_to(fs, root, MAX_WALKED_ENTRIES)
}

fn walk_rq_files_up_to(fs: &dyn Fs, root: &Path, limit: usize) -> Result<Vec<PathBuf>, RqError> {
    let mut walk = RqFileWalk {
        fs,
        root,
        root_key: fs.canonicalize(root).ok(),
        rules: SkipRules::load(fs, root),
        limit,
        walked: 0,
        walked_by_folder: HashMap::new(),
        paths: Vec::new(),
        visited: HashSet::new(),
    };
    if fs.is_dir(root) {
        walk.visit(root, None)?;
    }
    Ok(walk.paths)
}

pub fn resolve_under_workspace(logical: &Path, workspace: Option<&Path>) -> PathBuf {
    let Some(workspace) = workspace else {
        return logical.to_path_buf();
    };
    let root = workspace_root(workspace);
    if is_absolute(logical) {
        if with_upper_drive(logical).starts_with(with_upper_drive(root)) {
            return logical.to_path_buf();
        }
        return match logical.file_name() {
            Some(name) => root.join(name),
            None => root.to_path_buf(),
        };
    }
    root.join(logical)
}

fn is_absolute(path: &Path) -> bool {
    path.is_absolute() || drive_letter(path).is_some()
}

fn drive_letter(path: &Path) -> Option<char> {
    let text = path.to_string_lossy();
    let mut chars = text.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some(drive), Some(':'), Some('/' | '\\')) if drive.is_ascii_alphabetic() => Some(drive),
        _ => None,
    }
}

fn with_upper_drive(path: &Path) -> PathBuf {
    match drive_letter(path) {
        Some(drive) => {
            let text = path.to_string_lossy();
            let rest: String = text.chars().skip(1).collect();
            PathBuf::from(format!("{}{rest}", drive.to_ascii_uppercase()))
        }
        None => path.to_path_buf(),
    }
}

fn workspace_root(workspace: &Path) -> &Path {
    if workspace.is_file() {
        workspace.parent().unwrap_or(workspace)
    } else {
        workspace
    }
}

#[derive(Default)]
struct SkipRules {
    names: HashSet<String>,
    paths: HashSet<String>,
    included: HashSet<String>,
}

impl SkipRules {
    fn load(fs: &dyn Fs, root: &Path) -> Self {
        Self::parse(&fs.read(&root.join(IGNORE_FILE)).unwrap_or_default())
    }

    fn parse(content: &str) -> Self {
        let mut rules = Self::default();
        for line in content.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (included, entry) = match line.strip_prefix('!') {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let anchored = entry.starts_with('/') || entry.starts_with("./");
            let entry = entry
                .trim_start_matches("**/")
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/');
            if entry.is_empty() {
                continue;
            }
            if included {
                rules.included.insert(entry.to_string());
            } else if anchored || entry.contains('/') {
                rules.paths.insert(entry.to_string());
            } else {
                rules.names.insert(entry.to_string());
            }
        }
        rules
    }

    fn skips(&self, name: &str, relative: &str) -> bool {
        if self.included.contains(name) || self.included.contains(relative) {
            return false;
        }
        name.starts_with('.') || self.names.contains(name) || self.paths.contains(relative)
    }
}

struct RqFileWalk<'a> {
    fs: &'a dyn Fs,
    root: &'a Path,
    root_key: Option<PathBuf>,
    rules: SkipRules,
    limit: usize,
    walked: usize,
    walked_by_folder: HashMap<String, usize>,
    paths: Vec<PathBuf>,
    visited: HashSet<PathBuf>,
}

impl RqFileWalk<'_> {
    fn visit(&mut self, dir: &Path, relative: Option<&str>) -> Result<(), RqError> {
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
        self.count(relative, entries.len())?;
        for path in entries {
            if self.fs.is_dir(&path) {
                let (name, child) = self.names_of(&path);
                if self.rules.skips(&name, &child) {
                    Logger::debug(&format!("* Skipping directory {}", clean_path(&path)));
                    continue;
                }
                self.visit(&path, Some(&child))?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                self.paths.push(path);
            }
        }
        Ok(())
    }

    fn count(&mut self, relative: Option<&str>, entries: usize) -> Result<(), RqError> {
        self.walked += entries;
        let mut folder = relative;
        while let Some(current) = folder {
            *self
                .walked_by_folder
                .entry(current.to_string())
                .or_default() += entries;
            folder = current.rsplit_once('/').map(|(parent, _)| parent);
        }
        if self.walked > self.limit {
            return Err(self.too_many_files());
        }
        Ok(())
    }

    fn too_many_files(&self) -> RqError {
        let mut largest: Vec<(&String, &usize)> = self
            .walked_by_folder
            .iter()
            .filter(|(folder, _)| !folder.contains('/'))
            .collect();
        largest.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let folders: Vec<String> = largest
            .iter()
            .take(3)
            .map(|(folder, count)| match self.culprit(folder, **count) {
                (culprit, _) if culprit == folder.as_str() => format!("{folder} ({count})"),
                (culprit, _) => format!("{folder} ({count}, most in {culprit})"),
            })
            .collect();
        let largest = if folders.is_empty() {
            String::new()
        } else {
            format!(" Largest folders: {}.", folders.join(", "))
        };
        RqError::Generic(format!(
            "Too many files under {}: rq stops looking for .rq files after {} entries.{largest} Add the folders rq should skip to {}.",
            clean_path(self.root),
            self.limit,
            clean_path(&self.root.join(IGNORE_FILE))
        ))
    }

    fn culprit<'b>(&'b self, folder: &'b str, count: usize) -> (&'b str, usize) {
        let prefix = format!("{folder}/");
        let largest_child = self
            .walked_by_folder
            .iter()
            .filter(|(child, _)| {
                child
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/'))
            })
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)));
        match largest_child {
            Some((child, &child_count)) if child_count * 5 >= count * 4 => {
                self.culprit(child, child_count)
            }
            _ => (folder, count),
        }
    }

    fn names_of(&self, path: &Path) -> (String, String) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let relative = path
            .strip_prefix(self.root)
            .ok()
            .or_else(|| {
                self.root_key
                    .as_deref()
                    .and_then(|key| path.strip_prefix(key).ok())
            })
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| name.clone());
        (name, relative)
    }

    fn mark_visited(&mut self, dir: &Path) -> bool {
        let key = self.fs.real_path(dir).unwrap_or_else(|_| dir.to_path_buf());
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
    use super::{resolve_under_workspace, walk_rq_files, walk_rq_files_up_to, IGNORE_FILE};
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

    fn workspace_ignoring(ignore: &str, files: &[&str]) -> tempfile::TempDir {
        let dir = workspace_with(files);
        std::fs::write(dir.path().join(IGNORE_FILE), ignore).expect("write ignore file");
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
    fn walk_enters_dependency_directories_unless_the_ignore_file_lists_them() {
        let dir = workspace_with(&["a.rq", "node_modules/pkg/b.rq"]);
        assert_eq!(walked_names(dir.path()), ["a.rq", "node_modules/pkg/b.rq"]);
    }

    #[test]
    fn walk_skips_a_directory_named_in_the_ignore_file_at_any_depth() {
        let dir = workspace_ignoring(
            "node_modules\n",
            &["a.rq", "node_modules/b.rq", "web/node_modules/c.rq"],
        );
        assert_eq!(walked_names(dir.path()), ["a.rq"]);
    }

    #[test]
    fn walk_skips_only_the_relative_path_the_ignore_file_lists() {
        let dir = workspace_ignoring("fixtures/big/\n", &["fixtures/big/a.rq", "big/b.rq"]);
        assert_eq!(walked_names(dir.path()), ["big/b.rq"]);
    }

    #[test]
    fn walk_skips_only_the_root_directory_a_leading_slash_names() {
        let dir = workspace_ignoring("/target\n", &["target/a.rq", "api/target/b.rq"]);
        assert_eq!(walked_names(dir.path()), ["api/target/b.rq"]);
    }

    #[test]
    fn walk_skips_a_double_star_name_at_any_depth() {
        let dir = workspace_ignoring(
            "**/node_modules/\n",
            &["a.rq", "node_modules/b.rq", "web/node_modules/c.rq"],
        );
        assert_eq!(walked_names(dir.path()), ["a.rq"]);
    }

    #[test]
    fn walk_ignores_comments_and_blank_lines_in_the_ignore_file() {
        let dir = workspace_ignoring("# api\n\n  \n", &["api/a.rq"]);
        assert_eq!(walked_names(dir.path()), ["api/a.rq"]);
    }

    #[test]
    fn walk_does_not_skip_a_directory_merely_containing_an_ignored_name() {
        let dir = workspace_ignoring("target\n", &["target_api/a.rq"]);
        assert_eq!(walked_names(dir.path()), ["target_api/a.rq"]);
    }

    #[test]
    fn walk_skips_hidden_directories() {
        let dir = workspace_with(&["a.rq", ".git/b.rq", ".venv/c.rq"]);
        assert_eq!(walked_names(dir.path()), ["a.rq"]);
    }

    #[test]
    fn walk_enters_a_hidden_directory_the_ignore_file_includes() {
        let dir = workspace_ignoring("!.config\n", &[".config/a.rq", ".git/b.rq"]);
        assert_eq!(walked_names(dir.path()), [".config/a.rq"]);
    }

    #[test]
    fn walk_still_enters_a_hidden_root() {
        let dir = workspace_with(&[".config/a.rq"]);
        assert_eq!(walked_names(&dir.path().join(".config")), ["a.rq"]);
    }

    #[test]
    fn walk_of_a_path_that_is_not_a_directory_finds_nothing() {
        let dir = workspace_with(&["a.rq"]);
        assert!(walked_names(&dir.path().join("a.rq")).is_empty());
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
    fn walk_error_names_the_nested_folder_that_holds_most_entries() {
        let dir = workspace_with(&[
            "web/a.rq",
            "web/node_modules/0",
            "web/node_modules/1",
            "web/node_modules/2",
            "web/node_modules/3",
            "web/node_modules/4",
            "web/node_modules/5",
            "web/node_modules/6",
            "web/node_modules/7",
            "web/node_modules/8",
            "web/node_modules/9",
        ]);
        let target = walk_rq_files_up_to(&NativeFs, dir.path(), 12)
            .expect_err("over the limit")
            .to_string();
        assert!(
            target.contains("Largest folders: web (12, most in web/node_modules)"),
            "{target}"
        );
    }

    #[test]
    fn walk_error_names_the_largest_folders_and_the_ignore_file() {
        let dir = workspace_with(&["deps/1", "deps/2", "deps/3", "api/c.rq"]);
        let target = walk_rq_files_up_to(&NativeFs, dir.path(), 4)
            .expect_err("over the limit")
            .to_string();
        assert!(target.contains("Largest folders: deps (3)"), "{target}");
        assert!(target.contains(IGNORE_FILE), "{target}");
    }

    #[test]
    fn keeps_a_windows_absolute_path_already_inside_the_workspace() {
        let target = resolve_under_workspace(
            Path::new("C:/repo/sub/users.rq"),
            Some(Path::new("C:/repo")),
        );
        assert_eq!(target, PathBuf::from("C:/repo/sub/users.rq"));
    }

    #[test]
    fn keeps_a_windows_absolute_path_whatever_the_case_of_the_drive() {
        let target = resolve_under_workspace(
            Path::new("C:/repo/sub/users.rq"),
            Some(Path::new("c:/repo")),
        );
        assert_eq!(target, PathBuf::from("C:/repo/sub/users.rq"));
    }

    #[test]
    fn moves_a_windows_absolute_path_from_another_drive_into_the_workspace() {
        let target = resolve_under_workspace(
            Path::new("D:/elsewhere/users.rq"),
            Some(Path::new("C:/repo")),
        );
        assert_eq!(target, PathBuf::from("C:/repo").join("users.rq"));
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
