use rq_wasm::bindings;
use serde_json::Value;

#[test]
fn auth_fields_marks_redirect_uri_required_for_the_implicit_flow() {
    let output = bindings::auth_fields().unwrap_or_else(|_| String::from("[]"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    let implicit: Vec<(&str, bool)> = target
        .as_array()
        .and_then(|types| {
            types
                .iter()
                .find(|entry| entry["auth_type"] == "oauth2_implicit")
        })
        .and_then(|entry| entry["fields"].as_array())
        .map(|fields| {
            fields
                .iter()
                .map(|f| {
                    (
                        f["name"].as_str().unwrap_or_default(),
                        f["required"].as_bool().unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        implicit,
        [
            ("client_id", true),
            ("authorization_url", true),
            ("redirect_uri", true),
            ("scope", false)
        ]
    );
}
