pub mod read_file;
pub mod read_json;

use super::traits::FunctionContext;
use std::path::{Path, PathBuf};

pub(crate) fn read_source_relative(
    file_path: &str,
    ctx: &FunctionContext,
) -> Result<String, String> {
    let base = ctx
        .source_files
        .first()
        .map(|p| p.as_path())
        .unwrap_or(Path::new("."));
    let resolved = ctx
        .fs
        .resolve_path(base, file_path)
        .unwrap_or_else(|_| PathBuf::from(file_path));
    ctx.fs
        .read(&resolved)
        .map_err(|e| format!("Error reading file {file_path}: {e}"))
}
