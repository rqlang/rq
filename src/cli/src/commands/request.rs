use crate::commands::shared::{
    print_warnings, render_names, EnvArgs, FormatArgs, Location, SourceArgs,
};
use crate::commands::validators;
use crate::core::error::RqError;
use crate::core::formatter::{pretty_body, render, to_json, OutputFormat, TextBlock};
use clap::{Args, Subcommand};
use rq_lib::client::models::RequestDetails;
use rq_lib::lint::LintScope;
use rq_lib::{RequestExecutionResult, RqClient};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::str::FromStr;

#[derive(Serialize)]
struct AuthConfigView {
    name: String,
    #[serde(rename = "type")]
    auth_type: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintParts {
    meta: bool,
    headers: bool,
    body: bool,
}

impl Default for PrintParts {
    fn default() -> Self {
        Self {
            meta: true,
            headers: false,
            body: true,
        }
    }
}

impl FromStr for PrintParts {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err("Expected at least one of m, h, b".to_string());
        }
        let mut parts = Self {
            meta: false,
            headers: false,
            body: false,
        };
        for part in value.chars() {
            match part {
                'm' => parts.meta = true,
                'h' => parts.headers = true,
                'b' => parts.body = true,
                other => return Err(format!("Invalid part '{other}', expected any of m, h, b")),
            }
        }
        Ok(parts)
    }
}

#[derive(Serialize)]
struct ResultsEnvelope<T: Serialize> {
    results: Vec<T>,
}

#[derive(Serialize)]
struct PrintedResultView<'a> {
    request_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_headers: Option<&'a HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

impl<'a> PrintedResultView<'a> {
    fn new(result: &'a RequestExecutionResult, parts: PrintParts) -> Self {
        Self {
            request_name: &result.request_name,
            method: parts.meta.then_some(result.method.as_str()),
            url: parts.meta.then_some(result.url.as_str()),
            status: parts.meta.then_some(result.status),
            elapsed_ms: parts.meta.then_some(result.elapsed_ms),
            response_headers: parts.headers.then_some(&result.response_headers),
            body: parts.body.then_some(result.body.as_str()),
        }
    }
}

struct ExecutionResultsView {
    results: Vec<RequestExecutionResult>,
    parts: Option<PrintParts>,
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
    fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Json => self.to_json(),
            OutputFormat::Text => self.to_text(),
        }
    }

    fn to_json(&self) -> String {
        match self.parts {
            None => to_json(&ResultsEnvelope {
                results: self.results.iter().collect(),
            }),
            Some(parts) => to_json(&ResultsEnvelope {
                results: self
                    .results
                    .iter()
                    .map(|result| PrintedResultView::new(result, parts))
                    .collect(),
            }),
        }
    }

    fn to_text(&self) -> String {
        let parts = self.parts.unwrap_or_default();
        self.results
            .iter()
            .map(|result| render_execution_result(result, parts))
            .filter(|rendered| !rendered.is_empty())
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
    pub format: FormatArgs,
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
    pub format: FormatArgs,
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
    pub format: FormatArgs,

    #[arg(
        short = 'p',
        long = "print",
        value_name = "PARTS",
        help = "Response parts to print: m (meta), h (headers), b (body) [default: mb, or all with json]"
    )]
    pub print: Option<PrintParts>,

    #[arg(
        long = "no-lint",
        help = "Skip the lint summary printed before running"
    )]
    pub no_lint: bool,
}

pub fn execute_list(args: &ListArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let (requests, parse_errors) = RqClient::default().list_requests(source_path)?;
    print_warnings(&parse_errors, args.format.format);

    let names = requests.into_iter().map(|r| r.name).collect();
    print!(
        "{}",
        render_names(
            args.format.format,
            names,
            "Requests found:",
            "No requests found"
        )
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

    let details = RqClient::default()
        .get_request_details(
            source_path,
            &name,
            args.env_args.environment.as_deref(),
            !args.no_var_interpolation,
            false,
            &[],
        )
        .map_err(|e| with_typed_request_name(e, args.request_name_args.name.as_deref()))?;

    let view = RequestDetailsView::from(details);
    print!(
        "{}",
        render(args.format.format, &view, RequestDetailsView::to_text)
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
    let client = RqClient::default();
    if !args.no_lint {
        print_lint_summary(&client, &args.source.source, args.format.format);
    }
    let (results, warnings) = client
        .run(
            source_path,
            request_name.as_deref(),
            args.env_args.environment.as_deref(),
            &args.variable,
        )
        .await
        .map_err(|e| with_typed_request_name(e, args.request_name_args.name.as_deref()))?;

    print_warnings(&warnings, args.format.format);

    let view = ExecutionResultsView {
        results,
        parts: args.print,
    };
    print!("{}", view.render(args.format.format));

    Ok(())
}

fn print_lint_summary(client: &RqClient, source: &str, output: OutputFormat) {
    let Ok(diagnostics) = client.lint_path(Path::new(source), LintScope::SourceOnly) else {
        return;
    };
    if diagnostics.is_empty() {
        return;
    }
    let count = diagnostics.len();
    let noun = if count == 1 { "warning" } else { "warnings" };
    let source = quoted_source(source);
    let message = format!("{count} lint {noun} found, run `rq check -s {source}` for details");
    print_warnings(&[RqError::Generic(message)], output);
}

fn quoted_source(source: &str) -> String {
    if source.chars().any(char::is_whitespace) {
        format!("\"{source}\"")
    } else {
        source.to_string()
    }
}

fn with_typed_request_name(error: RqError, typed_name: Option<&str>) -> RqError {
    match (error, typed_name) {
        (RqError::RequestNotFound(_), Some(typed_name)) if typed_name.contains('.') => {
            RqError::RequestNotFound(typed_name.to_string())
        }
        (error, _) => error,
    }
}

fn render_execution_result(result: &RequestExecutionResult, parts: PrintParts) -> String {
    let sections = [
        parts.meta.then(|| render_meta(result)),
        parts
            .headers
            .then(|| render_headers(&result.response_headers)),
        parts
            .body
            .then(|| pretty_body(&result.body).trim_end().to_string()),
    ];
    let rendered = sections
        .into_iter()
        .flatten()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if rendered.is_empty() {
        rendered
    } else {
        format!("{rendered}\n")
    }
}

fn render_meta(result: &RequestExecutionResult) -> String {
    let reason = http::StatusCode::from_u16(result.status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .map(|reason| format!(" {reason}"))
        .unwrap_or_default();
    format!(
        "{}  {} {}\n{}{reason} · {} ms",
        result.request_name, result.method, result.url, result.status, result.elapsed_ms
    )
}

fn render_headers(headers: &HashMap<String, String>) -> String {
    headers
        .iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}
