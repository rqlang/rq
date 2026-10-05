use crate::commands::shared::{OutputArgs, SourceArgs};
use crate::core::formatter::{render_list, to_json, OutputFormat};
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
}

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Serialize)]
struct EnvironmentListView {
    name: String,
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
