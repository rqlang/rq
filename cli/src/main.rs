use clap::{CommandFactory, Parser};
use std::path::Path;

mod client;
mod commands;
mod core;
mod syntax;

use commands::Commands;
use core::exit_code::ExitCode;

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
    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        let exit_code = ExitCode::from(&e);
        std::process::exit(exit_code.code());
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let is_subcommand = args.len() > 1
        && (args[1] == "env" || args[1] == "auth" || args[1] == "request" || args[1] == "help");

    if is_subcommand {
        let args = Args::parse();
        crate::core::logger::Logger::init(args.debug);
        match args.command {
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
                crate::core::logger::Logger::init(default_args.debug);
                commands::request::execute_run(&default_args.run_args).await
            }
            Err(e)
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion =>
            {
                e.print().unwrap();
                Ok(())
            }
            Err(_) => {
                let default_args = DefaultArgs::parse();
                crate::core::logger::Logger::init(default_args.debug);
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
