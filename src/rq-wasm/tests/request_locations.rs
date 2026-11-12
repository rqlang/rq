use rq_wasm::bindings;
use serde_json::Value;

const FILES: &str = r#"{"/workspace/a.rq": "import \"b.rq\";\nrq a_req(\"http://x/a\");\n", "/workspace/b.rq": "let x = \"1\";\n\nrq b_req(\"http://x/b\");\n"}"#;

fn imported_request_details() -> Value {
    let output = bindings::get_request_details(
        FILES,
        "{}",
        "/workspace",
        "b_req",
        None,
        false,
        false,
        None,
        Some("/workspace/a.rq".to_string()),
    )
    .unwrap_or_else(|_| String::from("{}"));
    serde_json::from_str(&output).unwrap_or(Value::Null)
}

fn listed_imported_request() -> Value {
    let output = bindings::list_requests(FILES, "{}", "/workspace/a.rq")
        .unwrap_or_else(|_| String::from("{}"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    target["requests"]
        .as_array()
        .and_then(|requests| requests.iter().find(|r| r["name"] == "b_req").cloned())
        .unwrap_or(Value::Null)
}

#[test]
fn get_request_details_keeps_the_file_an_imported_request_is_loaded_from() {
    let target = imported_request_details();
    assert_eq!(target["file"], "/workspace/a.rq");
}

#[test]
fn get_request_details_locates_an_imported_request_in_the_file_that_declares_it() {
    let target = imported_request_details();
    assert_eq!(
        (&target["request_file"], &target["line"]),
        (&Value::from("/workspace/b.rq"), &Value::from(2))
    );
}

#[test]
fn list_requests_locates_an_imported_request_in_the_file_that_declares_it() {
    let target = listed_imported_request();
    assert_eq!(
        (&target["file"], &target["request_file"], &target["line"]),
        (
            &Value::from("/workspace/a.rq"),
            &Value::from("/workspace/b.rq"),
            &Value::from(2)
        )
    );
}
