use crate::commands::validators;
use crate::core::error::{warning_to_json, RqError};
use crate::core::formatter::{render_list, to_json, OutputFormat};
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

#[derive(Serialize)]
struct NameView {
    name: String,
}

pub fn render_names(
    output: OutputFormat,
    names: Vec<String>,
    title: &str,
    empty_msg: &str,
) -> String {
    match output {
        OutputFormat::Json => {
            let views: Vec<NameView> = names.into_iter().map(|name| NameView { name }).collect();
            to_json(&views)
        }
        OutputFormat::Text => render_list(&names, title, empty_msg),
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
