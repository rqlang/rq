use crate::commands::shared::{EnvArgs, Location, OutputArgs, SourceArgs};
use crate::core::formatter::{get_formatter, OutputFormat};
use clap::{Args, Subcommand};
use rq_lib::RqClient;
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Serialize)]
struct AuthListView {
    name: String,
    #[serde(rename = "type")]
    auth_type: String,
}

#[derive(Serialize)]
struct AuthDetailsView {
    name: String,
    #[serde(rename = "type")]
    auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<String>,
    fields: BTreeMap<String, String>,
    #[serde(flatten)]
    location: Location,
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

    #[arg(long = "no-var-interpolation", help = "Skip variable interpolation")]
    pub no_var_interpolation: bool,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let auth_list = RqClient::default().list_auth(source_path)?;

    match args.output.output {
        OutputFormat::Json => {
            let views: Vec<AuthListView> = auth_list
                .into_iter()
                .map(|auth| AuthListView {
                    name: auth.name,
                    auth_type: auth.auth_type,
                })
                .collect();
            print!("{}", get_formatter(&args.output.output).format(&views));
        }
        OutputFormat::Text => {
            if auth_list.is_empty() {
                println!("No auth configurations found");
            } else {
                println!("Auth configurations found:");
                for auth in auth_list {
                    println!("- {} ({})", auth.name, auth.auth_type);
                }
            }
        }
    }

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);

    let (name, auth_type, fields, file, line, character) = RqClient::default().get_auth_details(
        source_path,
        &args.name,
        args.env_args.environment.as_deref(),
        !args.no_var_interpolation,
    )?;

    let view = AuthDetailsView {
        name,
        auth_type,
        environment: args.env_args.environment.clone(),
        fields: fields.into_iter().collect(),
        location: Location::from_zero_based(file, line, character),
    };
    print!("{}", get_formatter(&args.output.output).format(&view));

    Ok(())
}
