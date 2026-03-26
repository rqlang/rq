use crate::client::RqClient;
use crate::core::error::RqError;
use clap::Args;
use serde::Serialize;

#[derive(Args)]
#[command(about = "Validate .rq files without executing requests")]
pub struct CheckArgs {
    #[arg(
        short = 's',
        long = "source",
        default_value = ".",
        help = "Path to the .rq file or directory"
    )]
    pub source: String,

    #[arg(
        short = 'e',
        long = "env",
        help = "Environment name to use for variable resolution"
    )]
    pub env: Option<String>,
}

#[derive(Serialize)]
struct CheckError {
    file: String,
    line: usize,
    column: usize,
    message: String,
}

#[derive(Serialize)]
struct CheckResult {
    errors: Vec<CheckError>,
}

pub fn execute(args: &CheckArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source);
    let errors = RqClient::check_path(path, args.env.as_deref())?;

    let check_errors: Vec<CheckError> = errors
        .into_iter()
        .filter_map(|e| {
            if let RqError::Syntax(se) = e {
                se.file_path.map(|f| CheckError {
                    file: f,
                    line: se.line,
                    column: se.column,
                    message: se.message,
                })
            } else {
                None
            }
        })
        .collect();

    let has_errors = !check_errors.is_empty();
    let result = CheckResult {
        errors: check_errors,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);

    if has_errors {
        std::process::exit(1);
    }

    Ok(())
}
