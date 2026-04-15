use std::path::{Path, PathBuf};

pub trait Fs: Send + Sync {
    fn read(&self, path: &Path) -> Result<String, String>;
    fn resolve_path(&self, base: &Path, relative: &str) -> Result<PathBuf, String>;
    fn exists(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn read_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, String>;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String>;
}
