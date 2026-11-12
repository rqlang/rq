use rq_wasm::bindings;
use serde_json::Value;

#[test]
fn variable_references_count_utf16_columns() {
    let workspace = tempfile::tempdir().unwrap();
    let file = workspace.path().join("a.rq");
    std::fs::write(&file, "[required(id)]\nrq get(\"😀{{id}}\", body: id);\n").unwrap();
    let source = workspace.path().to_string_lossy().replace('\\', "/");
    let scope_file = file.to_string_lossy().replace('\\', "/");
    let output = bindings::list_variable_refs(
        "{}",
        "{}",
        &source,
        "id",
        Some(scope_file),
        Some(1),
        Some(12),
    )
    .unwrap_or_default();
    let target: Vec<(u64, u64)> = serde_json::from_str::<Value>(&output)
        .unwrap_or(Value::Null)
        .as_array()
        .map(|refs| {
            refs.iter()
                .filter_map(|r| Some((r["line"].as_u64()?, r["character"].as_u64()?)))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(target, vec![(0, 10), (1, 12), (1, 25)]);
}

#[test]
fn check_reports_utf16_columns() {
    let workspace = tempfile::tempdir().unwrap();
    let file = workspace.path().join("a.rq");
    std::fs::write(&file, "rq get(url: \"http://x/😀\", body: not_found);\n").unwrap();
    let source = file.to_string_lossy().replace('\\', "/");
    let output = bindings::check("{}", "{}", &source, None).unwrap_or_default();
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    assert_eq!(target["errors"][0]["column"], Value::from(34), "{output}");
}
