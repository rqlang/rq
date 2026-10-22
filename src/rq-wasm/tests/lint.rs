use rq_wasm::bindings;
use serde_json::Value;

const DRAFT: &str = "ep widgets(\"http://localhost:8080/widgets\") {\n    rq list();\n}\n";

fn rules(output: &str, draft: &str) -> Vec<String> {
    let target: Value = serde_json::from_str(output).unwrap_or(Value::Null);
    target[draft]["diagnostics"]
        .as_array()
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|d| d["rule"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn lint_draft_in(workspace: &std::path::Path, others: &[(&str, &str)]) -> Vec<String> {
    let root = workspace.to_string_lossy().replace('\\', "/");
    let draft = format!("{root}/widgets.rq");
    let mut files = serde_json::Map::new();
    files.insert(draft.clone(), Value::from(DRAFT));
    for (name, source) in others {
        files.insert(format!("{root}/{name}"), Value::from(*source));
    }
    let output = bindings::lint(&Value::Object(files).to_string(), Some(root))
        .unwrap_or_else(|_| String::from("{}"));
    rules(&output, &draft)
}

#[test]
fn lint_reads_the_workspace_files_saved_on_disk() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("users.rq"),
        "ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n",
    )
    .unwrap();
    let target = lint_draft_in(workspace.path(), &[]);
    assert!(
        target.contains(&"duplicated_ep_base".to_string()),
        "{target:?}"
    );
}

#[test]
fn lint_reads_the_other_unsaved_drafts() {
    let workspace = tempfile::tempdir().unwrap();
    let target = lint_draft_in(
        workspace.path(),
        &[(
            "users.rq",
            "ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n",
        )],
    );
    assert!(
        target.contains(&"duplicated_ep_base".to_string()),
        "{target:?}"
    );
}

#[test]
fn lint_returns_a_result_for_every_draft() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().to_string_lossy().replace('\\', "/");
    let files = serde_json::json!({
        format!("{root}/a.rq"): "rq a(\"\");\n",
        format!("{root}/b.rq"): "rq b(\"http://x\");\n",
    })
    .to_string();
    let output = bindings::lint(&files, Some(root.clone())).unwrap_or_default();
    assert!(rules(&output, &format!("{root}/a.rq")).contains(&"empty_url_string".to_string()));
    assert!(!rules(&output, &format!("{root}/b.rq")).contains(&"empty_url_string".to_string()));
}
