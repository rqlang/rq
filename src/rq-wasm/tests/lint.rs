use rq_wasm::bindings;
use serde_json::Value;

const DRAFT: &str = "ep widgets(\"http://localhost:8080/widgets\") {\n    rq list();\n}\n";

fn rules(output: &str) -> Vec<String> {
    let target: Value = serde_json::from_str(output).unwrap_or(Value::Null);
    target["diagnostics"]
        .as_array()
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|d| d["rule"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn lint_draft_in(workspace: &std::path::Path, files_json: &str) -> Vec<String> {
    let root = workspace.to_string_lossy().replace('\\', "/");
    let draft = format!("{root}/widgets.rq");
    let output = bindings::lint(files_json, DRAFT, &draft, Some(root))
        .unwrap_or_else(|_| String::from("{}"));
    rules(&output)
}

#[test]
fn lint_reads_the_workspace_files_saved_on_disk() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("users.rq"),
        "ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n",
    )
    .unwrap();
    let target = lint_draft_in(workspace.path(), "{}");
    assert!(
        target.contains(&"duplicated_ep_base".to_string()),
        "{target:?}"
    );
}

#[test]
fn lint_reads_the_other_unsaved_drafts() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().to_string_lossy().replace('\\', "/");
    let files = serde_json::json!({
        format!("{root}/users.rq"): "ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n"
    })
    .to_string();
    let target = lint_draft_in(workspace.path(), &files);
    assert!(
        target.contains(&"duplicated_ep_base".to_string()),
        "{target:?}"
    );
}
