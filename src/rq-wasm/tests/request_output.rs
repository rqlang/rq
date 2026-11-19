use rq_wasm::bindings;
use serde_json::Value;

const FILES: &str = r#"{"/workspace/api/users.rq": "[output(\"out/\")]\nep users(\"http://localhost:8080/users\") {\n    [output(\"../../../users.json\")]\n    rq export();\n\n    rq list();\n}\n"}"#;

fn details(name: &str) -> Value {
    bindings::get_request_details(
        FILES,
        "{}",
        "/workspace",
        name,
        None,
        true,
        false,
        None,
        None,
    )
    .ok()
    .and_then(|output| serde_json::from_str(&output).ok())
    .unwrap_or(Value::Null)
}

#[test]
fn request_details_report_the_resolved_output() {
    let target = &details("users/list")["Output"];
    assert_eq!(
        (
            target["path"].as_str().map(|path| path.replace('\\', "/")),
            &target["applies_to_every_body"]
        ),
        (Some("/workspace/api/out".to_string()), &Value::Bool(false))
    );
}

#[test]
fn request_details_report_an_output_outside_the_workspace() {
    let target = &details("users/export")["Output"];
    assert!(target["error"].is_string());
}

#[test]
fn request_details_leave_out_a_request_without_output() {
    let files = r#"{"/workspace/health.rq": "rq health(\"http://localhost:8080/health\");\n"}"#;
    let target = bindings::get_request_details(
        files,
        "{}",
        "/workspace",
        "health",
        None,
        true,
        false,
        None,
        None,
    )
    .ok()
    .and_then(|output| serde_json::from_str::<Value>(&output).ok())
    .unwrap_or(Value::Null);
    assert!(target.get("Output").is_none() && target.get("Request").is_some());
}
