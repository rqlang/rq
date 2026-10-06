use clap::{CommandFactory, Parser};
use std::path::Path;

mod commands;
mod core;

use commands::Commands;
use core::error::{error_to_json, CheckFailed, RqError};
use core::exit_code::ExitCode;
use core::formatter::OutputFormat;
use core::logger::log_finished;

#[derive(Parser)]
#[command(name = "rq")]
#[command(
    about = "A simple request query language parser. Defaults to 'request run' if no subcommand is provided."
)]
#[command(version = crate::core::version::app_version())]
struct Args {
    #[arg(short, long, help = "Enable debug logging", global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser)]
#[command(name = "rq")]
struct DefaultArgs {
    #[arg(short, long, help = "Enable debug logging", global = true)]
    debug: bool,
    #[command(flatten)]
    run_args: commands::request::RunArgs,
}

#[tokio::main]
async fn main() {
    let output_format = extract_output_format(&std::env::args().collect::<Vec<_>>());
    if let Err(e) = run().await {
        if e.downcast_ref::<CheckFailed>().is_none() {
            match output_format {
                OutputFormat::Json => eprintln!("{}", error_to_json(e.as_ref())),
                OutputFormat::Text => eprintln!("Error: {e}"),
            }
        }
        let exit_code = ExitCode::from(&e);
        log_finished(exit_code.code(), Some(&e.to_string()));
        std::process::exit(exit_code.code());
    }
    log_finished(0, None);
}

fn extract_output_format(args: &[String]) -> OutputFormat {
    let pairs = args.iter().zip(args.iter().skip(1).map(Some).chain([None]));
    for (arg, next) in pairs {
        let inline_value = arg
            .strip_prefix("--format=")
            .or_else(|| arg.strip_prefix("--output="))
            .or_else(|| arg.strip_prefix("-f").filter(|value| !value.is_empty()))
            .or_else(|| arg.strip_prefix("-o").filter(|value| !value.is_empty()));
        let value = match arg.as_str() {
            "-f" | "--format" | "-o" | "--output" => next.map(String::as_str),
            _ => inline_value,
        };
        if value.is_some_and(|value| value.eq_ignore_ascii_case("json")) {
            return OutputFormat::Json;
        }
    }
    OutputFormat::Text
}

fn reject_legacy_output_flag(args: &[String]) -> Result<(), RqError> {
    let uses_legacy_flag = args
        .iter()
        .skip(1)
        .any(|arg| arg == "--output" || arg.starts_with("--output=") || arg.starts_with("-o"));
    if uses_legacy_flag {
        return Err(RqError::Generic(
            "-o/--output was renamed to -f/--format".to_string(),
        ));
    }
    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    reject_legacy_output_flag(&args)?;
    let is_subcommand = args.len() > 1
        && (args[1] == "env"
            || args[1] == "auth"
            || args[1] == "check"
            || args[1] == "request"
            || args[1] == "help");

    if is_subcommand {
        let args = Args::parse();
        crate::core::logger::init_logging(args.debug);
        match args.command {
            Some(Commands::Check(check_args)) => commands::check::execute(&check_args),
            Some(Commands::Env(env_command)) => match env_command.command {
                commands::env::EnvSubcommand::List(list_args) => {
                    commands::env::execute_list(&list_args)
                }
            },
            Some(Commands::Auth(auth_command)) => match auth_command.command {
                commands::auth::AuthSubcommand::List(list_args) => {
                    commands::auth::execute_list(&list_args)
                }
                commands::auth::AuthSubcommand::Show(show_args) => {
                    commands::auth::execute_show(&show_args)
                }
            },
            Some(Commands::Request(request_command)) => match request_command.command {
                commands::request::RequestSubcommand::List(list_args) => {
                    commands::request::execute_list(&list_args)
                }
                commands::request::RequestSubcommand::Show(show_args) => {
                    commands::request::execute_show(&show_args)
                }
                commands::request::RequestSubcommand::Run(run_args) => {
                    commands::request::execute_run(&run_args).await
                }
            },
            None => Ok(()),
        }
    } else {
        let result = Args::try_parse();

        match result {
            Ok(_) => {
                if args.len() == 1 && !has_rq_files_in_current_dir() {
                    Args::command().print_help()?;
                    println!();
                    return Ok(());
                }
                let default_args = DefaultArgs::parse();
                crate::core::logger::init_logging(default_args.debug);
                commands::request::execute_run(&default_args.run_args).await
            }
            Err(e)
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion =>
            {
                e.print()?;
                Ok(())
            }
            Err(_) => {
                let default_args = DefaultArgs::parse();
                crate::core::logger::init_logging(default_args.debug);
                commands::request::execute_run(&default_args.run_args).await
            }
        }
    }
}

/// Checks whether the current working directory contains any .rq files (non-recursive).
fn has_rq_files_in_current_dir() -> bool {
    let current_dir = Path::new(".");
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "rq" {
                    return true;
                }
            }
        }
    }
    false
}
