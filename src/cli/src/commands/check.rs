use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use crate::core::error::{CheckFailed, RqError};
use crate::core::formatter::OutputFormat;
use clap::Args;
use rq_lib::lint::LintDiagnostic;
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

    #[arg(
        long = "deny-warnings",
        help = "Exit with an error when lint reports any warning"
    )]
    pub deny_warnings: bool,
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
struct CheckWarning {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    line: usize,
    column: usize,
    rule: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_fix: Option<String>,
}

#[derive(Serialize)]
struct CheckResult {
    errors: Vec<CheckError>,
    warnings: Vec<CheckWarning>,
}

pub fn execute(args: &CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let client = RqClient::default();
    let errors = client.check_path(path, args.env_args.environment.as_deref())?;
    let warnings = client.lint_path(path)?;

    let result = CheckResult {
        errors: errors.into_iter().map(to_check_error).collect(),
        warnings: warnings.into_iter().map(to_check_warning).collect(),
    };

    match args.output.output {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Text => print!("{}", render_text(&result)),
    }

    let denied_warnings = args.deny_warnings && !result.warnings.is_empty();
    if result.errors.is_empty() && !denied_warnings {
        Ok(())
    } else {
        Err(Box::new(CheckFailed {
            error_count: result.errors.len(),
            warning_count: result.warnings.len(),
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

fn to_check_warning(diagnostic: LintDiagnostic) -> CheckWarning {
    CheckWarning {
        file: diagnostic.file,
        line: diagnostic.line,
        column: diagnostic.column,
        rule: diagnostic.rule.to_string(),
        message: diagnostic.message,
        suggested_fix: diagnostic.suggested_fix,
    }
}

fn render_text(result: &CheckResult) -> String {
    if result.errors.is_empty() && result.warnings.is_empty() {
        return "No errors found\n".to_string();
    }
    let mut out: String = result.errors.iter().map(render_error_line).collect();
    out.extend(result.warnings.iter().map(render_warning_line));
    out.push_str(&format!("\n{}\n", render_summary(result)));
    out
}

fn render_summary(result: &CheckResult) -> String {
    let errors = pluralize(result.errors.len(), "error");
    if result.warnings.is_empty() {
        return format!("{errors} found");
    }
    let warnings = pluralize(result.warnings.len(), "warning");
    format!("{errors}, {warnings} found")
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn render_error_line(error: &CheckError) -> String {
    let location = render_location(error.file.as_deref(), error.line, error.column);
    format!("{location}{}\n", error.message)
}

fn render_warning_line(warning: &CheckWarning) -> String {
    let location = render_location(
        warning.file.as_deref(),
        Some(warning.line),
        Some(warning.column),
    );
    let mut line = format!("{location}warning[{}]: {}\n", warning.rule, warning.message);
    if let Some(fix) = &warning.suggested_fix {
        line.push_str(&format!("  help: {fix}\n"));
    }
    line
}

fn render_location(file: Option<&str>, line: Option<usize>, column: Option<usize>) -> String {
    match (file, line, column) {
        (Some(file), Some(line), Some(column)) => format!("{file}:{line}:{column}: "),
        (Some(file), Some(line), None) => format!("{file}:{line}: "),
        (Some(file), None, _) => format!("{file}: "),
        (None, _, _) => String::new(),
    }
}
