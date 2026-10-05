use crate::commands::validators;
use crate::core::error::{warning_to_json, RqError};
use crate::core::formatter::OutputFormat;
use clap::Args;
use serde::Serialize;

#[derive(Debug, Args)]
pub struct OutputArgs {
    #[arg(
        short = 'o',
        long = "output",
        help = "Output format: text or json",
        default_value_t = OutputFormat::Text,
        value_enum,
        ignore_case = true
    )]
    pub output: OutputFormat,
}

#[derive(Debug, Args)]
pub struct SourceArgs {
    #[arg(
        short = 's',
        long = "source",
        default_value = ".",
        help = "Path to the .rq file or directory",
        value_parser = validators::validate_path_exists
    )]
    pub source: String,
}

#[derive(Debug, Args)]
pub struct EnvArgs {
    #[arg(
        short = 'e',
        long = "env",
        alias = "environment",
        help = "Environment name",
        value_parser = validators::validate_name
    )]
    pub environment: Option<String>,
}

#[derive(Serialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

impl Location {
    pub fn from_zero_based(file: String, line: usize, character: usize) -> Self {
        Self {
            file,
            line: line + 1,
            column: character + 1,
        }
    }
}

pub fn print_warnings(warnings: &[RqError], output: OutputFormat) {
    for warning in warnings {
        match output {
            OutputFormat::Json => eprintln!("{}", warning_to_json(warning)),
            OutputFormat::Text => eprintln!("Warning: {warning}"),
        }
    }
}
