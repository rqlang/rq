use crate::commands::shared::{OutputArgs, SourceArgs};
use clap::{Args, Subcommand};

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

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(&args.source.source);
    let env_list = crate::client::RqClient::list_environments(path)?;

    let formatter = crate::core::formatter::get_formatter(&args.output.output);
    print!(
        "{}",
        formatter.format_list(
            &env_list,
            "Environments found:",
            "No environments found in .rq files"
        )
    );

    Ok(())
}
