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

const SKIPPED_DIRECTORIES: &[&str] = &["node_modules", ".git", "target"];

pub fn is_skipped_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name))
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

#[cfg(test)]
mod tests {
    use super::{is_skipped_directory, resolve_under_workspace};
    use std::path::{Path, PathBuf};

    #[test]
    fn skips_dependency_and_build_directories() {
        for name in ["node_modules", ".git", "target"] {
            assert!(
                is_skipped_directory(Path::new("/workspace").join(name).as_path()),
                "`{name}` must be skipped"
            );
        }
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
