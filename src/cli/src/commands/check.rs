use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use crate::core::error::{CheckFailed, RqError};
use crate::core::formatter::OutputFormat;
use clap::Args;
use rq_lib::RqClient;
use serde::Serialize;

#[derive(Args)]
#[command(about = "Validate .rq files without executing requests")]
pub struct CheckArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub env_args: EnvArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Serialize)]
struct CheckError {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    message: String,
}

#[derive(Serialize)]
struct CheckResult {
    errors: Vec<CheckError>,
}

pub fn execute(args: &CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let errors = RqClient::default().check_path(path, args.env_args.environment.as_deref())?;

    let result = CheckResult {
        errors: errors.into_iter().map(to_check_error).collect(),
    };

    match args.output.output {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Text => print!("{}", render_text(&result)),
    }

    if result.errors.is_empty() {
        Ok(())
    } else {
        Err(Box::new(CheckFailed {
            error_count: result.errors.len(),
        }))
    }
}

fn to_check_error(error: RqError) -> CheckError {
    match error {
        RqError::Syntax(se) => CheckError {
            file: se.file_path,
            line: (se.line > 0).then_some(se.line),
            column: (se.column > 0).then_some(se.column),
            message: se.message,
        },
        other => CheckError {
            file: None,
            line: None,
            column: None,
            message: other.to_string(),
        },
    }
}

fn render_text(result: &CheckResult) -> String {
    if result.errors.is_empty() {
        return "No errors found\n".to_string();
    }
    let mut out: String = result.errors.iter().map(render_error_line).collect();
    let count = result.errors.len();
    let noun = if count == 1 { "error" } else { "errors" };
    out.push_str(&format!("\n{count} {noun} found\n"));
    out
}

fn render_error_line(error: &CheckError) -> String {
    let location = match (&error.file, error.line, error.column) {
        (Some(file), Some(line), Some(column)) => format!("{file}:{line}:{column}: "),
        (Some(file), Some(line), None) => format!("{file}:{line}: "),
        (Some(file), None, _) => format!("{file}: "),
        (None, _, _) => String::new(),
    };
    format!("{location}{}\n", error.message)
}
