use rq_wasm::bindings;
use serde_json::Value;

const FILES: &str = r#"{"/workspace/users.rq": "[doc.description(\"Users of the platform\")]\nep users(\"http://localhost:8080/users\") {\n    [doc.description(\"List every user\")]\n    rq list();\n\n    rq count(\"count\");\n}\n"}"#;

fn listed_request(name: &str) -> Value {
    let output =
        bindings::list_requests(FILES, "{}", "/workspace").unwrap_or_else(|_| String::from("{}"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    target["requests"]
        .as_array()
        .and_then(|requests| requests.iter().find(|r| r["name"] == name).cloned())
        .unwrap_or(Value::Null)
}

#[test]
fn list_requests_exposes_the_description_of_each_request() {
    let target = listed_request("users/list");
    assert_eq!(target["description"], "List every user");
}

#[test]
fn list_requests_exposes_the_description_of_the_endpoint() {
    let target = listed_request("users/count");
    assert_eq!(target["endpoint_description"], "Users of the platform");
}

#[test]
fn list_requests_reports_a_request_without_description_as_null() {
    let target = listed_request("users/count");
    assert_eq!(target["description"], Value::Null);
}

#[test]
fn list_requests_exposes_the_method_and_the_uninterpolated_url() {
    let target = listed_request("users/count");
    assert_eq!(
        (&target["method"], &target["url"]),
        (
            &Value::from("GET"),
            &Value::from("http://localhost:8080/users/count")
        )
    );
}

#[test]
fn get_request_details_exposes_the_description() {
    let output = bindings::get_request_details(
        FILES,
        "{}",
        "/workspace",
        "users/list",
        None,
        false,
        false,
        None,
        None,
    )
    .unwrap_or_else(|_| String::from("{}"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    assert_eq!(target["Description"], "List every user");
}
