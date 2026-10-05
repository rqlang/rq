use rq_wasm::bindings;

#[test]
fn disabled_debug_logging_captures_nothing() {
    bindings::set_debug_logging(true);
    bindings::set_debug_logging(false);
    let files = r#"{"/workspace/api.rq": "env local {\n    host: \"x\",\n}\n"}"#;
    let _ = bindings::list_environments(files, "{}", "/workspace");
    let target = bindings::take_debug_logs();
    assert_eq!(target, "");
}
