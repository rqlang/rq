use crate::commands::shared::{
    absolute_source, reference_views, EnvArgs, Location, OutputArgs, SourceArgs,
};
use crate::commands::validators;
use crate::core::formatter::{get_formatter, OutputFormat};
use clap::{Args, Subcommand};
use rq_lib::client::models::VariableEntry;
use rq_lib::RqClient;
use serde::Serialize;

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
    #[command(about = "Find all references to a variable")]
    Refs(RefsArgs),
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

#[derive(Args)]
pub struct RefsArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the variable to find references for",
        value_parser = validators::validate_name
    )]
    pub name: String,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Serialize)]
struct VariableView {
    name: String,
    value: String,
    source: String,
    #[serde(flatten)]
    location: Location,
}

impl From<VariableEntry> for VariableView {
    fn from(entry: VariableEntry) -> Self {
        Self {
            name: entry.name,
            value: entry.value,
            source: entry.source,
            location: Location::from_zero_based(entry.file, entry.line, entry.character),
        }
    }
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entries = RqClient::default().list_variables(path, args.env.environment.as_deref())?;
    let formatter = get_formatter(&args.output.output);

    match args.output.output {
        OutputFormat::Json => {
            let views: Vec<VariableView> = entries.into_iter().map(Into::into).collect();
            print!("{}", formatter.format(&views));
        }
        OutputFormat::Text => {
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
    let entry = RqClient::default().get_variable(
        path,
        &args.name,
        args.env.environment.as_deref(),
        !args.no_var_interpolation,
        None,
    )?;
    let view = VariableView::from(entry);
    print!("{}", get_formatter(&args.output.output).format(&view));
    Ok(())
}

pub fn execute_refs(args: &RefsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = absolute_source(&args.source.source);
    let refs = RqClient::default().list_variable_references(&path, &args.name, None)?;
    print!(
        "{}",
        get_formatter(&args.output.output).format_list(
            &reference_views(refs),
            "References found:",
            "No references found"
        )
    );
    Ok(())
}
