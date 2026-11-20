use rq_wasm::bindings;
use serde_json::Value;

const FILES: &str = r#"{"/workspace/api/orders.rq": "ep orders(\"http://localhost:8080/orders\") {\n    [ui.view(\"views/orders.html\")]\n    rq list();\n\n    rq count();\n}\n", "/workspace/api/views/orders.html": "<p></p>\n"}"#;

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

fn prepared(name: &str) -> Value {
    bindings::prepare_request(FILES, "{}", "/workspace", name, None, None, None)
        .ok()
        .and_then(|output| serde_json::from_str(&output).ok())
        .unwrap_or(Value::Null)
}

#[test]
fn request_details_report_the_resolved_view() {
    let target = &details("orders/list")["View"];
    assert_eq!(
        target["path"].as_str().map(|path| path.replace('\\', "/")),
        Some("/workspace/api/views/orders.html".to_string())
    );
}

#[test]
fn prepared_request_reports_the_same_view_as_the_details() {
    assert_eq!(
        prepared("orders/list")["view"],
        details("orders/list")["View"]
    );
}

#[test]
fn request_details_leave_out_a_request_without_view() {
    let target = details("orders/count");
    assert!(target.get("View").is_none() && target.get("Request").is_some());
}
