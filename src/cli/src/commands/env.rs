use crate::commands::shared::{render_names, FormatArgs, SourceArgs};
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
}

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub format: FormatArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let names = RqClient::default().list_environments(path)?;
    print!(
        "{}",
        render_names(
            args.format.format,
            names,
            "Environments found:",
            "No environments found in .rq files"
        )
    );

    Ok(())
}
