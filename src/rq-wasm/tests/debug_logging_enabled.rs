use rq_wasm::bindings;

#[test]
fn enabled_debug_logging_captures_the_trace_of_a_call() {
    bindings::set_debug_logging(true);
    let files = r#"{"/workspace/api.rq": "env local {\n    host: \"x\",\n}\n"}"#;
    let _ = bindings::list_environments(files, "{}", "/workspace");
    let target = bindings::take_debug_logs();
    assert!(target.contains("* Parsed /workspace/api.rq"), "{target}");
}
