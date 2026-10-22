use rq_wasm::bindings;
use serde_json::Value;

#[test]
fn list_variables_of_a_saved_file_includes_the_variables_it_imports() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("common.rq"),
        "let base_url = \"http://x\";\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("a.rq"),
        "import \"common\";\n\nrq list(\"{{base_url}}/items\");\n",
    )
    .unwrap();
    let source = workspace
        .path()
        .join("a.rq")
        .to_string_lossy()
        .replace('\\', "/");
    let output =
        bindings::list_variables("{}", "{}", &source, None).unwrap_or_else(|_| String::from("[]"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    let names: Vec<&str> = target
        .as_array()
        .map(|vars| vars.iter().filter_map(|v| v["name"].as_str()).collect())
        .unwrap_or_default();
    assert!(names.contains(&"base_url"), "{names:?}");
}
