use crate::commands::shared::{OutputArgs, SourceArgs};
use crate::core::formatter::OutputFormat;
use clap::{Args, Subcommand};
use rq_lib::RqClient;

#[derive(Args)]
#[command(name = "env")]
#[command(about = "Manage environments")]
pub struct EnvCommand {
    #[command(subcommand)]
    pub command: EnvSubcommand,
}

#[derive(Subcommand)]
pub enum EnvSubcommand {
    #[command(about = "List environments")]
    List(ListArgs),
    #[command(about = "Show environment location")]
    Show(ShowArgs),
}

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(short = 'n', long = "name", help = "Name of the environment to show")]
    pub name: String,

    #[arg(long = "no-var-interpolation", help = "Skip variable interpolation")]
    pub no_var_interpolation: bool,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let env_list = RqClient::default().list_environments(path)?;

    match args.output.output {
        OutputFormat::Json => {
            let entries: Vec<serde_json::Value> = env_list
                .iter()
                .map(|name| serde_json::json!({ "name": name }))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
            );
        }
        OutputFormat::Text => {
            let formatter = crate::core::formatter::get_formatter(&args.output.output);
            print!(
                "{}",
                formatter.format_list(
                    &env_list,
                    "Environments found:",
                    "No environments found in .rq files"
                )
            );
        }
    }

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entry = RqClient::default().get_environment(path, &args.name)?;
    match args.output.output {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entry).unwrap_or_default()
            );
        }
        OutputFormat::Text => {
            let formatter = crate::core::formatter::get_formatter(&args.output.output);
            print!("{}", formatter.format(&entry));
        }
    }
    Ok(())
}
