use std::path::Path;

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
