use crate::commands::shared::{
    print_warnings, render_names, EnvArgs, FormatArgs, Location, SourceArgs,
};
use crate::commands::validators;
use crate::core::error::{BodyNotWritten, RqError};
use crate::core::formatter::{render, to_json, OutputFormat, TextBlock};
use clap::{Args, Subcommand};
use rq_lib::client::models::{PreparedRequest, RequestDetails, RequestTarget};
use rq_lib::lint::LintScope;
use rq_lib::response_body::{
    temp_download_dir, BinaryBody, BodyFileInfo, OutputBody, OutputSkipped, OutputTarget,
};
use rq_lib::{RequestExecutionResult, ResponseBody, RqClient};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    url: String,
    method: String,
    headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<AuthConfigView>,
    confirmation_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<OutputView>,
    #[serde(flatten)]
    location: Location,
}

#[derive(Serialize)]
struct OutputView {
    path: String,
    applies_to: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl From<OutputTarget> for OutputView {
    fn from(target: OutputTarget) -> Self {
        let path = if target.is_folder {
            format!("{}{}", target.path, std::path::MAIN_SEPARATOR)
        } else {
            target.path
        };
        Self {
            path,
            applies_to: if target.applies_to_every_body {
                "every body"
            } else {
                "binary bodies"
            },
            error: target.error,
        }
    }
}

impl From<RequestDetails> for RequestDetailsView {
    fn from(details: RequestDetails) -> Self {
        let auth = match (details.auth_name, details.auth_type) {
            (Some(name), Some(auth_type)) => Some(AuthConfigView { name, auth_type }),
            _ => None,
        };
        Self {
            name: details.name,
            description: details.description,
            url: details.url,
            method: details.method,
            headers: details.headers.into_iter().collect(),
            body: details.body,
            timeout: details.timeout,
            auth,
            confirmation_required: details.confirmation_required,
            output: details.output.map(OutputView::from),
            location: Location::from_zero_based(
                details.request_file,
                details.line,
                details.character,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintParts {
    meta: bool,
    headers: bool,
    body: bool,
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
struct ResultsEnvelope<'a> {
    results: Vec<PrintedResultView<'a>>,
}

#[derive(Serialize)]
struct PrintedResultView<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    request_name: Option<&'a str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    body_file: Option<&'a SavedBodyView>,
}

impl<'a> PrintedResultView<'a> {
    fn new(output: &'a ExecutionOutput, parts: PrintParts) -> Self {
        let result = &output.result;
        let text_body = match &result.body {
            ResponseBody::Text(text) => Some(text.as_str()),
            ResponseBody::Binary(_) | ResponseBody::Output(_) => None,
        };
        Self {
            request_name: parts.meta.then_some(result.request_name.as_str()),
            method: parts.meta.then_some(result.method.as_str()),
            url: parts.meta.then_some(result.url.as_str()),
            status: parts.meta.then_some(result.status),
            elapsed_ms: parts.meta.then_some(result.elapsed_ms),
            response_headers: parts.headers.then_some(&result.response_headers),
            body: text_body.filter(|_| parts.body),
            body_file: output.saved_body.as_ref().filter(|_| parts.body),
        }
    }
}

#[derive(Serialize)]
struct SavedBodyView {
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    save_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,
    size: usize,
    binary: bool,
}

impl SavedBodyView {
    fn save(body: &BinaryBody) -> Self {
        Self::new(body.save_in(&temp_download_dir()), &body.info, true)
    }

    fn write(body: &OutputBody) -> Self {
        Self::new(body.save(), &body.info, body.binary)
    }

    fn new(
        saved: Result<PathBuf, rq_lib::error::RqError>,
        info: &BodyFileInfo,
        binary: bool,
    ) -> Self {
        Self {
            path: saved.as_ref().ok().map(|path| path.display().to_string()),
            save_error: saved.err().map(|e| e.to_string()),
            media_type: info.media_type.clone(),
            size: info.size,
            binary,
        }
    }

    fn to_text(&self) -> String {
        let description = match &self.media_type {
            Some(media_type) => format!("{media_type}, {} bytes", self.size),
            None => format!("{} bytes", self.size),
        };
        let label = if self.binary { "binary body" } else { "body" };
        match (&self.path, &self.save_error) {
            (Some(path), _) => format!("{label} ({description}) saved to {path}"),
            (None, Some(error)) => {
                format!("{label} ({description}) could not be saved: {error}")
            }
            (None, None) => format!("{label} ({description}) not saved"),
        }
    }
}

struct ExecutionOutput {
    result: RequestExecutionResult,
    saved_body: Option<SavedBodyView>,
    write_error: Option<OutputSkipped>,
}

impl ExecutionOutput {
    fn new(mut result: RequestExecutionResult, parts: PrintParts) -> Self {
        let saved_body = match &result.body {
            ResponseBody::Binary(body) if parts.body => Some(SavedBodyView::save(body)),
            ResponseBody::Output(body) => Some(SavedBodyView::write(body)),
            _ => None,
        };
        let write_error = Self::write_error(&result.body, saved_body.as_ref());
        match Self::unwritten_text(&result.body, saved_body.as_ref()) {
            Some((text, skipped)) => {
                result.body = ResponseBody::Text(text);
                result.output_skipped = Some(skipped);
                Self {
                    result,
                    saved_body: None,
                    write_error,
                }
            }
            None => Self {
                result,
                saved_body,
                write_error,
            },
        }
    }

    fn write_error(body: &ResponseBody, saved: Option<&SavedBodyView>) -> Option<OutputSkipped> {
        let (ResponseBody::Output(output), Some(error)) =
            (body, saved.and_then(|saved| saved.save_error.as_ref()))
        else {
            return None;
        };
        Some(OutputSkipped {
            path: output.path.display().to_string(),
            reason: error.clone(),
        })
    }

    fn unwritten_text(
        body: &ResponseBody,
        saved: Option<&SavedBodyView>,
    ) -> Option<(String, OutputSkipped)> {
        let ResponseBody::Output(output) = body else {
            return None;
        };
        let skipped = Self::write_error(body, saved)?;
        (!output.binary).then(|| (String::from_utf8_lossy(&output.bytes).into_owned(), skipped))
    }
}

struct ExecutionResultsView {
    results: Vec<ExecutionOutput>,
    parts: PrintParts,
}

impl RequestDetailsView {
    fn to_text(&self) -> String {
        let auth = self
            .auth
            .as_ref()
            .map(|auth| format!("{} ({})", auth.name, auth.auth_type));
        TextBlock::default()
            .field("name", &self.name)
            .optional("description", self.description.as_deref())
            .field("method", &self.method)
            .field("url", &self.url)
            .map("headers", &self.headers)
            .optional("body", self.body.as_deref())
            .optional("timeout", self.timeout.as_deref())
            .optional("auth", auth)
            .field("confirmation_required", self.confirmation_required)
            .optional(
                "output",
                self.output
                    .as_ref()
                    .map(|output| format!("{} ({})", output.path, output.applies_to)),
            )
            .optional(
                "output_error",
                self.output
                    .as_ref()
                    .and_then(|output| output.error.as_deref()),
            )
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
        to_json(&ResultsEnvelope {
            results: self
                .results
                .iter()
                .map(|result| PrintedResultView::new(result, self.parts))
                .collect(),
        })
    }

    fn to_text(&self) -> String {
        self.results
            .iter()
            .map(|result| render_execution_result(result, self.parts))
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
        help = "Name of the request (required)",
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
        default_value = "mb",
        help = "Response parts to print, combinable (e.g. mhb): m (meta), h (headers), b (body)"
    )]
    pub print: PrintParts,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        help = "Write the response body to PATH, or into it when PATH ends with a separator",
        value_parser = validators::validate_output
    )]
    pub output: Option<String>,

    #[arg(
        long = "no-lint",
        help = "Skip the lint summary printed before running"
    )]
    pub no_lint: bool,

    #[arg(
        short = 'y',
        long = "yes",
        help = "Send a request marked [confirm] without asking"
    )]
    pub yes: bool,
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
    let client = RqClient::default();
    let typed_name = required_request_name(
        &client,
        &args.request_name_args,
        &args.source.source,
        args.format.format,
    )?;
    let name = typed_name.replace('.', "/");

    let (details, warnings) = client
        .get_request_details(
            RequestTarget::new(source_path, &name),
            args.env_args.environment.as_deref(),
            !args.no_var_interpolation,
            false,
            &[],
        )
        .map_err(|e| with_typed_request_name(e, typed_name))?;

    print_warnings(&warnings, args.format.format);

    let view = RequestDetailsView::from(details);
    print!(
        "{}",
        render(args.format.format, &view, RequestDetailsView::to_text)
    );

    Ok(())
}

pub async fn execute_run(args: &RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(&args.source.source);
    let client = RqClient::default();
    let typed_name = required_request_name(
        &client,
        &args.request_name_args,
        &args.source.source,
        args.format.format,
    )?;
    let request_name = typed_name.replace('.', "/");
    if !args.no_lint {
        print_lint_summary(&client, &args.source.source, args.format.format);
    }
    let result = client
        .run(
            RequestTarget::new(source_path, &request_name).with_output(args.output.as_deref()),
            args.env_args.environment.as_deref(),
            &args.variable,
            &|warnings| print_warnings(warnings, args.format.format),
            &|prepared| confirm_request(prepared, args.env_args.environment.as_deref(), args.yes),
        )
        .await
        .map_err(|e| with_typed_request_name(e, typed_name))?;
    let output = ExecutionOutput::new(result, args.print);
    let unwritten_output = output.write_error.clone().filter(|_| args.output.is_some());
    if let Some(skipped) = output
        .result
        .output_skipped
        .as_ref()
        .filter(|_| unwritten_output.is_none())
    {
        print_warnings(&[output_skipped_warning(skipped)], args.format.format);
    }

    let view = ExecutionResultsView {
        results: vec![output],
        parts: args.print,
    };
    print!("{}", view.render(args.format.format));

    match unwritten_output {
        Some(skipped) => Err(Box::new(BodyNotWritten {
            path: skipped.path,
            reason: skipped.reason,
        })),
        None => Ok(()),
    }
}

fn output_skipped_warning(skipped: &OutputSkipped) -> RqError {
    RqError::Generic(format!(
        "Body not written to {}: {}",
        skipped.path, skipped.reason
    ))
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

const LISTED_NAMES_LIMIT: usize = 10;

fn required_request_name<'a>(
    client: &RqClient,
    args: &'a RequestNameArgs,
    source: &str,
    output: OutputFormat,
) -> Result<&'a str, RqError> {
    if let Some(name) = args.name.as_deref() {
        return Ok(name);
    }
    let (requests, parse_errors) = client.list_requests(Path::new(source))?;
    print_warnings(&parse_errors, output);
    let names = requests
        .into_iter()
        .map(|request| request.name.replace('/', "."))
        .collect::<BTreeSet<_>>();
    Err(RqError::Generic(missing_name_message(names, source)))
}

fn missing_name_message(names: BTreeSet<String>, source: &str) -> String {
    let source = quoted_source(source);
    if names.is_empty() {
        return format!("--name is required, but {source} has no requests");
    }
    let hidden = names.len().saturating_sub(LISTED_NAMES_LIMIT);
    let shown = names
        .into_iter()
        .take(LISTED_NAMES_LIMIT)
        .collect::<Vec<_>>()
        .join(", ");
    if hidden == 0 {
        format!("--name is required. Requests in {source}: {shown}")
    } else {
        format!(
            "--name is required. Requests in {source}: {shown} and {hidden} more, run `rq request list -s {source}` to see them all"
        )
    }
}

fn with_typed_request_name(error: RqError, typed_name: &str) -> RqError {
    match error {
        RqError::RequestNotFound(_) if typed_name.contains('.') => {
            RqError::RequestNotFound(typed_name.to_string())
        }
        error => error,
    }
}

fn render_execution_result(output: &ExecutionOutput, parts: PrintParts) -> String {
    let result = &output.result;
    let sections = [
        parts.meta.then(|| render_meta(result)),
        parts
            .headers
            .then(|| render_headers(&result.response_headers)),
        parts.body.then(|| render_body(output)),
    ];
    let rendered = sections
        .into_iter()
        .flatten()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if rendered.is_empty() || rendered.ends_with('\n') {
        rendered
    } else {
        format!("{rendered}\n")
    }
}

fn render_body(output: &ExecutionOutput) -> String {
    match (&output.result.body, &output.saved_body) {
        (ResponseBody::Text(text), _) => text.clone(),
        (_, Some(saved_body)) => saved_body.to_text(),
        (_, None) => String::new(),
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

fn confirm_request(
    prepared: &PreparedRequest,
    environment: Option<&str>,
    assume_yes: bool,
) -> Result<(), RqError> {
    if assume_yes {
        return Ok(());
    }
    let name = &prepared.request.name;
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(RqError::NotConfirmed(format!(
            "Request '{name}' is marked [confirm] and was not sent: run it from a terminal to be asked, or pass --yes to send it without asking"
        )));
    }
    eprint!("{} [y/N] ", confirmation_question(prepared, environment));
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if is_yes(&answer) {
        Ok(())
    } else {
        Err(RqError::NotConfirmed(format!(
            "Request '{name}' was not sent: not confirmed"
        )))
    }
}

fn confirmation_question(prepared: &PreparedRequest, environment: Option<&str>) -> String {
    let target = format!(
        "{} {}",
        prepared.request.method.as_str(),
        prepared.request.url
    );
    match environment {
        Some(environment) => format!("Run {target} against the `{environment}` environment?"),
        None => format!("Run {target}?"),
    }
}

fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::{
        is_yes, render_execution_result, ExecutionOutput, PrintParts, RequestExecutionResult,
        SavedBodyView,
    };
    use rq_lib::response_body::{BodyFileInfo, OutputBody};
    use rq_lib::ResponseBody;
    use std::collections::HashMap;
    use std::str::FromStr;

    fn result_with(body: ResponseBody) -> RequestExecutionResult {
        RequestExecutionResult {
            request_name: "r".to_string(),
            method: "GET".to_string(),
            url: "http://localhost".to_string(),
            status: 200,
            elapsed_ms: 1,
            request_headers: HashMap::new(),
            response_headers: HashMap::new(),
            body,
            output_skipped: None,
        }
    }

    fn render_body(body: &str) -> Result<String, String> {
        let output = ExecutionOutput {
            result: result_with(ResponseBody::Text(body.to_string())),
            saved_body: None,
            write_error: None,
        };
        Ok(render_execution_result(&output, PrintParts::from_str("b")?))
    }

    fn saved_body(media_type: Option<&str>) -> SavedBodyView {
        SavedBodyView {
            path: Some("/tmp/rq/id/logo.png".to_string()),
            save_error: None,
            media_type: media_type.map(str::to_string),
            size: 42,
            binary: true,
        }
    }

    fn binary_body() -> ResponseBody {
        ResponseBody::from_response(&HashMap::new(), vec![0, 0xff], "r")
    }

    #[test]
    fn body_keeps_its_trailing_whitespace() -> Result<(), String> {
        let target = render_body("text  \n\n")?;
        assert_eq!(target, "text  \n\n");
        Ok(())
    }

    #[test]
    fn body_without_trailing_newline_gets_one() -> Result<(), String> {
        let target = render_body("text")?;
        assert_eq!(target, "text\n");
        Ok(())
    }

    #[test]
    fn saved_binary_body_shows_media_type_size_and_path() {
        let target = saved_body(Some("image/png")).to_text();
        assert_eq!(
            target,
            "binary body (image/png, 42 bytes) saved to /tmp/rq/id/logo.png"
        );
    }

    #[test]
    fn saved_binary_body_without_media_type_shows_size_and_path() {
        let target = saved_body(None).to_text();
        assert_eq!(
            target,
            "binary body (42 bytes) saved to /tmp/rq/id/logo.png"
        );
    }

    #[test]
    fn body_written_to_its_output_is_not_called_binary() {
        let target = SavedBodyView {
            path: Some("out/users.json".to_string()),
            binary: false,
            ..saved_body(Some("application/json"))
        }
        .to_text();
        assert_eq!(
            target,
            "body (application/json, 42 bytes) saved to out/users.json"
        );
    }

    #[test]
    fn text_body_that_could_not_be_written_is_printed_with_the_reason() -> Result<(), String> {
        let folder = std::env::temp_dir();
        let body = ResponseBody::Output(OutputBody {
            info: BodyFileInfo {
                media_type: Some("application/json".to_string()),
                file_name: "users.json".to_string(),
                size: 2,
            },
            bytes: b"{}".to_vec(),
            path: folder.clone(),
            binary: false,
        });
        let target = ExecutionOutput::new(result_with(body), PrintParts::from_str("b")?);
        assert_eq!(
            (
                render_execution_result(&target, PrintParts::from_str("b")?),
                target.result.output_skipped.map(|skipped| skipped.path)
            ),
            ("{}\n".to_string(), Some(folder.display().to_string()))
        );
        Ok(())
    }

    #[test]
    fn unsaved_binary_body_shows_why() {
        let target = SavedBodyView {
            path: None,
            save_error: Some("File name too long".to_string()),
            ..saved_body(Some("image/png"))
        }
        .to_text();
        assert_eq!(
            target,
            "binary body (image/png, 42 bytes) could not be saved: File name too long"
        );
    }

    #[test]
    fn binary_body_is_not_saved_when_the_body_is_not_printed() -> Result<(), String> {
        let target = ExecutionOutput::new(result_with(binary_body()), PrintParts::from_str("mh")?);
        assert!(target.saved_body.is_none());
        Ok(())
    }

    #[test]
    fn accepts_y_and_yes_in_any_case() {
        assert!(["y\n", "Y", " yes ", "YES\r\n"]
            .iter()
            .all(|answer| is_yes(answer)));
    }

    #[test]
    fn treats_anything_else_as_no() {
        assert!(!["", "\n", "n", "no", "yep", "sí"]
            .iter()
            .any(|answer| is_yes(answer)));
    }
}
