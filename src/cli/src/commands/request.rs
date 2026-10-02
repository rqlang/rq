use crate::commands::shared::{print_warnings, EnvArgs, Location, OutputArgs, SourceArgs};
use crate::commands::validators;
use crate::core::formatter::{pretty_body, render, render_list, to_json, OutputFormat, TextBlock};
use clap::{Args, Subcommand};
use rq_lib::client::models::{RequestDetails, RequestInfo};
use rq_lib::{RequestExecutionResult, RqClient};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Serialize)]
struct AuthConfigView {
    name: String,
    #[serde(rename = "type")]
    auth_type: String,
}

#[derive(Serialize)]
struct RequestListView {
    name: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint_column: Option<usize>,
}

impl From<RequestInfo> for RequestListView {
    fn from(info: RequestInfo) -> Self {
        Self {
            name: info.name,
            file: info.file,
            endpoint: info.endpoint,
            endpoint_file: info.endpoint_file,
            endpoint_line: info.endpoint_line.map(|line| line + 1),
            endpoint_column: info.endpoint_character.map(|character| character + 1),
        }
    }
}

#[derive(Serialize)]
struct RequestDetailsView {
    name: String,
    url: String,
    method: String,
    headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<AuthConfigView>,
    #[serde(flatten)]
    location: Location,
}

impl From<RequestDetails> for RequestDetailsView {
    fn from(details: RequestDetails) -> Self {
        let auth = match (details.auth_name, details.auth_type) {
            (Some(name), Some(auth_type)) => Some(AuthConfigView { name, auth_type }),
            _ => None,
        };
        Self {
            name: details.name,
            url: details.url,
            method: details.method,
            headers: details.headers.into_iter().collect(),
            body: details.body,
            timeout: details.timeout,
            auth,
            location: Location::from_zero_based(details.file, details.line, details.character),
        }
    }
}

#[derive(Serialize)]
struct ExecutionResultsView {
    results: Vec<RequestExecutionResult>,
}

impl RequestDetailsView {
    fn to_text(&self) -> String {
        let auth = self
            .auth
            .as_ref()
            .map(|auth| format!("{} ({})", auth.name, auth.auth_type));
        TextBlock::default()
            .field("name", &self.name)
            .field("method", &self.method)
            .field("url", &self.url)
            .map("headers", &self.headers)
            .optional("body", self.body.as_deref())
            .optional("timeout", self.timeout.as_deref())
            .optional("auth", auth)
            .field("location", &self.location)
            .build()
    }
}

impl ExecutionResultsView {
    fn to_text(&self) -> String {
        self.results
            .iter()
            .map(render_execution_result)
            .collect::<Vec<_>>()
            .join("\n")
    }
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
    print_warnings(&parse_errors, args.output.output);

    match args.output.output {
        OutputFormat::Json => {
            let views: Vec<RequestListView> = requests.into_iter().map(Into::into).collect();
            print!("{}", to_json(&views));
        }
        OutputFormat::Text => {
            let names: Vec<String> = requests.into_iter().map(|r| r.name).collect();
            print!(
                "{}",
                render_list(&names, "Requests found:", "No requests found")
            );
        }
    }

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

    let view = RequestDetailsView::from(details);
    print!(
        "{}",
        render(args.output.output, &view, RequestDetailsView::to_text)
    );

    Ok(())
}

pub async fn execute_run(args: &RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let request_name = args
        .request_name_args
        .name
        .as_deref()
        .map(|n| n.replace('.', "/"));
    let (results, warnings) = RqClient::default()
        .run(
            source_path,
            request_name.as_deref(),
            args.env_args.environment.as_deref(),
            &args.variable,
        )
        .await?;

    print_warnings(&warnings, args.output.output);

    let view = ExecutionResultsView { results };
    print!(
        "{}",
        render(args.output.output, &view, ExecutionResultsView::to_text)
    );

    Ok(())
}

fn render_execution_result(result: &RequestExecutionResult) -> String {
    let reason = http::StatusCode::from_u16(result.status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .map(|reason| format!(" {reason}"))
        .unwrap_or_default();
    let mut out = format!(
        "{}  {} {}\n{}{reason} · {} ms\n",
        result.request_name, result.method, result.url, result.status, result.elapsed_ms
    );
    if !result.body.is_empty() {
        out.push('\n');
        out.push_str(pretty_body(&result.body).trim_end());
        out.push('\n');
    }
    out
}
