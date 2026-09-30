use crate::syntax::fs::Fs;
use crate::syntax::types::ValueType;
use std::path::PathBuf;

pub struct FunctionContext<'a> {
    pub source_files: &'a [PathBuf],
    pub fs: &'a dyn Fs,
}

pub trait RqFunction: Send + Sync {
    fn namespace(&self) -> &str;
    fn name(&self) -> &str;

    fn full_name(&self) -> String {
        format!("{}.{}", self.namespace(), self.name())
    }

    fn return_type(&self) -> ValueType {
        ValueType::String
    }

    fn validate_args(&self, _args: &[String]) -> Result<(), String> {
        Ok(())
    }

    fn execute(&self, args: &[String], ctx: &FunctionContext) -> Result<String, String>;
}
