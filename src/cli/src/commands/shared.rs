use crate::commands::validators;
use crate::core::formatter::OutputFormat;
use clap::Args;

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
