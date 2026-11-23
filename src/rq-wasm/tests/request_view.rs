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

const SHARED_VIEW: &str = r#"{"/workspace/api/orders.rq": "[ui.view(\"../views/orders.html\")]\nrq list(\"http://localhost:8080/orders\");\n", "/workspace/views/orders.html": "<p></p>\n"}"#;

fn check_errors(source_root: Option<String>) -> Value {
    bindings::check(
        SHARED_VIEW,
        "{}",
        "/workspace/api/orders.rq",
        None,
        source_root,
    )
    .ok()
    .and_then(|output| serde_json::from_str::<Value>(&output).ok())
    .map(|result| result["errors"].clone())
    .unwrap_or(Value::Null)
}

#[test]
fn check_accepts_a_view_inside_the_source_root_it_is_given() {
    assert_eq!(
        check_errors(Some("/workspace".to_string())),
        serde_json::json!([])
    );
}

#[test]
fn check_measures_a_view_against_the_folder_of_the_file_without_a_source_root() {
    let target = check_errors(None);
    assert!(target[0]["message"]
        .as_str()
        .is_some_and(|message| message.contains("is outside the source folder")));
}
