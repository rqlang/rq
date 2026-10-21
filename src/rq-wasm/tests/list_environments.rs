use rq_wasm::bindings;
use serde_json::Value;

#[test]
fn list_environments_exposes_the_ui_color_of_each_environment() {
    let files = r#"{"/workspace/api.rq": "[ui.color(red)]\nenv production {\n    host: \"x\",\n}\n\nenv local {\n    host: \"y\",\n}\n"}"#;
    let output = bindings::list_environments(files, "{}", "/workspace")
        .unwrap_or_else(|_| String::from("{}"));
    let target: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
    let colors: Vec<(&str, &Value)> = target["environments"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|e| (e["name"].as_str().unwrap_or_default(), &e["color"]))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        colors,
        [
            ("local", &Value::Null),
            ("production", &Value::String("red".to_string()))
        ]
    );
}
