use rq_wasm::bindings;
use serde_json::Value;

fn required_variable_at(content: &str, line: usize, character: usize) -> Value {
    let workspace = tempfile::tempdir().unwrap();
    let file = workspace.path().join("a.rq");
    std::fs::write(&file, content).unwrap();
    let source = workspace.path().to_string_lossy().replace('\\', "/");
    let scope_file = file.to_string_lossy().replace('\\', "/");
    let output =
        bindings::get_required_variable("{}", "{}", &source, "id", &scope_file, line, character)
            .unwrap_or_default();
    serde_json::from_str(&output).unwrap_or(Value::String(output))
}

#[test]
fn required_variable_points_at_the_attribute_of_the_enclosing_request() {
    let target = required_variable_at("[required(id)]\nrq get(id);\n", 1, 7);
    assert_eq!(
        (&target["line"], &target["character"], &target["source"]),
        (&Value::from(0), &Value::from(10), &Value::from("required"))
    );
}

#[test]
fn required_variable_outside_a_declaring_request_is_null() {
    let target = required_variable_at(
        "[required(id)]\nrq get(id);\nlet url = \"{{id}}\";\n",
        2,
        13,
    );
    assert_eq!(target, Value::Null);
}
