use super::super::traits::{FunctionContext, RqFunction};
use super::read_source_relative;

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
        read_source_relative(&args[0], ctx)
    }
}
