use crate::syntax::parse_result::{EndpointDefinition, Request};
use crate::syntax::rq_file::RqFile;
use crate::syntax::token::TokenType;
use serde::Serialize;

mod rules;

#[derive(Debug, Clone, Serialize)]
pub struct LintDiagnostic {
    pub severity: &'static str,
    pub rule: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LintResult {
    pub ok: bool,
    pub diagnostics: Vec<LintDiagnostic>,
}

pub struct LintContext<'a> {
    pub source: &'a str,
    pub rq_file: &'a RqFile,
    pub display_path: &'a str,
    pub workspace_requests: &'a [Request],
    pub workspace_endpoints: &'a [EndpointSummary],
}

#[derive(Debug, Clone)]
pub struct EndpointSummary {
    pub name: String,
    pub url: String,
    pub auth: Option<String>,
    pub own_auth: Option<String>,
    pub qs: Option<String>,
    pub file: String,
    pub extends: Option<String>,
    pub is_template: bool,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone)]
pub struct EndpointExtension {
    pub child: String,
    pub base: String,
    pub offset: usize,
}

pub trait LintRule: Send + Sync {
    #[allow(dead_code)]
    fn id(&self) -> &'static str;
    #[allow(dead_code)]
    fn description(&self) -> &'static str;
    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>);
}

#[cfg(feature = "native")]
pub fn lint(
    source: &str,
    path: Option<&str>,
    workspace_path: Option<&std::path::Path>,
) -> LintResult {
    use crate::native::NativeFs;
    let display_path = path.unwrap_or("<inline>");
    let source_path =
        crate::paths::resolve_under_workspace(std::path::Path::new(display_path), workspace_path);
    let rq_file = RqFile::from_content_lenient(source_path.clone(), source, &NativeFs);
    let (workspace_requests, workspace_endpoints) = workspace_path
        .map(|w| collect_workspace(w, path.map(|_| source_path.as_path()), &rq_file))
        .unwrap_or_default();
    lint_rq_file(
        &rq_file,
        source,
        display_path,
        &workspace_requests,
        &workspace_endpoints,
    )
}

pub fn lint_rq_file(
    rq_file: &RqFile,
    source: &str,
    display_path: &str,
    workspace_requests: &[Request],
    workspace_endpoints: &[EndpointSummary],
) -> LintResult {
    let ctx = LintContext {
        source,
        rq_file,
        display_path,
        workspace_requests,
        workspace_endpoints,
    };
    let mut diagnostics = Vec::new();
    for rule in rules::all() {
        rule.check(&ctx, &mut diagnostics);
    }
    LintResult {
        ok: diagnostics.is_empty(),
        diagnostics,
    }
}

pub struct WorkspaceCollector {
    draft_bare_names: std::collections::HashSet<String>,
    seen_requests: std::collections::HashSet<(std::path::PathBuf, usize, usize)>,
    requests: Vec<Request>,
    endpoints: Vec<EndpointSummary>,
}

impl WorkspaceCollector {
    pub fn new(draft_rq_file: &RqFile) -> Self {
        WorkspaceCollector {
            draft_bare_names: draft_rq_file
                .requests
                .iter()
                .map(|r| bare_request_name(&r.request.name).to_string())
                .collect(),
            seen_requests: std::collections::HashSet::new(),
            requests: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    pub fn absorb(
        &mut self,
        path: &std::path::Path,
        content: &str,
        fs: &dyn crate::syntax::fs::Fs,
    ) {
        use std::path::PathBuf;
        let parsed = RqFile::from_content_lenient(path.to_path_buf(), content, fs);
        for req_with_vars in parsed.requests {
            let req = req_with_vars.request;
            let bare = bare_request_name(&req.name).to_string();
            if self.draft_bare_names.contains(&bare) {
                continue;
            }
            let source_key = req
                .source_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_default();
            if !self
                .seen_requests
                .insert((source_key, req.line, req.character))
            {
                continue;
            }
            self.requests.push(req);
        }
        let extensions = endpoint_extensions(content);
        let auth_attributes = endpoint_auth_attributes(content);
        for endpoint in parsed.endpoints.values() {
            if !declared_in(endpoint, path) {
                continue;
            }
            self.endpoints.push(EndpointSummary {
                name: endpoint.name.clone(),
                url: endpoint.url.clone(),
                auth: endpoint.auth.clone(),
                own_auth: auth_attributes
                    .iter()
                    .find(|a| a.endpoint == endpoint.name)
                    .map(|a| a.provider.clone()),
                qs: endpoint.qs.clone(),
                file: path.display().to_string(),
                extends: extensions
                    .iter()
                    .find(|e| e.child == endpoint.name)
                    .map(|e| e.base.clone()),
                is_template: endpoint.is_template,
                line: endpoint.line,
                character: endpoint.character,
            });
        }
    }

    pub fn finish(self) -> (Vec<Request>, Vec<EndpointSummary>) {
        (self.requests, self.endpoints)
    }
}

#[cfg(feature = "native")]
fn collect_workspace(
    workspace_path: &std::path::Path,
    draft_path: Option<&std::path::Path>,
    draft_rq_file: &RqFile,
) -> (Vec<Request>, Vec<EndpointSummary>) {
    use crate::native::NativeFs;

    let mut collector = WorkspaceCollector::new(draft_rq_file);
    let draft_canonical = draft_path.and_then(|p| std::fs::canonicalize(p).ok());

    walk_rq_files(workspace_path, &mut |path| {
        if let Some(draft) = &draft_canonical {
            if let Ok(canon) = std::fs::canonicalize(path) {
                if &canon == draft {
                    return;
                }
            }
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        collector.absorb(path, &content, &NativeFs);
    });

    collector.finish()
}

fn declared_in(endpoint: &EndpointDefinition, path: &std::path::Path) -> bool {
    match &endpoint.source_path {
        Some(source) => std::path::Path::new(source) == path,
        None => true,
    }
}

pub fn local_endpoints<'a>(ctx: &'a LintContext) -> Vec<&'a EndpointDefinition> {
    let mut declared: Vec<&EndpointDefinition> = ctx
        .rq_file
        .endpoints
        .values()
        .filter(|e| !e.is_template)
        .filter(|e| declared_in(e, &ctx.rq_file.path))
        .collect();
    declared.sort_by_key(|e| (e.line, e.character));
    declared
}

pub fn endpoint_children<'a>(
    ctx: &'a LintContext,
) -> Vec<(&'a EndpointDefinition, Vec<&'a Request>)> {
    local_endpoints(ctx)
        .into_iter()
        .map(|endpoint| {
            let children: Vec<&Request> = ctx
                .rq_file
                .requests
                .iter()
                .map(|r| &r.request)
                .filter(|r| r.endpoint.as_deref() == Some(endpoint.name.as_str()))
                .filter(|r| request_declared_in(r, &ctx.rq_file.path))
                .collect();
            (endpoint, children)
        })
        .collect()
}

pub fn query_params(raw_url: &str) -> Vec<(String, String)> {
    let Some((_, query)) = raw_url.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (key.trim().to_string(), value.trim().to_string()),
            None => (pair.trim().to_string(), String::new()),
        })
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

pub fn request_own_headers(
    request: &Request,
    endpoint: &EndpointDefinition,
) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .filter(|(key, value)| {
            !endpoint
                .headers
                .iter()
                .any(|(ep_key, ep_value)| ep_key == key && ep_value == value)
        })
        .cloned()
        .collect()
}

fn request_declared_in(request: &Request, path: &std::path::Path) -> bool {
    match &request.source_path {
        Some(source) => std::path::Path::new(source) == path,
        None => true,
    }
}

pub fn significant_tokens(source: &str) -> Vec<crate::syntax::token::Token> {
    let Ok(tokens) = crate::syntax::tokenize(source) else {
        return Vec::new();
    };
    tokens
        .into_iter()
        .filter(|t| {
            !matches!(
                t.token_type,
                TokenType::Whitespace | TokenType::Newline | TokenType::Comment
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct EndpointAuthAttribute {
    pub endpoint: String,
    pub provider: String,
}

pub fn endpoint_auth_attributes(source: &str) -> Vec<EndpointAuthAttribute> {
    let significant = significant_tokens(source);
    let mut found = Vec::new();
    let mut pending: Option<String> = None;
    let mut index = 0;
    while index < significant.len() {
        let token = &significant[index];
        if let Some(provider) = read_auth_attribute(&significant, index) {
            pending = Some(provider);
            index += 6;
            continue;
        }
        if token.token_type == TokenType::Keyword {
            if token.value == "rq" {
                pending = None;
            }
            if token.value == "ep" {
                if let (Some(provider), Some(name)) = (pending.take(), significant.get(index + 1)) {
                    if name.token_type == TokenType::Identifier {
                        found.push(EndpointAuthAttribute {
                            endpoint: name.value.clone(),
                            provider,
                        });
                    }
                }
            }
        }
        index += 1;
    }
    found
}

fn read_auth_attribute(tokens: &[crate::syntax::token::Token], index: usize) -> Option<String> {
    let window = tokens.get(index..index + 6)?;
    let [open, keyword, lparen, value, rparen, close] = window else {
        return None;
    };
    if open.value != "[" || close.value != "]" {
        return None;
    }
    if keyword.value != "auth" || lparen.value != "(" || rparen.value != ")" {
        return None;
    }
    if value.token_type != TokenType::String {
        return None;
    }
    let raw = &value.value;
    if raw.len() < 2 {
        return None;
    }
    Some(raw[1..raw.len() - 1].to_string())
}

pub fn endpoint_extensions(source: &str) -> Vec<EndpointExtension> {
    let Ok(tokens) = crate::syntax::tokenize(source) else {
        return Vec::new();
    };
    let significant: Vec<&crate::syntax::token::Token> = tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.token_type,
                TokenType::Whitespace | TokenType::Newline | TokenType::Comment
            )
        })
        .collect();

    let mut found = Vec::new();
    for window in significant.windows(4) {
        let [keyword, child, angle, base] = window else {
            continue;
        };
        if keyword.token_type != TokenType::Keyword || keyword.value != "ep" {
            continue;
        }
        if child.token_type != TokenType::Identifier || base.token_type != TokenType::Identifier {
            continue;
        }
        if angle.value != "<" {
            continue;
        }
        found.push(EndpointExtension {
            child: child.value.clone(),
            base: base.value.clone(),
            offset: angle.span.start,
        });
    }
    found
}

#[cfg(feature = "native")]
fn walk_rq_files(root: &std::path::Path, visit: &mut dyn FnMut(&std::path::Path)) {
    if root.is_file() {
        if root.extension().and_then(|s| s.to_str()) == Some("rq") {
            visit(root);
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if crate::paths::is_skipped_directory(&path) {
                continue;
            }
            walk_rq_files(&path, visit);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
            visit(&path);
        }
    }
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

fn line_col(source: &str, offset: usize) -> (usize, usize) {
    if offset > source.len() {
        return (1, 1);
    }
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() + 1;
    let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = prefix[last_line_start..].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::{lint, lint_rq_file};
    use crate::native::NativeFs;
    use crate::syntax::rq_file::RqFile;
    use std::path::PathBuf;

    fn nested_workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (name, content) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, content).expect("write");
        }
        std::fs::create_dir_all(dir.path().join("api")).expect("mkdir");
        dir
    }

    fn workspace_rules(dir: &tempfile::TempDir, draft: &str) -> Vec<String> {
        lint(draft, Some("api/users.rq"), Some(dir.path()))
            .diagnostics
            .into_iter()
            .map(|d| d.rule.to_string())
            .collect()
    }

    fn lint_source(source: &str) -> Vec<String> {
        let rq_file = RqFile::from_content_lenient(PathBuf::from("draft.rq"), source, &NativeFs);
        lint_rq_file(&rq_file, source, "draft.rq", &[], &[])
            .diagnostics
            .into_iter()
            .map(|d| d.rule.to_string())
            .collect()
    }

    #[test]
    fn flags_both_defects_in_generated_endpoint() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      rq list(\"\");\n    \
                      rq get(\"/{widget_id}\");\n}\n";
        let rules = lint_source(source);
        assert!(
            rules.contains(&"empty_url_string".to_string()),
            "expected empty_url_string, got {rules:?}"
        );
        assert!(
            rules.contains(&"single_brace_interpolation".to_string()),
            "expected single_brace_interpolation, got {rules:?}"
        );
    }

    #[test]
    fn flags_a_hand_rolled_bearer_header_on_a_template_endpoint() {
        let source =
            "ep base(url: \"{{base_url}}\", headers: $[\"Authorization\": \"Bearer {{token}}\"]);\n";
        let rules = lint_source(source);
        assert_eq!(
            rules,
            vec!["manual_auth_header".to_string()],
            "the header should be the only complaint, got {rules:?}"
        );
    }

    #[test]
    fn accepts_the_auth_provider_rewrite_of_that_endpoint() {
        let source = "auth api_auth(auth_type.bearer) {\n    \
                      token: \"{{api_token}}\",\n}\n\n\
                      [auth(\"api_auth\")]\n\
                      ep base(url: \"{{base_url}}\");\n";
        let rules = lint_source(source);
        assert!(rules.is_empty(), "expected no diagnostics, got {rules:?}");
    }

    #[test]
    fn accepts_the_idiomatic_rewrite_of_that_endpoint() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      rq list();\n\n    \
                      [required(widget_id)]\n    \
                      rq get(widget_id);\n}\n";
        let rules = lint_source(source);
        assert!(rules.is_empty(), "expected no diagnostics, got {rules:?}");
    }

    #[test]
    fn skips_the_on_disk_draft_when_its_path_is_relative_to_the_workspace() {
        let dir = nested_workspace(&[(
            "api/users.rq",
            "rq list_users(\"http://localhost:8080/users\");\n",
        )]);
        let draft = "rq fetch_users(\"http://localhost:8080/users\");\n";
        let rules: Vec<String> = lint(draft, Some("api/users.rq"), Some(dir.path()))
            .diagnostics
            .into_iter()
            .map(|d| d.rule.to_string())
            .collect();
        assert!(
            !rules.contains(&"top_level_rq_should_be_ep".to_string()),
            "the draft must not be absorbed as its own workspace peer, got {rules:?}"
        );
    }

    #[test]
    fn resolves_imports_of_a_relative_draft_against_the_workspace() {
        let dir = nested_workspace(&[(
            "api/shared.rq",
            "ep base(url: \"http://localhost:8080\", qs: \"v=1\");\n",
        )]);
        let draft =
            "import \"shared\";\n\nep users<base>(\"/users\", qs: \"v=1\") {\n    rq list();\n}\n";
        let rules: Vec<String> = lint(draft, Some("api/users.rq"), Some(dir.path()))
            .diagnostics
            .into_iter()
            .map(|d| d.rule.to_string())
            .collect();
        assert!(
            rules.contains(&"duplicated_ep_config".to_string()),
            "the imported template must resolve under the workspace, got {rules:?}"
        );
    }

    #[test]
    fn reports_the_logical_path_even_when_it_is_resolved_under_the_workspace() {
        let dir = nested_workspace(&[]);
        let draft = "rq list(\"\");\n";
        let target = lint(draft, Some("api/users.rq"), Some(dir.path()));
        assert_eq!(
            target.diagnostics[0].file.as_deref(),
            Some("api/users.rq"),
            "diagnostics keep the logical path the caller passed"
        );
    }

    #[test]
    fn does_not_walk_into_dependency_and_build_directories() {
        let peer = "rq fetch_users(\"http://localhost:8080/users\");\n";
        for skipped in ["node_modules", ".git", "target"] {
            let dir = nested_workspace(&[(&format!("{skipped}/dep.rq"), peer)]);
            let rules = workspace_rules(&dir, "rq list(\"http://localhost:8080/users\");\n");
            assert!(
                !rules.contains(&"top_level_rq_should_be_ep".to_string()),
                "`{skipped}` must not be scanned as workspace source, got {rules:?}"
            );
        }
    }

    #[test]
    fn still_walks_ordinary_nested_directories() {
        let dir = nested_workspace(&[(
            "vendor/dep.rq",
            "rq fetch_users(\"http://localhost:8080/users\");\n",
        )]);
        let rules = workspace_rules(&dir, "rq list(\"http://localhost:8080/users\");\n");
        assert!(
            rules.contains(&"top_level_rq_should_be_ep".to_string()),
            "an ordinary subdirectory is still workspace source, got {rules:?}"
        );
    }
}
