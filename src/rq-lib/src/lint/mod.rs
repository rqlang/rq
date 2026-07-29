use crate::syntax::parse_result::Request;
use crate::syntax::rq_file::RqFile;
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
    use std::path::PathBuf;
    let display_path = path.unwrap_or("<inline>");
    let rq_file = RqFile::from_content_lenient(PathBuf::from(display_path), source, &NativeFs);
    let workspace_requests = workspace_path
        .map(|w| collect_workspace_requests(w, path.map(std::path::Path::new), &rq_file))
        .unwrap_or_default();
    lint_rq_file(&rq_file, source, display_path, &workspace_requests)
}

pub fn lint_rq_file(
    rq_file: &RqFile,
    source: &str,
    display_path: &str,
    workspace_requests: &[Request],
) -> LintResult {
    let ctx = LintContext {
        source,
        rq_file,
        display_path,
        workspace_requests,
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

#[cfg(feature = "native")]
fn collect_workspace_requests(
    workspace_path: &std::path::Path,
    draft_path: Option<&std::path::Path>,
    draft_rq_file: &RqFile,
) -> Vec<Request> {
    use crate::native::NativeFs;
    use std::collections::HashSet;
    use std::path::PathBuf;

    let draft_bare_names: HashSet<String> = draft_rq_file
        .requests
        .iter()
        .map(|r| bare_request_name(&r.request.name).to_string())
        .collect();

    let mut seen_requests: HashSet<(PathBuf, usize, usize)> = HashSet::new();
    let draft_canonical = draft_path.and_then(|p| std::fs::canonicalize(p).ok());

    let mut requests = Vec::new();
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
        let parsed = RqFile::from_content_lenient(path.to_path_buf(), &content, &NativeFs);
        for req_with_vars in parsed.requests {
            let req = req_with_vars.request;
            let bare = bare_request_name(&req.name).to_string();
            if draft_bare_names.contains(&bare) {
                continue;
            }
            let source_key = req
                .source_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_default();
            if !seen_requests.insert((source_key, req.line, req.character)) {
                continue;
            }
            requests.push(req);
        }
    });

    requests
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
            walk_rq_files(&path, visit);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
            visit(&path);
        }
    }
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}
