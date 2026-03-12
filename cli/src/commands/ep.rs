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
    #[command(about = "Find all references to an endpoint")]
    Refs(RefsArgs),
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

    #[arg(long = "no-var-interpolation", help = "Skip variable interpolation")]
    pub no_var_interpolation: bool,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct RefsArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the endpoint to find references for",
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

pub fn execute_refs(args: &RefsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let refs = crate::client::RqClient::list_endpoint_references(path, &args.name)?;
    let formatter = crate::core::formatter::get_formatter(&args.output.output);
    print!(
        "{}",
        formatter.format_list(&refs, "References found:", "No references found")
    );
    Ok(())
}
