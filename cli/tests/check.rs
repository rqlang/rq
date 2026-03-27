mod common;
use common::rq_cmd;
use serde_json::Value;

fn run_check(args: &[&str]) -> (bool, Value) {
    let output = rq_cmd().args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    (output.status.success(), json)
}

fn error_messages(json: &Value) -> Vec<String> {
    json["errors"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|e| e["message"].as_str().unwrap_or("").to_string())
        .collect()
}

fn error_count(json: &Value) -> usize {
    json["errors"].as_array().map(|a| a.len()).unwrap_or(0)
}

#[test]
fn test_check_valid_basic_file() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_basic.rq"]);
    assert!(
        success,
        "expected exit 0, got errors: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_read_file_let_with_missing_file() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_read_file_let.rq"]);
    assert!(
        success,
        "io.read_file with missing file should not be a syntax error, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_read_file_inline_in_request_body() {
    let (success, json) =
        run_check(&["check", "-s", "tests/check/input/valid_read_file_inline.rq"]);
    assert!(
        success,
        "inline io.read_file with missing file should not be a syntax error, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_multiple_read_file_calls_in_endpoint() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/valid_read_file_multiple.rq",
    ]);
    assert!(
        success,
        "multiple io.read_file calls should not produce syntax errors, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_sys_funcs_random_and_datetime() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_sys_funcs.rq"]);
    assert!(
        success,
        "random.guid and datetime.now should be valid, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_variable_reference_chain() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_variable_chain.rq"]);
    assert!(
        success,
        "variable chain let b = a should be valid, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_with_environment_flag() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/valid_with_env.rq",
        "-e",
        "local",
    ]);
    assert!(
        success,
        "file with env var should be valid when -e local is passed, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_valid_directory() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_dir"]);
    assert!(
        success,
        "directory of valid files should pass, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_error_undefined_variable_in_let() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/err_undefined_var_ref.rq"]);
    assert!(!success, "expected exit 1 for undefined variable reference");
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("kk") && msgs[0].contains("not defined"),
        "expected 'kk is not defined', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_undefined_variable_in_url() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/err_undefined_var_url.rq"]);
    assert!(!success, "expected exit 1 for undefined variable in URL");
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("missing_var"),
        "expected message about 'missing_var', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_undefined_variable_in_body() {
    let (success, json) =
        run_check(&["check", "-s", "tests/check/input/err_undefined_var_body.rq"]);
    assert!(!success, "expected exit 1 for undefined variable in body");
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("missing_body_var"),
        "expected message about 'missing_body_var', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_sys_func_missing_required_arg() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/err_sys_func_no_args.rq"]);
    assert!(!success, "expected exit 1 for io.read_file() with no args");
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("read_file") || msgs[0].contains("requires"),
        "expected message about missing arg, got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_env_var_without_env_flag() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_with_env.rq"]);
    assert!(
        !success,
        "expected exit 1 when env var is used without -e flag"
    );
    let msgs = error_messages(&json);
    assert!(
        msgs.iter().any(|m| m.contains("base_url")),
        "expected error about 'base_url', got: {:?}",
        msgs
    );
}

#[test]
fn test_check_error_directory_with_invalid_file() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/dir_with_error"]);
    assert!(
        !success,
        "expected exit 1 when directory contains invalid file"
    );
    assert!(error_count(&json) > 0);
}

#[test]
fn test_check_error_reports_all_errors_not_just_first() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/err_multiple_errors.rq"]);
    assert!(!success, "expected exit 1 for multiple errors");
    let msgs = error_messages(&json);
    assert!(
        msgs.iter().any(|m| m.contains("undefined_one")),
        "expected error for 'undefined_one', got: {:?}",
        msgs
    );
    assert!(
        msgs.iter().any(|m| m.contains("undefined_two")),
        "expected error for 'undefined_two', got: {:?}",
        msgs
    );
}

#[test]
fn test_check_error_output_includes_file_line_column() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/err_undefined_var_ref.rq"]);
    assert!(!success);
    let errors = json["errors"].as_array().unwrap();
    let e = &errors[0];
    assert!(e["file"].as_str().is_some(), "error must have 'file' field");
    assert!(e["line"].as_u64().is_some(), "error must have 'line' field");
    assert!(
        e["column"].as_u64().is_some(),
        "error must have 'column' field"
    );
    assert!(
        e["message"].as_str().is_some(),
        "error must have 'message' field"
    );
    assert!(e["line"].as_u64().unwrap() > 0, "line must be positive");
}

#[test]
fn test_check_error_undefined_var_in_empty_endpoint_url() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_undefined_var_in_empty_endpoint.rq",
    ]);
    assert!(
        !success,
        "expected exit 1 for undefined variable in endpoint URL"
    );
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("not_found"),
        "expected error about 'not_found', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_valid_empty_endpoint_with_defined_var() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/valid_empty_endpoint_with_defined_var.rq",
    ]);
    assert!(
        success,
        "endpoint URL referencing a defined variable should be valid, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_output_is_valid_json() {
    let (success, json) = run_check(&["check", "-s", "tests/check/input/valid_basic.rq"]);
    assert!(success);
    assert!(
        json["errors"].is_array(),
        "output must be JSON with 'errors' array"
    );
}

#[test]
fn test_check_error_undefined_headers_var_in_rq() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_undefined_headers_var_in_rq.rq",
    ]);
    assert!(
        !success,
        "expected exit 1 for undefined headers variable in rq"
    );
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("missing_headers"),
        "expected error about 'missing_headers', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_undefined_headers_var_in_ep() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_undefined_headers_var_in_ep.rq",
    ]);
    assert!(
        !success,
        "expected exit 1 for undefined headers variable in ep"
    );
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("missing_headers"),
        "expected error about 'missing_headers', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_undefined_qs_var_in_ep() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_undefined_qs_var_in_ep.rq",
    ]);
    assert!(!success, "expected exit 1 for undefined qs variable in ep");
    assert_eq!(error_count(&json), 1);
    let msgs = error_messages(&json);
    assert!(
        msgs[0].contains("missing_qs"),
        "expected error about 'missing_qs', got: {}",
        msgs[0]
    );
}

#[test]
fn test_check_error_duplicate_undefined_var_positional_reports_all() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_duplicate_undefined_var_positional.rq",
    ]);
    assert!(!success, "expected exit 1 for undefined variables");
    assert_eq!(
        error_count(&json),
        3,
        "expected 3 errors (url, headers, body), got: {:?}",
        error_messages(&json)
    );
}

#[test]
fn test_check_error_duplicate_undefined_var_named_reports_all() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_duplicate_undefined_var_named.rq",
    ]);
    assert!(!success, "expected exit 1 for undefined variables");
    assert_eq!(
        error_count(&json),
        3,
        "expected 3 errors (url:, headers:, body:), got: {:?}",
        error_messages(&json)
    );
}

#[test]
fn test_check_valid_headers_var_used_in_rq() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/valid_headers_var_in_rq.rq",
    ]);
    assert!(
        success,
        "headers variable used as headers arg should be valid, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_error_interpolation_in_let_uses_env_only_var() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_interpolation_in_let_env_only_var.rq",
    ]);
    assert!(
        !success,
        "expected exit 1 when let interpolation references a var only defined in an env block"
    );
    let msgs = error_messages(&json);
    assert!(
        msgs.iter().any(|m| m.contains("base_url")),
        "expected error about 'base_url', got: {:?}",
        msgs
    );
    let errors = json["errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|e| e["line"].as_u64() == Some(5)),
        "expected an error on line 5 (the let declaration), got: {:?}",
        errors
    );
}

#[test]
fn test_check_valid_interpolation_in_let_with_env_flag() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_interpolation_in_let_env_only_var.rq",
        "-e",
        "local",
    ]);
    assert!(
        success,
        "expected exit 0 when the correct env is selected, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_error_interpolation_in_let_imported_env_only_var() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_interpolation_in_let_imported_env_only_var.rq",
    ]);
    assert!(
        !success,
        "expected exit 1 when imported let interpolation references a var only defined in an env block"
    );
    assert_eq!(
        error_count(&json),
        1,
        "expected exactly 1 error (no cascade), got: {:?}",
        error_messages(&json)
    );
    let errors = json["errors"].as_array().unwrap();
    let e = &errors[0];
    assert!(
        e["message"].as_str().unwrap_or("").contains("base_url"),
        "expected error about 'base_url', got: {}",
        e["message"]
    );
    assert!(
        e["file"]
            .as_str()
            .unwrap_or("")
            .contains("_shared"),
        "expected error to point to the shared/imported file, got: {}",
        e["file"]
    );
}

#[test]
fn test_check_valid_interpolation_in_let_imported_with_env_flag() {
    let (success, json) = run_check(&[
        "check",
        "-s",
        "tests/check/input/err_interpolation_in_let_imported_env_only_var.rq",
        "-e",
        "local",
    ]);
    assert!(
        success,
        "expected exit 0 when the correct env is selected, got: {:?}",
        error_messages(&json)
    );
    assert_eq!(error_count(&json), 0);
}

#[test]
fn test_check_nonexistent_source_exits_nonzero() {
    let output = rq_cmd()
        .args(["check", "-s", "tests/check/input/nonexistent.rq"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "expected failure for nonexistent source"
    );
}
