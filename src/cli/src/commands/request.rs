use crate::commands::shared::{EnvArgs, OutputArgs, SourceArgs};
use crate::commands::validators;
use crate::core::logger::Logger;
use clap::{Args, Subcommand};
use rq_lib::{RequestExecutionResult, RqClient};
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
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfigView>,
}

#[derive(Serialize)]
struct RequestDetailsJsonView {
    #[serde(rename = "Request")]
    name: String,
    #[serde(rename = "URL")]
    url: String,
    #[serde(rename = "Method")]
    method: String,
    #[serde(rename = "Headers")]
    headers: HashMap<String, String>,
    #[serde(rename = "Body", skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    timeout: Option<String>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    auth: Option<AuthConfigView>,
    file: String,
    line: usize,
    character: usize,
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

    #[arg(long = "no-var-interpolation", help = "Skip variable interpolation")]
    pub no_var_interpolation: bool,

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
    let (requests, parse_errors) = RqClient::default().list_requests(source_path)?;

    for e in &parse_errors {
        match args.output.output {
            crate::core::formatter::OutputFormat::Json => {
                eprintln!("{}", crate::core::error::error_to_json(e));
            }
            crate::core::formatter::OutputFormat::Text => {
                eprintln!("Warning: Failed to parse: {e}");
            }
        }
    }

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
        .ok_or("Request name is required")?
        .replace('.', "/");

    let details = RqClient::default().get_request_details(
        source_path,
        &name,
        args.env_args.environment.as_deref(),
        !args.no_var_interpolation,
        false,
        &[],
    )?;

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

    match args.output.output {
        crate::core::formatter::OutputFormat::Json => {
            let view = RequestDetailsJsonView {
                name: details.name,
                url: details.url,
                method: details.method,
                headers: headers_map,
                body: details.body,
                timeout: details.timeout,
                auth,
                file: details.file,
                line: details.line,
                character: details.character,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&view).unwrap_or_default()
            );
        }
        crate::core::formatter::OutputFormat::Text => {
            let formatter = crate::core::formatter::get_formatter(&args.output.output);
            let view = RequestDetailsView {
                name: details.name,
                url: details.url,
                method: details.method,
                headers: headers_map,
                body: details.body,
                timeout: details.timeout,
                auth,
            };
            print!("{}", formatter.format(&view));
        }
    }

    Ok(())
}

pub async fn execute_run(args: &RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let request_name = args
        .request_name_args
        .name
        .as_deref()
        .map(|n| n.replace('.', "/"));
    let (results, parse_warnings) = RqClient::default()
        .run(
            source_path,
            request_name.as_deref(),
            args.env_args.environment.as_deref(),
            &args.variable,
        )
        .await?;

    for w in &parse_warnings {
        match args.output.output {
            crate::core::formatter::OutputFormat::Json => {
                eprintln!("{}", crate::core::error::error_to_json(w));
            }
            crate::core::formatter::OutputFormat::Text => {
                eprintln!("Warning: Failed to parse: {w}");
            }
        }
    }

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
