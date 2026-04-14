use super::super::traits::{FunctionContext, RqFunction};
use std::path::PathBuf;

pub struct IoReadFile;

impl RqFunction for IoReadFile {
    fn namespace(&self) -> &str {
        "io"
    }

    fn name(&self) -> &str {
        "read_file"
    }

    fn validate_args(&self, args: &[String]) -> Result<(), String> {
        if args.is_empty() {
            return Err("io.read_file() requires a file path argument".to_string());
        }
        Ok(())
    }

    fn execute(&self, args: &[String], ctx: &FunctionContext) -> Result<String, String> {
        let file_path = &args[0];
        let base = ctx
            .source_files
            .first()
            .map(|p| p.as_path())
            .unwrap_or(std::path::Path::new("."));
        let resolved = ctx
            .fs
            .resolve_path(base, file_path)
            .unwrap_or_else(|_| PathBuf::from(file_path));
        ctx.fs
            .read(&resolved)
            .map_err(|e| format!("Error reading file {file_path}: {e}"))
    }
}
