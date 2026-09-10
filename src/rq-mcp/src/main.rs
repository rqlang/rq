use anyhow::Result;
use rmcp::{
    handler::server::{
        router::{prompt::PromptRouter, tool::ToolRouter},
        wrapper::Parameters,
    },
    model::{
        AnnotateAble, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourcesResult, PaginatedRequestParams,
        PromptMessage, PromptMessageRole, ProtocolVersion, RawResource, ReadResourceRequestParams,
        ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
    },
    prompt, prompt_handler, prompt_router, schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use rq_lib::client::models::RequestInfo;
use rq_lib::error::RqError;
use rq_lib::RqClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

const LANGUAGE_DEFINITION_URI: &str = "rqlang://docs/language-definition";
const LANGUAGE_DEFINITION_MD: &str = include_str!("../../../docs/LANGUAGE_DEFINITION.md");

const IDIOMS_URI: &str = "rqlang://docs/idioms";
const IDIOMS_MD: &str = include_str!("../../../docs/RQLANG_IDIOMS.md");

const DRAFT_FILE_NAME: &str = "draft.rq";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ValidateRqParams {
    source: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    workspace_path: Option<String>,
    #[serde(default)]
    env: Option<String>,
}

#[derive(Debug, Serialize)]
struct ValidateDiagnostic {
    severity: &'static str,
    message: String,
    line: usize,
    column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

#[derive(Debug, Serialize)]
struct ValidateResult {
    ok: bool,
    diagnostics: Vec<ValidateDiagnostic>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetRqReferenceParams {
    doc: ReferenceDoc,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ReferenceDoc {
    LanguageDefinition,
    Idioms,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListRequestsParams {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LintRqParams {
    source: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    workspace_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListRequestsResult {
    requests: Vec<RequestInfo>,
    parse_errors: Vec<ValidateDiagnostic>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct GenerateRqArgs {
    intent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace_path: Option<String>,
}

#[derive(Clone)]
struct RqMcp {
    #[allow(dead_code)]
    tool_router: ToolRouter<RqMcp>,
    #[allow(dead_code)]
    prompt_router: PromptRouter<RqMcp>,
}

#[tool_router]
impl RqMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    #[tool(
        description = "Validate rqlang source against the real parser and analyzer. Returns { ok, diagnostics[] } where each diagnostic has severity, message, line, column, and optional file. `path` is the logical filename used in diagnostics. `workspace_path` is the directory the draft is checked against — pass it whenever the draft has `import` statements or relies on `.env` secrets, otherwise those imports cannot be resolved and will be reported as errors. When `workspace_path` is omitted the directory is taken from `path`, falling back to the current working directory. The draft is never written to disk. `env` selects an environment for variable resolution."
    )]
    fn validate_rq(
        &self,
        Parameters(ValidateRqParams {
            source,
            path,
            workspace_path,
            env,
        }): Parameters<ValidateRqParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = validate_source(
            &source,
            path.as_deref(),
            workspace_path.as_deref(),
            env.as_deref(),
        )
        .map_err(|m| McpError::internal_error(m, None))?;
        let json = serde_json::to_string(&result)
            .map_err(|e| McpError::internal_error(format!("serialize failed: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Run idiom and style lint rules against rqlang source. Returns { ok, diagnostics[] } where each diagnostic carries severity (always `error`), rule id, message, line, column, optional file, and optional suggested_fix. When `workspace_path` is provided, the lint also sees requests from other .rq files under that path and can flag cross-file conflicts (e.g. a draft request that should be merged with an existing endpoint elsewhere). `path` is the logical filename of the draft and tells the workspace walker which file to skip. Call after validate_rq (syntax must already be ok) and iterate: fix the diagnostics, re-validate, then re-lint until ok: true. Pure check, no side effects."
    )]
    fn lint_rq(
        &self,
        Parameters(LintRqParams {
            source,
            path,
            workspace_path,
        }): Parameters<LintRqParams>,
    ) -> Result<CallToolResult, McpError> {
        let workspace = workspace_path.as_deref().map(Path::new);
        let result = rq_lib::lint::lint(&source, path.as_deref(), workspace);
        let json = serde_json::to_string(&result)
            .map_err(|e| McpError::internal_error(format!("serialize failed: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Return the full text of one rqlang documentation file: `language-definition` (the complete grammar reference — statement forms, interpolation, attributes, built-in functions) or `idioms` (style preferences and canonical examples). The same documents are also published as MCP resources, but this tool works on clients that do not let the model read resources. Call it for both documents before generating or refactoring any .rq file — never guess rqlang syntax or probe for it with repeated validate_rq calls."
    )]
    fn get_rq_reference(
        &self,
        Parameters(GetRqReferenceParams { doc }): Parameters<GetRqReferenceParams>,
    ) -> Result<CallToolResult, McpError> {
        let body = match doc {
            ReferenceDoc::LanguageDefinition => LANGUAGE_DEFINITION_MD,
            ReferenceDoc::Idioms => IDIOMS_MD,
        };
        Ok(CallToolResult::success(vec![Content::text(body)]))
    }

    #[tool(
        description = "List every named rqlang request reachable from `path`. `path` is optional: omit it to use the server's current working directory (typically the active workspace folder). Pass an explicit file or directory only when the user wants to inspect somewhere else. Returns { requests: [{name, endpoint, file, endpoint_file, endpoint_line, endpoint_character}], parse_errors: [diagnostic] }. Use this before generating a new request to avoid name collisions."
    )]
    fn list_requests(
        &self,
        Parameters(ListRequestsParams { path }): Parameters<ListRequestsParams>,
    ) -> Result<CallToolResult, McpError> {
        match list_requests_at(path.as_deref()) {
            Ok(result) => {
                let json = serde_json::to_string(&result).map_err(|e| {
                    McpError::internal_error(format!("serialize failed: {e}"), None)
                })?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(ListRequestsError::User(msg)) => {
                Ok(CallToolResult::error(vec![Content::text(msg)]))
            }
            Err(ListRequestsError::Internal(msg)) => Err(McpError::internal_error(msg, None)),
        }
    }
}

#[prompt_router]
impl RqMcp {
    #[prompt(name = "generate_rq")]
    async fn generate_rq(
        &self,
        Parameters(args): Parameters<GenerateRqArgs>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let body = build_generate_rq_prompt(&args);
        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, body)])
                .with_description(
                    "Author a validated rqlang (.rq) snippet for the described intent.",
                ),
        )
    }
}

fn build_generate_rq_prompt(args: &GenerateRqArgs) -> String {
    let workspace = args.workspace_path.as_deref();
    let steps = [
        read_resources_step(),
        list_requests_step(workspace),
        file_layout_step(),
        draft_step(),
        validate_step(workspace),
        lint_step(workspace),
        present_step(),
    ];
    let workflow: String = steps
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {s}\n", i + 1))
        .collect();

    let constraints = generate_rq_constraints();

    format!(
        "You are helping the user author rqlang (.rq) files for the following intent:\n\n\
         > {intent}\n\n\
         Workflow:\n{workflow}\n{constraints}",
        intent = args.intent,
    )
}

fn read_resources_step() -> String {
    format!(
        "Read both rqlang documents before writing anything: the grammar reference and the \
         style guide. Call `get_rq_reference` with `doc=\"language-definition\"` and then with \
         `doc=\"idioms\"` — those are the same documents published as the \
         `{LANGUAGE_DEFINITION_URI}` and `{IDIOMS_URI}` resources, so read the resources instead \
         if your client exposes them to you."
    )
}

fn list_requests_step(workspace_path: Option<&str>) -> String {
    match workspace_path {
        Some(path) => format!(
            "Call the `list_requests` tool with path=\"{path}\" so any names you \
             generate don't collide with existing requests."
        ),
        None => "Call the `list_requests` tool (omit `path` — it defaults to the current \
             workspace) so any names you generate don't collide with existing requests."
            .into(),
    }
}

fn file_layout_step() -> String {
    "Decide the file layout BEFORE drafting. Each `ep` goes in its own file named after the \
     endpoint — `ep users` in `users.rq`, `ep widgets` in `widgets.rq` — so a request for \
     several endpoints produces several files, never one file with several `ep` blocks. Any \
     `env`, `auth` or `let` definitions that more than one file needs go in a shared file \
     (e.g. `shared.rq`) that the others `import` with a relative path."
        .into()
}

fn draft_step() -> String {
    "Draft the contents of every file in that layout.".into()
}

fn validate_step(workspace_path: Option<&str>) -> String {
    let workspace_clause = match workspace_path {
        Some(path) => format!("`workspace_path=\"{path}\"`"),
        None => "`workspace_path` set to the current working directory".into(),
    };
    format!(
        "Validate each file separately: call `validate_rq` once per file with that file's \
         content as `source`, that file's name as `path`, and {workspace_clause} so `import` \
         statements and `.env` secrets resolve against the real workspace. Fix diagnostics and \
         re-validate until every file returns `ok: true`."
    )
}

fn lint_step(workspace_path: Option<&str>) -> String {
    let workspace_clause = match workspace_path {
        Some(path) => format!("`workspace_path=\"{path}\"`"),
        None => "`workspace_path` set to the current working directory".into(),
    };
    format!(
        "Lint each file separately: call `lint_rq` once per file with the same `source` and \
         `path`, plus {workspace_clause} so the lint sees the other .rq files and can flag \
         cross-file conflicts. Apply the `suggested_fix` for each diagnostic (or rewrite to \
         satisfy the `rule`), then re-validate and re-lint. Some rules are satisfied by MOVING \
         code into another file rather than editing the current one — `multiple_endpoints_per_file` \
         is cleared by splitting the file into one file per `ep`, not by deleting an endpoint. \
         Never drop content the user asked for in order to silence a diagnostic. Iterate until \
         every file returns `ok: true`."
    )
}

fn present_step() -> String {
    "Present each file in its own markdown code block with its filename on the line directly \
     above it; briefly explain any non-obvious choices."
        .into()
}

fn generate_rq_constraints() -> String {
    format!(
        "\
Constraints:
- Use only constructs documented in `{LANGUAGE_DEFINITION_URI}`. Do not invent syntax.
- Identifiers (request names, variable names, env names) use snake_case.
- Variable references use `{{{{name}}}}`. The `${{...}}` prefix is for JSON literal bodies (e.g. `body: ${{\"name\": \"alice\"}}`) — it is NOT variable interpolation, and a JSON body must never be written as a quoted string like `body: \"{{}}\"`.
- Never put two `ep` blocks in the same file.
- Do not run the request yourself.
- Do not suggest CLI commands (e.g. `rq request run …`) for executing the generated snippet. Only mention how to run it if the user explicitly asks."
    )
}

#[derive(Debug)]
enum ListRequestsError {
    User(String),
    Internal(String),
}

fn list_requests_at(path: Option<&str>) -> Result<ListRequestsResult, ListRequestsError> {
    let resolved = resolve_workspace_path(path)?;
    let (requests, parse_errors) = RqClient::default()
        .list_requests(Path::new(&resolved))
        .map_err(|e| {
            if is_user_error(&e) {
                ListRequestsError::User(e.to_string())
            } else {
                ListRequestsError::Internal(e.to_string())
            }
        })?;
    let parse_errors = parse_errors
        .into_iter()
        .map(|e| map_diagnostic(e, &resolved, Path::new(&resolved)))
        .collect();
    Ok(ListRequestsResult {
        requests,
        parse_errors,
    })
}

fn resolve_workspace_path(provided: Option<&str>) -> Result<String, ListRequestsError> {
    match provided {
        Some(p) => Ok(p.to_string()),
        None => std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| {
                ListRequestsError::User(format!(
                    "no path provided and current directory is unavailable: {e}"
                ))
            }),
    }
}

fn is_user_error(error: &RqError) -> bool {
    matches!(
        error,
        RqError::DirectoryNotFound(_)
            | RqError::NotADirectory(_)
            | RqError::RequestNotFound(_)
            | RqError::EnvironmentNotFound(_)
    )
}

fn validate_source(
    source: &str,
    path: Option<&str>,
    workspace_path: Option<&str>,
    env: Option<&str>,
) -> Result<ValidateResult, String> {
    let display_path = path.unwrap_or("<inline>");
    let draft_path = resolve_draft_path(path, workspace_path)?;
    let errors = RqClient::default()
        .check_source(source, &draft_path, env)
        .map_err(|e| format!("check failed: {e}"))?;
    let diagnostics: Vec<ValidateDiagnostic> = errors
        .into_iter()
        .map(|e| map_diagnostic(e, display_path, &draft_path))
        .collect();
    Ok(ValidateResult {
        ok: diagnostics.is_empty(),
        diagnostics,
    })
}

fn resolve_draft_path(path: Option<&str>, workspace_path: Option<&str>) -> Result<PathBuf, String> {
    let logical = path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DRAFT_FILE_NAME));
    let resolved = rq_lib::paths::resolve_under_workspace(&logical, workspace_path.map(Path::new));
    if resolved.is_absolute() {
        return Ok(resolved);
    }
    let cwd =
        std::env::current_dir().map_err(|e| format!("current directory is unavailable: {e}"))?;
    Ok(cwd.join(resolved))
}

fn reported_file(file_path: Option<String>, display_path: &str, draft_path: &Path) -> String {
    match file_path {
        Some(path) if Path::new(&path) != draft_path => path,
        _ => display_path.to_string(),
    }
}

fn map_diagnostic(error: RqError, display_path: &str, draft_path: &Path) -> ValidateDiagnostic {
    match error {
        RqError::Syntax(se) => ValidateDiagnostic {
            severity: "error",
            message: se.message,
            line: se.line,
            column: se.column,
            file: Some(reported_file(se.file_path, display_path, draft_path)),
        },
        other => ValidateDiagnostic {
            severity: "error",
            message: other.to_string(),
            line: 0,
            column: 0,
            file: Some(display_path.to_string()),
        },
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for RqMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(format!(
            "rq-mcp v1. Tools: validate_rq (parse + analyze, returns syntax/semantic \
             diagnostics), lint_rq (style/idiom rules, returns rule-tagged diagnostics with \
             suggested fixes), list_requests (enumerate named requests under a path so \
             generated ones avoid name collisions), get_rq_reference (return the full text of \
             the rqlang grammar reference or the idioms guide). After drafting any .rq snippet, always \
             call validate_rq first; once it returns ok:true, call lint_rq and iterate \
             until that also returns ok:true. Output may span several files: keep one `ep` \
             per file named after the endpoint, and call validate_rq and lint_rq once per \
             file with that file's name as `path`. Some lint rules are cleared by moving \
             code into another file rather than editing the current one — never delete \
             content the user asked for just to silence a diagnostic. \
             Before generating or refactoring any .rq file, read both rqlang documents with \
             get_rq_reference (`doc=\"language-definition\"` for the grammar, `doc=\"idioms\"` \
             for style); they are also published as the {LANGUAGE_DEFINITION_URI} and \
             {IDIOMS_URI} resources for clients that expose resource reads. Never infer rqlang \
             syntax from repeated validate_rq attempts or by searching the filesystem for \
             documentation. Prompts: \
             generate_rq (drives the full generate → validate → lint → iterate loop). \
             Behavior: this server is for authoring .rq files, not running them. Do not \
             propose or suggest CLI commands (e.g. `rq request run …`) for executing \
             generated snippets unless the user explicitly asks how to run them."
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new(LANGUAGE_DEFINITION_URI, "language-definition")
                    .with_title("rqlang Language Definition")
                    .with_description(
                        "Full reference for the rqlang DSL: statement forms (rq, ep, env, \
                     auth, let, import), variable interpolation, attributes, and built-in \
                     functions. Read this before generating any .rq snippet.",
                    )
                    .with_mime_type("text/markdown")
                    .with_size(LANGUAGE_DEFINITION_MD.len() as u32)
                    .no_annotation(),
                RawResource::new(IDIOMS_URI, "idioms")
                    .with_title("rqlang Idioms & Style Guide")
                    .with_description(
                        "Opinionated style preferences and canonical examples for generating \
                     .rq files (when to introduce an ep, verb-only naming, JSON body \
                     syntax, multi-file split with import, etc.). Read alongside the \
                     language definition before drafting or refactoring any .rq snippet.",
                    )
                    .with_mime_type("text/markdown")
                    .with_size(IDIOMS_MD.len() as u32)
                    .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match request.uri.as_str() {
            LANGUAGE_DEFINITION_URI => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                LANGUAGE_DEFINITION_MD,
                request.uri,
            )
            .with_mime_type("text/markdown")])),
            IDIOMS_URI => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                IDIOMS_MD,
                request.uri,
            )
            .with_mime_type("text/markdown")])),
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    std::panic::set_hook(Box::new(|info| {
        eprintln!("rq-mcp PANIC: {info}");
        if let Some(loc) = info.location() {
            eprintln!("  at {}:{}:{}", loc.file(), loc.line(), loc.column());
        }
    }));

    tracing::info!("rq-mcp starting on stdio");

    let service = RqMcp::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serve error: {e:?}");
    })?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_source_returns_ok_for_clean_request() {
        let target = validate_source(
            "rq basic(\"http://localhost:8080/get\");\n",
            None,
            None,
            None,
        )
        .expect("validate_source failed");
        assert!(target.ok);
        assert!(target.diagnostics.is_empty());
    }

    #[test]
    fn validate_source_reports_syntax_error_with_line_and_column() {
        let target = validate_source(
            "rq basic(\"http://localhost:8080/get\"\n",
            Some("draft.rq"),
            None,
            None,
        )
        .expect("validate_source failed");
        assert!(!target.ok);
        assert_eq!(target.diagnostics.len(), 1);
        let diag = &target.diagnostics[0];
        assert_eq!(diag.severity, "error");
        assert!(diag.line >= 1);
        assert!(diag.column >= 1);
        assert_eq!(diag.file.as_deref(), Some("draft.rq"));
        assert!(!diag.message.is_empty());
    }

    #[test]
    fn validate_source_blames_the_imported_file_that_actually_has_the_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shared.rq"), "let broken = ;\n").expect("write");
        let target = validate_source(
            "import \"shared\";\n\nrq list(\"http://localhost:8080/users\");\n",
            Some("users.rq"),
            dir.path().to_str(),
            None,
        )
        .expect("validate_source failed");
        assert!(!target.ok, "expected a diagnostic");
        let blamed = target.diagnostics[0].file.as_deref().expect("file");
        assert!(
            blamed.ends_with("shared.rq"),
            "the broken sibling should be blamed, got {blamed}"
        );
    }

    #[test]
    fn list_requests_at_blames_the_broken_file_not_the_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("good.rq"),
            "rq greet(\"http://example.test/hi\");\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("broken.rq"), "rq oops(\"http://x\"\n").expect("write");
        let target = list_requests_at(dir.path().to_str()).expect("list failed");
        assert_eq!(
            target.parse_errors.len(),
            1,
            "got: {:?}",
            target.parse_errors
        );
        let blamed = target.parse_errors[0].file.as_deref().expect("file");
        assert!(
            blamed.ends_with("broken.rq"),
            "the broken file should be blamed, got {blamed}"
        );
    }

    #[test]
    fn validate_source_defaults_file_to_inline_when_no_path() {
        let target = validate_source("rq basic(\"http://x\"\n", None, None, None)
            .expect("validate_source failed");
        assert!(!target.ok);
        assert_eq!(target.diagnostics[0].file.as_deref(), Some("<inline>"));
    }

    #[test]
    fn validate_source_accepts_empty_input() {
        let target = validate_source("", None, None, None).expect("validate_source failed");
        assert!(target.ok);
        assert!(target.diagnostics.is_empty());
    }

    #[test]
    fn validate_source_resolves_imports_against_workspace_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("shared.rq"),
            "env local {\n    base_url: \"http://localhost:8080\",\n}\n",
        )
        .expect("write shared");
        let target = validate_source(
            "import \"shared\";\n\nrq list(\"{{base_url}}/users\");\n",
            Some("users.rq"),
            dir.path().to_str(),
            Some("local"),
        )
        .expect("validate_source failed");
        assert!(target.ok, "expected ok, got {:?}", target.diagnostics);
    }

    #[test]
    fn validate_source_reports_import_missing_from_workspace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = validate_source(
            "import \"shared\";\n\nrq list(\"http://localhost:8080/users\");\n",
            Some("users.rq"),
            dir.path().to_str(),
            None,
        )
        .expect("validate_source failed");
        assert!(!target.ok);
        assert!(target.diagnostics[0].message.contains("shared"));
    }

    #[test]
    fn validate_source_does_not_write_draft_into_workspace() {
        let dir = tempfile::tempdir().expect("tempdir");
        validate_source(
            "rq basic(\"http://localhost:8080/get\");\n",
            Some("users.rq"),
            dir.path().to_str(),
            None,
        )
        .expect("validate_source failed");
        assert!(!dir.path().join("users.rq").exists());
        assert_eq!(
            std::fs::read_dir(dir.path()).expect("read_dir").count(),
            0,
            "validation must not create files in the workspace"
        );
    }

    #[test]
    fn validate_source_resolves_an_import_next_to_a_relative_draft_in_a_subdirectory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("api")).expect("mkdir");
        std::fs::write(
            dir.path().join("api/shared.rq"),
            "ep base(url: \"http://localhost:8080\");\n",
        )
        .expect("write");
        let target = validate_source(
            "import \"shared\";\n\nep users<base>(\"/users\") {\n    rq list();\n}\n",
            Some("api/users.rq"),
            dir.path().to_str(),
            None,
        )
        .expect("validate_source failed");
        assert!(target.ok, "expected ok, got {:?}", target.diagnostics);
    }

    #[test]
    fn resolve_draft_path_preserves_the_subdirectory_of_a_relative_path() {
        let target =
            resolve_draft_path(Some("api/users.rq"), Some("/workspace")).expect("resolve failed");
        assert_eq!(target, PathBuf::from("/workspace/api/users.rq"));
    }

    #[test]
    fn resolve_draft_path_keeps_an_absolute_path_already_inside_the_workspace() {
        let target = resolve_draft_path(Some("/workspace/api/users.rq"), Some("/workspace"))
            .expect("resolve failed");
        assert_eq!(target, PathBuf::from("/workspace/api/users.rq"));
    }

    #[test]
    fn resolve_draft_path_prefers_workspace_path_over_path_parent() {
        let target = resolve_draft_path(Some("/elsewhere/users.rq"), Some("/workspace"))
            .expect("resolve failed");
        assert_eq!(target, PathBuf::from("/workspace/users.rq"));
    }

    #[test]
    fn resolve_draft_path_falls_back_to_path_parent() {
        let target =
            resolve_draft_path(Some("/workspace/api/users.rq"), None).expect("resolve failed");
        assert_eq!(target, PathBuf::from("/workspace/api/users.rq"));
    }

    #[test]
    fn resolve_draft_path_defaults_to_cwd_and_draft_file_name() {
        let expected = std::env::current_dir().expect("cwd").join(DRAFT_FILE_NAME);
        let target = resolve_draft_path(None, None).expect("resolve failed");
        assert_eq!(target, expected);
    }

    #[test]
    fn resolve_draft_path_uses_workspace_file_parent_when_given_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace_file = dir.path().join("existing.rq");
        std::fs::write(&workspace_file, "").expect("write");
        let target =
            resolve_draft_path(Some("users.rq"), workspace_file.to_str()).expect("resolve failed");
        assert_eq!(target, dir.path().join("users.rq"));
    }

    #[test]
    fn list_requests_at_single_file_returns_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("greet.rq");
        std::fs::write(&file_path, "rq greet(\"http://example.test/hi\");\n")
            .expect("write fixture");
        let target = list_requests_at(file_path.to_str()).expect("list failed");
        assert!(target.parse_errors.is_empty());
        assert_eq!(target.requests.len(), 1);
        assert_eq!(target.requests[0].name, "greet");
        assert!(target.requests[0].endpoint.is_none());
    }

    #[test]
    fn list_requests_at_directory_returns_all_requests() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("a.rq"),
            "rq alpha(\"http://example.test/a\");\n",
        )
        .expect("write a");
        std::fs::write(
            dir.path().join("b.rq"),
            "rq bravo(\"http://example.test/b\");\n",
        )
        .expect("write b");
        let target = list_requests_at(dir.path().to_str()).expect("list failed");
        assert!(target.parse_errors.is_empty());
        assert_eq!(target.requests.len(), 2);
        let mut names: Vec<&str> = target.requests.iter().map(|r| r.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "bravo"]);
    }

    #[test]
    fn list_requests_at_missing_path_is_user_error() {
        let err = list_requests_at(Some("/tmp/does-not-exist-rq-mcp")).expect_err("should fail");
        assert!(matches!(err, ListRequestsError::User(_)));
    }

    #[test]
    fn resolve_workspace_path_returns_provided_when_present() {
        let target = resolve_workspace_path(Some("/some/path")).expect("ok");
        assert_eq!(target, "/some/path");
    }

    #[test]
    fn resolve_workspace_path_defaults_to_current_dir() {
        let expected = std::env::current_dir()
            .expect("cwd")
            .to_string_lossy()
            .to_string();
        let target = resolve_workspace_path(None).expect("ok");
        assert_eq!(target, expected);
    }

    #[test]
    fn list_requests_at_none_uses_current_directory() {
        let result = list_requests_at(None);
        assert!(
            result.is_ok(),
            "should resolve to CWD without error in test harness"
        );
    }

    #[test]
    fn generate_rq_prompt_includes_intent_and_pipeline_references() {
        let args = GenerateRqArgs {
            intent: "create-user POST".into(),
            workspace_path: None,
        };
        let body = build_generate_rq_prompt(&args);
        assert!(body.contains("create-user POST"), "missing intent");
        assert!(
            body.contains(LANGUAGE_DEFINITION_URI),
            "missing grammar reference"
        );
        assert!(body.contains("validate_rq"), "missing validate_rq mention");
        assert!(
            body.contains("{{name}}"),
            "missing literal {{name}} interpolation syntax"
        );
        assert!(
            body.contains("${...}"),
            "missing JSON literal prefix reference"
        );
    }

    #[test]
    fn idioms_resource_uri_is_namespaced() {
        assert_eq!(IDIOMS_URI, "rqlang://docs/idioms");
    }

    #[test]
    fn resource_uris_carry_a_path_segment() {
        for uri in [LANGUAGE_DEFINITION_URI, IDIOMS_URI] {
            let authority_and_path = uri
                .strip_prefix("rqlang://")
                .unwrap_or_else(|| panic!("{uri} must use the rqlang:// scheme"));
            let (authority, path) = authority_and_path.split_once('/').unwrap_or_else(|| {
                panic!(
                    "{uri} puts the resource name in the authority slot; \
                     clients normalize or reject an empty path"
                )
            });
            assert!(!authority.is_empty(), "{uri} has an empty authority");
            assert!(!path.is_empty(), "{uri} has an empty path");
        }
    }

    #[test]
    fn idioms_resource_is_embedded_and_substantive() {
        assert!(
            IDIOMS_MD.len() > 1000,
            "expected substantive markdown, got {} bytes",
            IDIOMS_MD.len()
        );
    }

    #[test]
    fn idioms_resource_covers_each_lint_rule_it_backs() {
        for needle in [
            "[required(",
            "Only introduce an `ep` block",
            "Always use relative import paths",
            "name requests after the verb alone",
            "For write actions, include a body",
            "JSON body syntax: always use the `${...}` prefix",
            "ep users<users_base>",
            "Never hand-write an `Authorization` header",
            "Never put a credential literal in a `.rq` file",
            "goes on the `ep`, not on each `rq`",
            "extending the same template declare identically belongs on the template",
            "Never re-declare on a child what the template already gives it",
        ] {
            assert!(
                IDIOMS_MD.contains(needle),
                "idioms doc missing guidance for: {needle}"
            );
        }
    }

    #[test]
    fn generate_rq_prompt_references_idioms_resource() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "x".into(),
            workspace_path: None,
        });
        assert!(
            body.contains(IDIOMS_URI),
            "prompt should tell the AI to read {IDIOMS_URI}"
        );
        assert!(
            body.contains(LANGUAGE_DEFINITION_URI),
            "prompt should still tell the AI to read {LANGUAGE_DEFINITION_URI}"
        );
    }

    fn reference_text(doc: ReferenceDoc) -> String {
        let result = RqMcp::new()
            .get_rq_reference(Parameters(GetRqReferenceParams { doc }))
            .expect("get_rq_reference failed");
        result
            .content
            .into_iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect()
    }

    #[test]
    fn get_rq_reference_returns_the_language_definition() {
        let target = reference_text(ReferenceDoc::LanguageDefinition);
        assert_eq!(target, LANGUAGE_DEFINITION_MD);
    }

    #[test]
    fn get_rq_reference_returns_the_idioms_guide() {
        let target = reference_text(ReferenceDoc::Idioms);
        assert_eq!(target, IDIOMS_MD);
    }

    #[test]
    fn get_rq_reference_serves_the_same_bodies_as_the_resources() {
        assert!(reference_text(ReferenceDoc::LanguageDefinition).contains("rq "));
        assert!(reference_text(ReferenceDoc::Idioms)
            .contains("Never hand-write an `Authorization` header"));
    }

    #[test]
    fn server_instructions_point_at_the_reference_tool() {
        let instructions = RqMcp::new().get_info().instructions.expect("instructions");
        assert!(
            instructions.contains("get_rq_reference"),
            "clients that cannot read resources need the tool named in the instructions"
        );
    }

    #[test]
    fn generate_rq_prompt_points_at_the_reference_tool() {
        let args = GenerateRqArgs {
            intent: "list users".into(),
            workspace_path: None,
        };
        let body = build_generate_rq_prompt(&args);
        assert!(body.contains("get_rq_reference"), "got: {body}");
    }

    #[test]
    fn server_instructions_reference_both_resources() {
        let info = RqMcp::new().get_info();
        let instructions = info.instructions.expect("instructions present");
        assert!(
            instructions.contains(IDIOMS_URI),
            "instructions should advertise {IDIOMS_URI}"
        );
        assert!(
            instructions.contains(LANGUAGE_DEFINITION_URI),
            "instructions should advertise {LANGUAGE_DEFINITION_URI}"
        );
    }

    #[test]
    fn generate_rq_prompt_forbids_unsolicited_cli_suggestions() {
        let args = GenerateRqArgs {
            intent: "anything".into(),
            workspace_path: None,
        };
        let body = build_generate_rq_prompt(&args);
        assert!(
            body.contains("Do not suggest CLI commands"),
            "missing constraint against unsolicited CLI suggestions"
        );
        assert!(
            body.contains("explicitly asks"),
            "constraint should clarify it kicks in only without explicit ask"
        );
    }

    #[test]
    fn generate_rq_prompt_uses_cwd_default_step_when_path_absent() {
        let args = GenerateRqArgs {
            intent: "anything".into(),
            workspace_path: None,
        };
        let body = build_generate_rq_prompt(&args);
        assert!(body.contains("list_requests"), "list_requests step missing");
        assert!(
            body.contains("omit `path`") || body.contains("defaults to the current workspace"),
            "expected wording that tells the AI to omit path to use CWD"
        );
    }

    #[test]
    fn generate_rq_prompt_includes_workspace_step_when_path_present() {
        let args = GenerateRqArgs {
            intent: "anything".into(),
            workspace_path: Some("/some/path".into()),
        };
        let body = build_generate_rq_prompt(&args);
        assert!(body.contains("list_requests"), "workspace step missing");
        assert!(body.contains("/some/path"), "workspace path missing");
    }

    #[test]
    fn generate_rq_prompt_requires_one_endpoint_per_file() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "users and widgets CRUD".into(),
            workspace_path: None,
        });
        assert!(
            body.contains("Each `ep` goes in its own file"),
            "prompt must state the one-ep-per-file layout"
        );
        assert!(
            body.contains("Never put two `ep` blocks in the same file"),
            "prompt must carry the constraint"
        );
    }

    #[test]
    fn generate_rq_prompt_drives_the_loop_per_file() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "x".into(),
            workspace_path: None,
        });
        assert!(
            body.contains("Validate each file separately"),
            "validate step must be per-file"
        );
        assert!(
            body.contains("Lint each file separately"),
            "lint step must be per-file"
        );
        assert!(
            body.contains("every file returns `ok: true`"),
            "termination must be defined over every file, not one snippet"
        );
    }

    #[test]
    fn generate_rq_prompt_explains_that_splitting_clears_the_endpoint_rule() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "x".into(),
            workspace_path: None,
        });
        assert!(
            body.contains("multiple_endpoints_per_file"),
            "prompt must name the rule that is cleared by splitting"
        );
        assert!(
            body.contains("Never drop content the user asked for"),
            "prompt must forbid deleting an endpoint to silence the rule"
        );
    }

    #[test]
    fn generate_rq_prompt_asks_for_one_code_block_per_file() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "x".into(),
            workspace_path: None,
        });
        assert!(
            body.contains("own markdown code block with its filename"),
            "presentation step must be per-file and name each file"
        );
    }

    #[test]
    fn server_instructions_describe_multi_file_output() {
        let info = RqMcp::new().get_info();
        let instructions = info.instructions.expect("instructions present");
        assert!(
            instructions.contains("one `ep` per file"),
            "instructions must state the layout for hosts that skip the prompt"
        );
        assert!(
            instructions.contains("once per file"),
            "instructions must tell the host to lint each file separately"
        );
    }

    #[test]
    fn generate_rq_prompt_passes_workspace_path_to_validate_step() {
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "anything".into(),
            workspace_path: Some("/some/path".into()),
        });
        let validate_step = body
            .lines()
            .find(|l| l.contains("validate_rq"))
            .expect("validate step present");
        assert!(
            validate_step.contains("workspace_path"),
            "validate step must pass workspace_path so imports resolve: {validate_step}"
        );
    }

    #[test]
    fn language_definition_resource_is_embedded_and_substantive() {
        assert_eq!(LANGUAGE_DEFINITION_URI, "rqlang://docs/language-definition");
        assert!(
            LANGUAGE_DEFINITION_MD.len() > 1000,
            "expected substantive markdown, got {} bytes",
            LANGUAGE_DEFINITION_MD.len()
        );
    }

    #[test]
    fn language_definition_covers_core_constructs() {
        for needle in ["rq", "ep", "env", "auth", "let", "import"] {
            assert!(
                LANGUAGE_DEFINITION_MD.contains(needle),
                "language definition is missing reference to `{needle}`"
            );
        }
    }

    #[test]
    fn list_requests_at_directory_with_broken_file_reports_parse_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("good.rq"),
            "rq good(\"http://example.test/g\");\n",
        )
        .expect("write good");
        std::fs::write(dir.path().join("broken.rq"), "rq broken(\"http://x\"\n")
            .expect("write broken");
        let target = list_requests_at(dir.path().to_str()).expect("list failed");
        assert_eq!(target.requests.len(), 1);
        assert_eq!(target.requests[0].name, "good");
        assert!(!target.parse_errors.is_empty());
    }
}

#[cfg(test)]
mod surface_tests {
    use super::*;

    const SURFACE_JSON: &str = include_str!("../surface.json");

    fn surface() -> serde_json::Value {
        serde_json::from_str(SURFACE_JSON).expect("surface.json parses")
    }

    fn render(template: &str) -> String {
        template
            .replace("{language_definition_uri}", LANGUAGE_DEFINITION_URI)
            .replace("{idioms_uri}", IDIOMS_URI)
    }

    #[test]
    fn surface_json_matches_every_tool_description() {
        let surface = surface();
        let router = RqMcp::tool_router();
        for tool in router.list_all() {
            let expected = surface["tools"][tool.name.as_ref()]["description"]
                .as_str()
                .unwrap_or_else(|| panic!("surface.json has no description for `{}`", tool.name));
            let actual = tool.description.as_deref().unwrap_or_default();
            assert_eq!(
                actual, expected,
                "`{}` description drifted from surface.json — update both, they are one surface \
                 served by two implementations",
                tool.name
            );
        }
    }

    #[test]
    fn surface_json_lists_exactly_the_tools_the_server_serves() {
        let surface = surface();
        let mut served: Vec<String> = RqMcp::tool_router()
            .list_all()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        let mut declared: Vec<String> = surface["tools"]
            .as_object()
            .expect("tools object")
            .keys()
            .cloned()
            .collect();
        served.sort();
        declared.sort();
        assert_eq!(served, declared);
    }

    #[test]
    fn surface_json_matches_the_server_instructions() {
        let surface = surface();
        let served = RqMcp::new().get_info().instructions.expect("instructions");
        assert_eq!(
            served,
            render(surface["instructions"].as_str().expect("instructions"))
        );
    }

    #[test]
    fn surface_json_matches_the_generate_rq_prompt() {
        let surface = surface();
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "list users".into(),
            workspace_path: None,
        });
        let steps = &surface["prompt"]["steps"];
        let clause = surface["prompt"]["workspace_clause_without"]
            .as_str()
            .expect("clause");
        for key in [
            "read_resources",
            "list_requests_default",
            "file_layout",
            "draft",
            "validate",
            "lint",
            "present",
        ] {
            let step =
                render(steps[key].as_str().expect(key)).replace("{workspace_clause}", clause);
            assert!(
                body.contains(&step),
                "generate_rq prompt drifted from surface.json step `{key}`"
            );
        }
        assert!(body.contains(&render(
            surface["prompt"]["constraints"]
                .as_str()
                .expect("constraints")
        )));
    }

    #[test]
    fn surface_json_matches_the_workspace_variant_of_the_prompt() {
        let surface = surface();
        let body = build_generate_rq_prompt(&GenerateRqArgs {
            intent: "list users".into(),
            workspace_path: Some("/repo".into()),
        });
        let clause = surface["prompt"]["workspace_clause_with"]
            .as_str()
            .expect("clause")
            .replace("{path}", "/repo");
        for key in ["list_requests_with_workspace", "validate", "lint"] {
            let step = render(surface["prompt"]["steps"][key].as_str().expect(key))
                .replace("{workspace_clause}", &clause)
                .replace("{path}", "/repo");
            assert!(
                body.contains(&step),
                "generate_rq prompt drifted from surface.json step `{key}`"
            );
        }
    }

    #[test]
    fn surface_json_matches_the_resource_definitions() {
        let surface = surface();
        for (name, uri) in [
            ("language-definition", LANGUAGE_DEFINITION_URI),
            ("idioms", IDIOMS_URI),
        ] {
            assert_eq!(surface["resources"][name]["uri"], uri);
        }
    }
}
