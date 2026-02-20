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

        let resolved_path = if let Some(src) = ctx.source_files.first() {
            if let Some(parent) = src.parent() {
                parent.join(file_path)
            } else {
                PathBuf::from(file_path)
            }
        } else {
            PathBuf::from(file_path)
        };

        match std::fs::read_to_string(&resolved_path) {
            Ok(content) => Ok(content),
            Err(e) => Err(format!("Error reading file {file_path}: {e}")),
        }
    }
}
