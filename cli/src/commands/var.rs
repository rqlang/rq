use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use crate::commands::validators;
use crate::core::formatter::OutputFormat;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(name = "var")]
#[command(about = "Manage variables")]
pub struct VarCommand {
    #[command(subcommand)]
    pub command: VarSubcommand,
}

#[derive(Subcommand)]
pub enum VarSubcommand {
    #[command(about = "List variables")]
    List(ListArgs),
    #[command(about = "Show variable location")]
    Show(ShowArgs),
}

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub env: EnvArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the variable to show",
        value_parser = validators::validate_name
    )]
    pub name: String,

    #[command(flatten)]
    pub env: EnvArgs,

    #[arg(long = "no-var-interpolation", help = "Skip variable interpolation")]
    pub no_var_interpolation: bool,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entries = crate::client::RqClient::list_variables(path, args.env.environment.as_deref())?;

    match args.output.output {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
            );
        }
        OutputFormat::Text => {
            let formatter = crate::core::formatter::get_formatter(&args.output.output);
            let names: Vec<String> = entries.into_iter().map(|e| e.name).collect();
            print!(
                "{}",
                formatter.format_list(
                    &names,
                    "Variables found:",
                    "No variables found in .rq files"
                )
            );
        }
    }

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entry = crate::client::RqClient::get_variable(
        path,
        &args.name,
        args.env.environment.as_deref(),
        !args.no_var_interpolation,
    )?;
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
