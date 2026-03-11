use crate::commands::shared::{OutputArgs, SourceArgs};
use crate::commands::validators;
use crate::core::formatter::OutputFormat;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(name = "ep")]
#[command(about = "Manage endpoints")]
pub struct EpCommand {
    #[command(subcommand)]
    pub command: EpSubcommand,
}

#[derive(Subcommand)]
pub enum EpSubcommand {
    #[command(about = "Show endpoint location")]
    Show(ShowArgs),
}

#[derive(Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the endpoint to show",
        value_parser = validators::validate_name
    )]
    pub name: String,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entry = crate::client::RqClient::get_endpoint(path, &args.name)?;
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
