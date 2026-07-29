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
use std::path::Path;
use tracing_subscriber::EnvFilter;

const LANGUAGE_DEFINITION_URI: &str = "rqlang://language-definition";
const LANGUAGE_DEFINITION_MD: &str = include_str!("../../../docs/LANGUAGE_DEFINITION.md");

const IDIOMS_URI: &str = "rqlang://idioms";
const IDIOMS: &str = "\
Style preferences (apply unless the user asks otherwise):
- Prefer the `[required(name)]` attribute over declaring a `let name = …` upfront when the value is supplied at runtime.
- Only introduce an `ep` block when two or more requests share a base URL, auth, or headers. A single standalone request should be a top-level `rq` with the full URL — do not wrap a lone request in an `ep`. **When adding a new request, first call `list_requests` to see what exists; if there is already a request for the same entity (same noun in the URL path), refactor those siblings into a shared `ep` together with the new request rather than appending another top-level `rq`.**
- Put the base URL directly on the `ep`. Avoid splitting it into a base `ep` extended via `ep child<base>(…)` unless you genuinely need to share the base across multiple sibling endpoints.
- Put environment-specific values (base URLs, tokens, hostnames) in `env` blocks instead of hard-coded literals.
- When two or more `.rq` files would share the same `env`, `auth`, or `let` definitions, extract the shared pieces into a dedicated file (e.g. `shared.rq` or `envs.rq`) and `import` it from each consumer. Do not duplicate `env` or `auth` blocks across files. **Always use relative import paths** (e.g. `import \"shared\";`, `import \"common/envs\";`, `import \"../shared\";`) — never absolute paths like `\"/Users/...\"` or `\"/etc/...\"`, even though the parser accepts them. Absolute paths make the file non-portable across machines and break the project as soon as someone else checks it out. The `.rq` extension is optional.
- Inside an `ep` block, name requests after the verb alone — `list` for GET on the collection, `get` for GET on a single resource, plus `post`, `put`, `patch`, `delete`. The endpoint name already supplies the noun, so do not repeat it: write `rq list()`, not `rq get_widgets()`. Add a descriptive name (with `[method(VERB)]` if needed) only when two requests under the same `ep` share a verb (e.g. `rq create_one` next to `rq create_from_csv`, both POSTing). Outside of an `ep`, use a descriptive name (the noun belongs in the request name).
- For a path parameter that the caller supplies at runtime, pass it as a bare identifier in URL position with `[required(name)]`, rather than declaring a `let` and interpolating with `{{name}}` in a string URL.
- For write actions, include a body. Default pattern: `body: io.read_file(\"<entity>-<verb>.json\")` — a JSON fixture next to the .rq file named after the entity and verb (e.g. `users-post.json`, `users-put.json`, `users-patch.json`) so the user has a clear place to edit the payload. POST, PUT, and PATCH should generally have a body; DELETE typically should not. Omit the body only if the user explicitly says the request needs none.
- **JSON body syntax: always use the `${...}` prefix, never a quoted string.** For inline JSON, write `body: ${\"name\": \"alice\"}` or `body: ${}` for an empty object. NEVER write `body: \"{}\"` or `body: \"{\\\"name\\\": \\\"alice\\\"}\"` — those send a string body, not JSON, and will break the receiving API. The `${...}` form also auto-adds the `Accept: application/json` header.

Examples.

Single request for an entity — no `ep` needed, descriptive name carries the noun:
```
rq get_widget(\"http://localhost:8080/widgets/1\");
```

Multiple requests for the same entity — refactor into a shared `ep` with verb-only names and bodies on write actions:
```
ep users(\"http://localhost:8080/users\") {
    rq list();

    [required(user_id)]
    rq get(user_id);

    rq post(body: io.read_file(\"users-post.json\"));

    [required(user_id)]
    rq put(user_id, body: io.read_file(\"users-put.json\"));

    [required(user_id)]
    rq patch(user_id, body: io.read_file(\"users-patch.json\"));

    [required(user_id)]
    rq delete(user_id);
}
```

Multi-file split — shared env/auth in one file, domain endpoints in their own files. Use this when more than one `.rq` file would otherwise duplicate the same env or auth block.

shared.rq:
```
env local {
    base_url: \"http://localhost:8080\",
}
```

users.rq:
```
import \"shared\";

ep users(\"{{base_url}}/users\") {
    rq list();

    [required(user_id)]
    rq get(user_id);
}
```

widgets.rq:
```
import \"shared\";

ep widgets(\"{{base_url}}/widgets\") {
    rq list();

    [required(widget_id)]
    rq get(widget_id);
}
```

Avoid this shape (let-then-interpolate, base-ep extension, duplicated nouns, missing bodies on write actions):
```
let user_id = \"1\";
ep users_base(\"http://localhost:8080/users\");
ep users<users_base>() {
    rq get_users(\"\");
    rq get_user(\"/{{user_id}}\");
    rq post_user(\"\");
    rq put_user(\"/{{user_id}}\");
    rq patch_user(\"/{{user_id}}\");
    rq delete_user(\"/{{user_id}}\");
}
```

Also avoid: duplicating the same `env local { base_url: … }` block across `users.rq` and `widgets.rq` instead of extracting it to a shared file.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ValidateRqParams {
    source: String,
    #[serde(default)]
    path: Option<String>,
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
        description = "Validate rqlang source against the real parser and analyzer. Returns { ok, diagnostics[] } where each diagnostic has severity, message, line, column, and optional file. `path` is the logical filename used in diagnostics; `env` selects an environment for variable resolution."
    )]
    fn validate_rq(
        &self,
        Parameters(ValidateRqParams { source, path, env }): Parameters<ValidateRqParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = validate_source(&source, path.as_deref(), env.as_deref())
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
    let mut steps: Vec<String> = Vec::new();
    steps.push(
        "Read the `rqlang://language-definition` resource for the grammar AND the \
         `rqlang://idioms` resource for style preferences and canonical examples."
            .into(),
    );
    let list_requests_step = match &args.workspace_path {
        Some(path) => format!(
            "Call the `list_requests` tool with path=\"{path}\" so any names you \
             generate don't collide with existing requests."
        ),
        None => "Call the `list_requests` tool (omit `path` — it defaults to the current \
             workspace) so any names you generate don't collide with existing requests."
            .into(),
    };
    steps.push(list_requests_step);
    steps.push("Draft a `.rq` snippet satisfying the intent.".into());
    steps.push(
        "Call the `validate_rq` tool with the draft as `source`. If diagnostics come \
         back, fix them and re-validate. Iterate until `ok: true`."
            .into(),
    );
    let lint_step = match &args.workspace_path {
        Some(path) => format!(
            "Call the `lint_rq` tool with the same `source` AND `workspace_path=\"{path}\"` so \
             the lint sees requests in other .rq files and can flag cross-file conflicts. If \
             diagnostics come back, apply the `suggested_fix` for each one (or rewrite to \
             satisfy the `rule`), re-validate, and re-lint. Iterate until `lint_rq` returns \
             `ok: true`."
        ),
        None => "Call the `lint_rq` tool with the same `source` and pass `workspace_path` as \
             the current working directory (or omit it if you cannot determine the workspace) \
             so the lint can flag cross-file conflicts. If diagnostics come back, apply the \
             `suggested_fix` for each one (or rewrite to satisfy the `rule`), re-validate, \
             and re-lint. Iterate until `lint_rq` returns `ok: true`."
            .into(),
    };
    steps.push(lint_step);
    steps.push(
        "Present the validated snippet in a markdown code block; briefly explain any \
         non-obvious choices."
            .into(),
    );
    let workflow: String = steps
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {s}\n", i + 1))
        .collect();

    let constraints = "\
Constraints:
- Use only constructs documented in `rqlang://language-definition`. Do not invent syntax.
- Identifiers (request names, variable names, env names) use snake_case.
- Variable references use `{{name}}`. The `${...}` prefix is for JSON literal bodies (e.g. `body: ${\"name\": \"alice\"}`) — it is NOT variable interpolation, and a JSON body must never be written as a quoted string like `body: \"{}\"`.
- Do not run the request yourself.
- Do not suggest CLI commands (e.g. `rq request run …`) for executing the generated snippet. Only mention how to run it if the user explicitly asks.";

    format!(
        "You are helping the user author an rqlang (.rq) snippet for the following intent:\n\n\
         > {intent}\n\n\
         Workflow:\n{workflow}\n{constraints}",
        intent = args.intent,
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
        .map(|e| map_diagnostic(e, &resolved))
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
    env: Option<&str>,
) -> Result<ValidateResult, String> {
    let display_path = path.unwrap_or("<inline>");
    let dir = tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {e}"))?;
    let file_path = dir.path().join("inline.rq");
    std::fs::write(&file_path, source).map_err(|e| format!("failed to write temp file: {e}"))?;
    let errors = RqClient::default()
        .check_path(&file_path, env)
        .map_err(|e| format!("check failed: {e}"))?;
    let diagnostics: Vec<ValidateDiagnostic> = errors
        .into_iter()
        .map(|e| map_diagnostic(e, display_path))
        .collect();
    Ok(ValidateResult {
        ok: diagnostics.is_empty(),
        diagnostics,
    })
}

fn map_diagnostic(error: RqError, display_path: &str) -> ValidateDiagnostic {
    match error {
        RqError::Syntax(se) => ValidateDiagnostic {
            severity: "error",
            message: se.message,
            line: se.line,
            column: se.column,
            file: Some(display_path.to_string()),
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
        .with_instructions(
            "rq-mcp v1. Tools: validate_rq (parse + analyze, returns syntax/semantic \
             diagnostics), lint_rq (style/idiom rules, returns rule-tagged diagnostics with \
             suggested fixes), list_requests (enumerate named requests under a path so \
             generated ones avoid name collisions). After drafting any .rq snippet, always \
             call validate_rq first; once it returns ok:true, call lint_rq and iterate \
             until that also returns ok:true. Resources: rqlang://language-definition \
             (full rqlang reference) and rqlang://idioms (style preferences and canonical \
             examples) — read both before generating or refactoring any .rq file. Prompts: \
             generate_rq (drives the full generate → validate → lint → iterate loop). \
             Behavior: this server is for authoring .rq files, not running them. Do not \
             propose or suggest CLI commands (e.g. `rq request run …`) for executing \
             generated snippets unless the user explicitly asks how to run them."
                .to_string(),
        )
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
                    .with_size(IDIOMS.len() as u32)
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
                IDIOMS,
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
        let target = validate_source("rq basic(\"http://localhost:8080/get\");\n", None, None)
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
    fn validate_source_defaults_file_to_inline_when_no_path() {
        let target =
            validate_source("rq basic(\"http://x\"\n", None, None).expect("validate_source failed");
        assert!(!target.ok);
        assert_eq!(target.diagnostics[0].file.as_deref(), Some("<inline>"));
    }

    #[test]
    fn validate_source_accepts_empty_input() {
        let target = validate_source("", None, None).expect("validate_source failed");
        assert!(target.ok);
        assert!(target.diagnostics.is_empty());
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
            body.contains("rqlang://language-definition"),
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
        assert_eq!(IDIOMS_URI, "rqlang://idioms");
    }

    #[test]
    fn idioms_const_covers_all_canonical_rules() {
        for needle in [
            "[required(",
            "Only introduce an `ep` block when two or more requests share",
            "first call `list_requests`",
            "refactor those siblings into a shared `ep`",
            "env` blocks",
            "extract the shared pieces into a dedicated file",
            "import \"shared\";",
            "Always use relative import paths",
            "never absolute paths",
            "name requests after the verb alone",
            "do not repeat it",
            "For write actions, include a body",
            "JSON body syntax: always use the `${...}` prefix",
            "send a string body, not JSON",
            "io.read_file",
            "users-post.json",
            "users-put.json",
            "users-patch.json",
            "DELETE typically should not",
            "Single request for an entity",
            "refactor into a shared `ep`",
            "Multi-file split",
            "shared.rq:",
            "Avoid this shape",
            "Also avoid: duplicating",
            "rq get_widget(\"http://localhost:8080/widgets/1\")",
            "rq list()",
            "rq get(user_id)",
            "rq post(body: io.read_file",
            "ep users<users_base>",
        ] {
            assert!(
                IDIOMS.contains(needle),
                "IDIOMS const missing canonical rule: {needle}"
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
            body.contains("rqlang://idioms"),
            "prompt should tell the AI to read rqlang://idioms"
        );
        assert!(
            body.contains("rqlang://language-definition"),
            "prompt should still tell the AI to read rqlang://language-definition"
        );
    }

    #[test]
    fn server_instructions_reference_both_resources() {
        let info = RqMcp::new().get_info();
        let instructions = info.instructions.expect("instructions present");
        assert!(
            instructions.contains("rqlang://idioms"),
            "instructions should advertise rqlang://idioms"
        );
        assert!(
            instructions.contains("rqlang://language-definition"),
            "instructions should advertise rqlang://language-definition"
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
    fn language_definition_resource_is_embedded_and_substantive() {
        assert_eq!(LANGUAGE_DEFINITION_URI, "rqlang://language-definition");
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
