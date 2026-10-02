use crate::commands::shared::{Location, OutputArgs, SourceArgs};
use crate::core::formatter::{render, render_list, to_json, OutputFormat, TextBlock};
use clap::{Args, Subcommand};
use rq_lib::RqClient;
use serde::Serialize;

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

#[derive(Serialize)]
struct EnvironmentListView {
    name: String,
}

#[derive(Serialize)]
struct EnvironmentView {
    name: String,
    #[serde(flatten)]
    location: Location,
}

impl EnvironmentView {
    fn to_text(&self) -> String {
        TextBlock::default()
            .field("name", &self.name)
            .field("location", &self.location)
            .build()
    }
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let env_list = RqClient::default().list_environments(path)?;
    match args.output.output {
        OutputFormat::Json => {
            let views: Vec<EnvironmentListView> = env_list
                .into_iter()
                .map(|name| EnvironmentListView { name })
                .collect();
            print!("{}", to_json(&views));
        }
        OutputFormat::Text => {
            print!(
                "{}",
                render_list(
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
    let view = EnvironmentView {
        name: entry.name,
        location: Location::from_zero_based(entry.file, entry.line, entry.character),
    };
    print!(
        "{}",
        render(args.output.output, &view, EnvironmentView::to_text)
    );
    Ok(())
}
