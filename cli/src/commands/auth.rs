use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use clap::{Args, Subcommand};
use serde::Serialize;
use std::{collections::HashMap, path::Path};

#[derive(Serialize)]
pub struct AuthDetailsView {
    #[serde(rename = "Auth Configuration")]
    pub name: String,
    #[serde(rename = "Type")]
    pub auth_type: String,
    #[serde(rename = "Environment", skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(rename = "Fields")]
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Args)]
#[command(about = "Manage authentication")]
pub struct AuthCommand {
    #[command(subcommand)]
    pub command: AuthSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthSubcommand {
    #[command(about = "List authentication configurations")]
    List(ListArgs),
    #[command(about = "Show authentication details")]
    Show(ShowArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the auth configuration to show",
        value_parser = crate::commands::validators::validate_name
    )]
    pub name: String,

    #[command(flatten)]
    pub env_args: EnvArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let auth_list = crate::client::RqClient::list_auth(source_path)?;

    let formatter = crate::core::formatter::get_formatter(&args.output.output);
    print!(
        "{}",
        formatter.format_list(
            &auth_list,
            "Auth configurations found:",
            "No auth configurations found"
        )
    );

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let (auth_name, auth_type_str, fields) = crate::client::RqClient::get_auth_details(
        source_path,
        &args.name,
        args.env_args.environment.as_deref(),
    )?;

    let formatter = crate::core::formatter::get_formatter(&args.output.output);

    let view = AuthDetailsView {
        name: auth_name,
        auth_type: auth_type_str,
        environment: args.env_args.environment.clone(),
        fields,
    };
    print!("{}", formatter.format(&view));

    Ok(())
}
