use super::super::traits::{FunctionContext, RqFunction};
use super::read_source_relative;
use crate::syntax::types::ValueType;

pub struct IoReadJson;

impl RqFunction for IoReadJson {
    fn namespace(&self) -> &str {
        "io"
    }

    fn name(&self) -> &str {
        "read_json"
    }

    fn return_type(&self) -> ValueType {
        ValueType::Json
    }

    fn validate_args(&self, args: &[String]) -> Result<(), String> {
        if args.is_empty() {
            return Err("io.read_json() requires a file path argument".to_string());
        }
        Ok(())
    }

    fn execute(&self, args: &[String], ctx: &FunctionContext) -> Result<String, String> {
        let file_path = &args[0];
        let content = read_source_relative(file_path, ctx)?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("Error parsing JSON file {file_path}: {e}"))?;
        Ok(content)
    }
}
