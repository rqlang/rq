use crate::client::{RequestExecutionResult, RqClient, RqConfig};
use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use crate::commands::validators;
use crate::core::logger::Logger;
use clap::{Args, Subcommand};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize)]
pub struct AuthConfigView {
    pub name: String,
    #[serde(rename = "type")]
    pub auth_type: String,
}

#[derive(Serialize)]
pub struct RequestDetailsView {
    #[serde(rename = "Request")]
    pub name: String,
    #[serde(rename = "URL")]
    pub url: String,
    #[serde(rename = "Method")]
    pub method: String,
    #[serde(rename = "Headers")]
    pub headers: HashMap<String, String>,
    #[serde(rename = "Body", skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfigView>,
}

#[derive(Serialize)]
pub struct ExecutionResultsView {
    pub results: Vec<RequestExecutionResult>,
}

#[derive(Debug, Args)]
#[command(about = "Manage requests")]
pub struct RequestCommand {
    #[command(subcommand)]
    pub command: RequestSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RequestSubcommand {
    #[command(about = "List requests")]
    List(ListArgs),
    #[command(about = "Show request details")]
    Show(ShowArgs),
    #[command(about = "Run a request")]
    Run(RunArgs),
}

#[derive(Debug, Args)]
pub struct RequestNameArgs {
    #[arg(
        short = 'n',
        long = "name",
        help = "Name of the request",
        value_parser = validators::validate_name
    )]
    pub name: Option<String>,
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

    #[command(flatten)]
    pub request_name_args: RequestNameArgs,

    #[command(flatten)]
    pub env_args: EnvArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub source: SourceArgs,

    #[command(flatten)]
    pub request_name_args: RequestNameArgs,

    #[command(flatten)]
    pub env_args: EnvArgs,

    #[arg(
        short = 'v',
        long = "variable",
        value_name = "NAME=VALUE",
        help = "Override requests variables",
        value_parser = validators::validate_variable
    )]
    pub variable: Vec<String>,

    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let requests = RqClient::list_requests(source_path)?;

    let formatter = crate::core::formatter::get_formatter(&args.output.output);
    print!(
        "{}",
        formatter.format_list(&requests, "", "No requests found")
    );

    Ok(())
}

pub fn execute_show(args: &ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let name = args
        .request_name_args
        .name
        .as_deref()
        .ok_or("Request name is required")?;
    let details =
        RqClient::get_request_details(source_path, name, args.env_args.environment.as_deref())?;

    let formatter = crate::core::formatter::get_formatter(&args.output.output);

    let auth = if let (Some(auth_name), Some(auth_type)) = (&details.auth_name, &details.auth_type)
    {
        Some(AuthConfigView {
            name: auth_name.to_string(),
            auth_type: auth_type.to_string(),
        })
    } else {
        None
    };

    let mut headers_map = HashMap::new();
    for (key, value) in &details.headers {
        headers_map.insert(key.clone(), value.clone());
    }

    let view = RequestDetailsView {
        name: details.name,
        url: details.url,
        method: details.method,
        headers: headers_map,
        body: details.body,
        auth,
    };
    print!("{}", formatter.format(&view));

    Ok(())
}

pub async fn execute_run(args: &RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = RqConfig {
        source_path: args.source.source.clone(),
        request_name: args.request_name_args.name.clone(),
        environment: args.env_args.environment.clone(),
        variables: args.variable.clone(),
        output_format: args.output.output.to_string(),
    };

    let client = RqClient::new(config);
    let results = client.run().await?;

    for result in &results {
        let elapsed_str = format!("{} ms", result.elapsed_ms);
        Logger::debug("\n--- HTTP Response ---");
        Logger::debug(&format!(
            "Response status: {} ({})",
            result.status, elapsed_str
        ));
        Logger::debug("Response Headers:");
        for (key, value) in &result.response_headers {
            Logger::debug(&format!("  {key}: {value}"));
        }
        Logger::debug("");
        Logger::debug("--- End Response ---\n");
    }

    let formatter = crate::core::formatter::get_formatter(&args.output.output);
    let view = ExecutionResultsView { results };
    print!("{}", formatter.format(&view));

    Ok(())
}
