use crate::commands::shared::{
    absolute_source, render_references, Location, OutputArgs, SourceArgs,
};
use crate::commands::validators;
use crate::core::formatter::{render, render_list, to_json, OutputFormat, TextBlock};
use clap::{Args, Subcommand};
use rq_lib::client::models::EndpointEntry;
use rq_lib::RqClient;
use serde::Serialize;

#[derive(Args)]
#[command(name = "ep")]
#[command(about = "Manage endpoints")]
pub struct EpCommand {
    #[command(subcommand)]
    pub command: EpSubcommand,
}

#[derive(Subcommand)]
pub enum EpSubcommand {
    #[command(about = "List endpoints")]
    List(ListArgs),
    #[command(about = "Show endpoint location")]
    Show(ShowArgs),
    #[command(about = "Find all references to an endpoint")]
    Refs(RefsArgs),
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

#[derive(Serialize)]
struct EndpointView {
    name: String,
    is_template: bool,
    #[serde(flatten)]
    location: Location,
}

impl EndpointView {
    fn to_text(&self) -> String {
        TextBlock::default()
            .field("name", &self.name)
            .field("template", self.is_template)
            .field("location", &self.location)
            .build()
    }
}

impl From<EndpointEntry> for EndpointView {
    fn from(entry: EndpointEntry) -> Self {
        Self {
            name: entry.name,
            is_template: entry.is_template,
            location: Location::from_zero_based(entry.file, entry.line, entry.character),
        }
    }
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entries = RqClient::default().list_endpoints(path)?;
    match args.output.output {
        OutputFormat::Json => {
            let views: Vec<EndpointView> = entries.into_iter().map(Into::into).collect();
            print!("{}", to_json(&views));
        }
        OutputFormat::Text => {
            let names: Vec<String> = entries.into_iter().map(|e| e.name).collect();
            print!(
                "{}",
                render_list(
                    &names,
                    "Endpoints found:",
                    "No endpoints found in .rq files"
                )
            );
        }
    }

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let entry = RqClient::default().get_endpoint(path, &args.name, None)?;
    let view = EndpointView::from(entry);
    print!(
        "{}",
        render(args.output.output, &view, EndpointView::to_text)
    );
    Ok(())
}

pub fn execute_refs(args: &RefsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = absolute_source(&args.source.source);
    let refs = RqClient::default().list_endpoint_references(&path, &args.name, None)?;
    print!("{}", render_references(refs, args.output.output));
    Ok(())
}
