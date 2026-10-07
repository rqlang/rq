// Integration tests for rq CLI
use libtest_mimic::{run, Arguments, Failed, Trial};
use std::fs;
use std::path::Path;

mod common;
use common::{rq_cmd, validate_json_response};

fn main() {
    let args = Arguments::from_args();

    let mut trials: Vec<Trial> = vec![
        // Fixture tests (Manual setup)
        Trial::test("request_secrets_env_vars", test_request_secrets),
        Trial::test(
            "request_json_warning_uses_warning_key",
            test_request_json_warning_uses_warning_key,
        ),
        Trial::test(
            "request_json_output_ends_with_newline",
            test_request_json_output_ends_with_newline,
        ),
        Trial::test(
            "request_text_output_starts_with_status",
            test_request_text_output_starts_with_status,
        ),
        Trial::test(
            "request_legacy_output_flag_points_to_format",
            test_request_legacy_output_flag_points_to_format,
        ),
        Trial::test(
            "request_print_headers_omits_meta_and_body",
            test_request_print_headers_omits_meta_and_body,
        ),
        Trial::test(
            "request_print_body_omits_meta",
            test_request_print_body_omits_meta,
        ),
        Trial::test(
            "request_print_filters_json_fields",
            test_request_print_filters_json_fields,
        ),
        Trial::test(
            "request_json_default_prints_meta_and_body",
            test_request_json_default_prints_meta_and_body,
        ),
        Trial::test(
            "request_print_rejects_unknown_part",
            test_request_print_rejects_unknown_part,
        ),
        Trial::test(
            "request_debug_logs_masked_request_and_response",
            test_request_debug_logs_masked_request_and_response,
        ),
        Trial::test(
            "request_debug_header_masks_cli_variables",
            test_request_debug_header_masks_cli_variables,
        ),
        Trial::test(
            "request_debug_footer_reports_exit_code",
            test_request_debug_footer_reports_exit_code,
        ),
        Trial::test(
            "request_debug_traces_secret_and_variable_sources",
            test_request_debug_traces_secret_and_variable_sources,
        ),
        Trial::test(
            "request_secrets_uppercase_prefixes",
            test_request_secrets_uppercase_prefixes,
        ),
        Trial::test(
            "request_auth_token_backdoor",
            test_request_auth_token_backdoor,
        ),
        Trial::test(
            "request_auth_token_backdoor_upper",
            test_request_auth_token_backdoor_upper,
        ),
        Trial::test(
            "request_cli_variable_override",
            test_request_cli_variable_override,
        ),
        Trial::test("request_dotenv_file", test_request_dotenv),
        Trial::test(
            "request_run_file_not_found",
            test_request_run_file_not_found,
        ),
        Trial::test(
            "request_run_invalid_request_name",
            test_request_run_invalid_request_name,
        ),
        Trial::test(
            "request_run_invalid_variable_format",
            test_request_run_invalid_variable_format,
        ),
        Trial::test(
            "request_run_invalid_variable_name",
            test_request_run_invalid_variable_name,
        ),
        Trial::test(
            "request_run_ep_dot_notation",
            test_request_run_ep_dot_notation,
        ),
        Trial::test(
            "request_run_connection_refused",
            test_request_run_connection_refused,
        ),
        Trial::test(
            "request_required_variable_satisfied_by_cli",
            test_request_required_variable_satisfied_by_cli,
        ),
        Trial::test(
            "request_read_file_body_sends_no_content_type",
            test_read_file_body_sends_no_content_type,
        ),
        Trial::test(
            "request_read_json_body_sends_json_content_type",
            test_read_json_body_sends_json_content_type,
        ),
        Trial::test(
            "request_run_prints_lint_summary",
            test_request_run_prints_lint_summary,
        ),
        Trial::test(
            "request_run_json_lint_summary_uses_warning_key",
            test_request_run_json_lint_summary_uses_warning_key,
        ),
        Trial::test(
            "request_run_no_lint_skips_lint_summary",
            test_request_run_no_lint_skips_lint_summary,
        ),
        Trial::test(
            "request_run_lint_summary_quotes_source_with_spaces",
            test_request_run_lint_summary_quotes_source_with_spaces,
        ),
        Trial::test(
            "request_run_file_lint_ignores_sibling_files",
            test_request_run_file_lint_ignores_sibling_files,
        ),
        #[cfg(unix)]
        Trial::test(
            "request_run_terminates_on_symlink_loop",
            test_request_run_terminates_on_symlink_loop,
        ),
    ];

    // Discover tests from organized directories
    let discovered_tests = discover_directory_tests();
    trials.extend(discovered_tests);

    run(&args, trials).exit();
}

// --- Fixture Tests ---

fn test_request_json_warning_uses_warning_key() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/input/foo.rq",
            "-f",
            "json",
        ])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warning: serde_json::Value = serde_json::from_str(stderr.trim())
        .map_err(|e| format!("stderr is not valid JSON: {e}\n{stderr}"))?;
    if warning["warning"]["message"] != "No requests found in the file" {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

const LINT_WARNING_FIXTURE: &str = "tests/request/run/fixtures/lint_warning/users.rq";

fn run_lint_warning_fixture(extra: &[&str]) -> Result<std::process::Output, Failed> {
    rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            LINT_WARNING_FIXTURE,
            "-n",
            "missing",
        ])
        .args(extra)
        .output()
        .map_err(|e| format!("Failed to execute: {e}").into())
}

fn test_request_run_prints_lint_summary() -> Result<(), Failed> {
    let output = run_lint_warning_fixture(&[])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!(
        "Warning: 1 lint warning found, run `rq check -s {LINT_WARNING_FIXTURE}` for details\n\
         Error: Request not found: missing\n"
    );
    if stderr != expected || output.status.code() != Some(5) {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_run_json_lint_summary_uses_warning_key() -> Result<(), Failed> {
    let output = run_lint_warning_fixture(&["-f", "json"])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stderr.lines().next().unwrap_or("");
    let warning: serde_json::Value = serde_json::from_str(first_line)
        .map_err(|e| format!("stderr is not valid JSON: {e}\n{stderr}"))?;
    let message = warning["warning"]["message"].as_str().unwrap_or("");
    if !message.starts_with("1 lint warning found") {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_run_no_lint_skips_lint_summary() -> Result<(), Failed> {
    let output = run_lint_warning_fixture(&["--no-lint"])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr != "Error: Request not found: missing\n" {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_run_lint_summary_quotes_source_with_spaces() -> Result<(), Failed> {
    let dir = std::env::temp_dir().join(format!("rq lint quoted {}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let source = dir.join("my users.rq");
    std::fs::copy(LINT_WARNING_FIXTURE, &source)
        .map_err(|e| format!("Failed to copy fixture: {e}"))?;
    let output = rq_cmd()
        .args(["request", "run", "-s"])
        .arg(&source)
        .args(["-n", "missing"])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"));
    std::fs::remove_dir_all(&dir).ok();
    let output = output?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!("run `rq check -s \"{}\"` for details", source.display());
    if !stderr.contains(&expected) {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_run_file_lint_ignores_sibling_files() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/fixtures/lint_scope/get.rq",
            "-n",
            "missing",
        ])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr != "Error: Request not found: missing\n" {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

#[cfg(unix)]
fn test_request_run_terminates_on_symlink_loop() -> Result<(), Failed> {
    let root = common::symlink_loop_dir("rq_run_symlink_loop")?;
    let mut cmd = rq_cmd();
    cmd.args(["request", "run", "-s"])
        .arg(root.join("sub"))
        .args(["-n", "missing"]);
    let output = common::output_within(cmd, std::time::Duration::from_secs(20));
    std::fs::remove_dir_all(&root).ok();
    let output = output?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() != Some(5)
        || !stderr.starts_with("Warning: 1 lint warning found")
        || !stderr.contains("Request 'missing' not found")
    {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_json_output_ends_with_newline() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/input/foo.rq",
            "-f",
            "json",
        ])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.ends_with("}\n") {
        return Err(format!("Expected trailing newline, got: {stdout:?}").into());
    }
    Ok(())
}

fn test_request_text_output_starts_with_status() -> Result<(), Failed> {
    let output = rq_cmd()
        .args(["request", "run", "-s", "tests/request/run/input/basic.rq"])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let request_line = lines.next().unwrap_or_default();
    let status_line = lines.next().unwrap_or_default();
    if request_line != "basic  GET http://localhost:8080/get"
        || !status_line.starts_with("200 OK · ")
        || !status_line.ends_with(" ms")
    {
        return Err(format!("Unexpected output: {stdout}").into());
    }
    Ok(())
}

fn test_request_legacy_output_flag_points_to_format() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/input/basic.rq",
            "-o",
            "json",
        ])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() || !stderr.contains("-o/--output was renamed to -f/--format") {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn run_basic_with(extra: &[&str]) -> Result<std::process::Output, Failed> {
    rq_cmd()
        .args([
            "request",
            "run",
            "--no-lint",
            "-s",
            "tests/request/run/input/basic.rq",
        ])
        .args(extra)
        .output()
        .map_err(|e| format!("Failed to execute: {e}").into())
}

fn test_request_print_headers_omits_meta_and_body() -> Result<(), Failed> {
    let output = run_basic_with(&["-p", "h"])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success()
        || !stdout.contains("content-type: application/json")
        || stdout.contains("200 OK")
        || stdout.contains('{')
    {
        return Err(format!("Unexpected output: {stdout}").into());
    }
    Ok(())
}

fn test_request_print_body_omits_meta() -> Result<(), Failed> {
    let output = run_basic_with(&["-p", "b"])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !stdout.starts_with('{') || stdout.contains("200 OK") {
        return Err(format!("Unexpected output: {stdout}").into());
    }
    Ok(())
}

fn parse_first_result(output: &std::process::Output) -> Result<serde_json::Value, Failed> {
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Invalid JSON output: {e}"))?;
    Ok(envelope["results"][0].clone())
}

fn test_request_print_filters_json_fields() -> Result<(), Failed> {
    let output = run_basic_with(&["-p", "h", "-f", "json"])?;
    let result = parse_first_result(&output)?;
    if !result["request_name"].is_null()
        || !result["response_headers"].is_object()
        || !result["status"].is_null()
        || !result["body"].is_null()
        || !result["request_headers"].is_null()
    {
        return Err(format!("Unexpected result: {result}").into());
    }
    Ok(())
}

fn test_request_json_default_prints_meta_and_body() -> Result<(), Failed> {
    let output = run_basic_with(&["-f", "json"])?;
    let result = parse_first_result(&output)?;
    if result["request_name"] != "basic"
        || result["status"] != 200
        || !result["body"].is_string()
        || !result["response_headers"].is_null()
        || !result["request_headers"].is_null()
    {
        return Err(format!("Unexpected result: {result}").into());
    }
    Ok(())
}

fn test_request_print_rejects_unknown_part() -> Result<(), Failed> {
    let output = run_basic_with(&["-p", "mx"])?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() || !stderr.contains("Invalid part 'x', expected any of m, h, b") {
        return Err(format!("Unexpected stderr: {stderr}").into());
    }
    Ok(())
}

fn test_request_debug_logs_masked_request_and_response() -> Result<(), Failed> {
    let dir = std::env::temp_dir().join(format!("rq_test_debug_log_{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let file = dir.join("debug.rq");
    fs::write(
        &file,
        "rq get(\"http://localhost:8080/get\", $[\n    \"Authorization\": \"Bearer s3cr3t\"\n]);\n",
    )
    .map_err(|e| format!("Failed to write temp file: {e}"))?;
    let output = rq_cmd()
        .args(["request", "run", "-d", "-s"])
        .arg(&file)
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    fs::remove_dir_all(&dir).ok();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("> GET http://localhost:8080/get\n")
        || !stderr.contains("> Authorization: ***\n")
        || !stderr.contains("< 200 OK (")
        || stderr.contains("s3cr3t")
    {
        return Err(format!("Unexpected debug output: {stderr}").into());
    }
    Ok(())
}

fn debug_stderr(args: &[&str]) -> Result<String, Failed> {
    let output = rq_cmd()
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

fn test_request_debug_header_masks_cli_variables() -> Result<(), Failed> {
    let stderr = debug_stderr(&[
        "request",
        "run",
        "-d",
        "-s",
        "tests/request/run/input/basic.rq",
        "-v",
        "token=s3cr3t",
    ])?;
    if !stderr.contains("* rq ")
        || !stderr.contains("-v token=***")
        || !stderr.contains("* Working directory: ")
        || stderr.contains("s3cr3t")
    {
        return Err(format!("Unexpected debug header: {stderr}").into());
    }
    Ok(())
}

fn test_request_debug_footer_reports_exit_code() -> Result<(), Failed> {
    let stderr = debug_stderr(&[
        "request",
        "run",
        "-d",
        "-s",
        "tests/request/run/input/basic.rq",
        "-e",
        "missing",
    ])?;
    if !stderr.contains("* Finished with exit code 3: Environment not found: missing") {
        return Err(format!("Unexpected debug footer: {stderr}").into());
    }
    Ok(())
}

fn test_request_debug_traces_secret_and_variable_sources() -> Result<(), Failed> {
    let stderr = debug_stderr(&[
        "request",
        "run",
        "-d",
        "-s",
        "tests/request/run/input/environments__env_local__.rq",
        "-e",
        "local",
    ])?;
    if !stderr.contains("* Secrets from tests/request/run/input/.env: env_secret")
        || !stderr.contains("* Variable base_url from env:local")
        || stderr.contains("secret_from_env_file")
    {
        return Err(format!("Unexpected debug trace: {stderr}").into());
    }
    Ok(())
}

fn test_request_secrets() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/secrets/secrets.rq",
            "--environment",
            "local",
        ])
        .env("rq__os_token", "from_env_specific_os")
        .env("rq__secret_value", "from_env_specific_dot_env")
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/secrets/secrets.json"),
    )
    .map_err(Failed::from)
}

fn test_request_secrets_uppercase_prefixes() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/secrets_uppercase_prefixes/test.rq",
            "--environment",
            "local",
        ])
        .env("RQ__ENV__LOCAL__VAR_FROM_OS", "val_os")
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/secrets_uppercase_prefixes/test.json"),
    )
    .map_err(Failed::from)
}

fn test_request_auth_token_backdoor() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/auth_token_backdoor/test.rq",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/auth_token_backdoor/test.json"),
    )
    .map_err(Failed::from)
}

fn test_request_auth_token_backdoor_upper() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/auth_token_backdoor_upper/test.rq",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/auth_token_backdoor_upper/test.json"),
    )
    .map_err(Failed::from)
}

fn test_request_cli_variable_override() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/cli_override/override.rq",
            "-v",
            "color=red",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/cli_override/override.json"),
    )
    .map_err(Failed::from)
}

fn test_request_dotenv() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-f",
            "json",
            "-s",
            "tests/request/run/fixtures/dotenv/dotenv.rq",
            "--environment",
            "local",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_json_response(
        &stdout,
        Path::new("tests/request/run/fixtures/dotenv/dotenv.json"),
    )
    .map_err(Failed::from)
}

fn test_request_run_file_not_found() -> Result<(), Failed> {
    let output = rq_cmd()
        .args(["request", "run", "-s", "non_existent_file"])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Path does not exist") {
        return Err(
            format!("Expected error message about path not existing, got: {stderr}").into(),
        );
    }

    Ok(())
}

fn test_request_run_invalid_request_name() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-n",
            "invalid-name!",
            "-s",
            "tests/request/run/input",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Name must match pattern") {
        return Err(format!("Expected error message about invalid pattern, got: {stderr}").into());
    }

    Ok(())
}

fn test_request_run_invalid_variable_format() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-v",
            "invalid_format",
            "-s",
            "tests/request/run/input",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Variable must be in format NAME=VALUE") {
        return Err(format!("Expected error message about variable format, got: {stderr}").into());
    }

    Ok(())
}

fn test_request_run_invalid_variable_name() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-v",
            "1invalid=value",
            "-s",
            "tests/request/run/input",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Invalid variable name") {
        return Err(
            format!("Expected error message about invalid variable name, got: {stderr}").into(),
        );
    }

    Ok(())
}

fn sent_content_type(source: &str) -> Result<Option<String>, Failed> {
    let output = rq_cmd()
        .args(["request", "run", "-d", "--no-lint", "-s", source])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("Request failed: {stderr}").into());
    }

    Ok(stderr
        .lines()
        .filter_map(|line| line.split_once("] > content-type: "))
        .map(|(_, value)| value.to_string())
        .next())
}

fn test_read_file_body_sends_no_content_type() -> Result<(), Failed> {
    let target = sent_content_type("tests/request/run/input/sys_func/read_file_json_body.rq")?;

    if let Some(content_type) = target {
        return Err(format!(
            "io.read_file() returns a string, so its body must not derive a content type, got {content_type}"
        )
        .into());
    }

    Ok(())
}

fn test_read_json_body_sends_json_content_type() -> Result<(), Failed> {
    let target = sent_content_type("tests/request/run/input/sys_func/read_json_body__code_0__.rq")?;

    match target.as_deref() {
        Some("application/json") => Ok(()),
        other => {
            Err(format!("io.read_json() body must derive application/json, got {other:?}").into())
        }
    }
}

fn test_request_run_ep_dot_notation() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "api.get",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.code() == Some(2) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Validation rejected dot notation name: {stderr}").into());
    }
    if output.status.code() == Some(5) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Request 'api.get' was not found (dot notation not normalized): {stderr}"
        )
        .into());
    }

    Ok(())
}

fn test_request_run_connection_refused() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/fixtures/connection_refused/test.rq",
            "-f",
            "json",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if output.status.success() {
        return Err("Expected command to fail but it succeeded".into());
    }

    if output.status.code() != Some(6) {
        return Err(format!("Expected exit code 6, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim())
        .map_err(|e| format!("stderr is not valid JSON: {e}\nstderr: {stderr}"))?;

    let error = parsed
        .get("error")
        .ok_or("Missing 'error' key in JSON output")?;

    if error.get("type").and_then(|t| t.as_str()) != Some("network") {
        return Err(format!(
            "Expected error.type 'network', got: {:?}",
            error.get("type")
        )
        .into());
    }

    let message = error
        .get("message")
        .and_then(|m| m.as_str())
        .ok_or("Missing error.message in JSON output")?;

    if !message.contains("Connection refused") && !message.contains("connection refused") {
        return Err(format!(
            "Expected error message to contain 'Connection refused', got: {message}"
        )
        .into());
    }

    Ok(())
}

fn test_request_required_variable_satisfied_by_cli() -> Result<(), Failed> {
    let output = rq_cmd()
        .args([
            "request",
            "run",
            "-s",
            "tests/request/run/input/required/missing__code_3__.rq",
            "-v",
            "user_id=42",
        ])
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Expected success when required variable is supplied via CLI, got: {stderr}"
        )
        .into());
    }

    Ok(())
}

// --- Directory Test Discovery ---

fn discover_directory_tests() -> Vec<Trial> {
    let mut trials = Vec::new();

    // Request Run Tests
    trials.extend(discover_tests_in_root(Path::new("tests/request/run/input")));

    trials
}

fn discover_tests_in_root(root_dir: &Path) -> Vec<Trial> {
    let mut trials = Vec::new();
    collect_tests(root_dir, Path::new(""), &mut trials);
    trials
}

fn collect_tests(base_dir: &Path, relative_dir: &Path, trials: &mut Vec<Trial>) {
    let current_dir = base_dir.join(relative_dir);
    if let Ok(entries) = fs::read_dir(&current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap();
                let new_relative = relative_dir.join(dir_name);
                collect_tests(base_dir, &new_relative, trials);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rq") {
                let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let dir_str = relative_dir.to_string_lossy().to_string();

                let test_name = if dir_str.is_empty() {
                    file_stem.clone()
                } else {
                    // Replace path separators with underscores for the test name
                    let safe_dir = dir_str.replace(std::path::MAIN_SEPARATOR, "_");
                    format!("{safe_dir}_{file_stem}")
                };

                let dir_name_owned = dir_str;
                let file_stem_owned = file_stem;

                trials.push(Trial::test(test_name, move || {
                    run_directory_test(&dir_name_owned, &file_stem_owned)
                }));
            }
        }
    }
}

fn run_directory_test(dir_name: &str, file_name: &str) -> Result<(), Failed> {
    let (input_file, expected_file, expected_json) = if dir_name.is_empty() {
        (
            format!("tests/request/run/input/{file_name}.rq"),
            format!("tests/request/run/expected/{file_name}.txt"),
            format!("tests/request/run/expected/{file_name}.json"),
        )
    } else {
        (
            format!("tests/request/run/input/{dir_name}/{file_name}.rq"),
            format!("tests/request/run/expected/{dir_name}/{file_name}.txt"),
            format!("tests/request/run/expected/{dir_name}/{file_name}.json"),
        )
    };

    let expected_code =
        extract_exit_code_from_name(dir_name).or_else(|| extract_exit_code_from_name(file_name));
    let env_name = extract_env_from_name(file_name);
    let request_name = extract_request_from_name(file_name);
    let use_dir_source = file_name.contains("__dir__");

    let mut cmd = rq_cmd();

    if use_dir_source {
        let path = Path::new(&input_file);
        let parent = path.parent().unwrap();
        cmd.args([
            "request",
            "run",
            "--source",
            parent.to_string_lossy().as_ref(), // Fixed clippy: unnecessary to_string
        ]);
    } else {
        cmd.args(["request", "run", "--source", &input_file]);
    }

    cmd.arg("--no-lint");

    if let Some(ref env) = env_name {
        cmd.args(["--environment", env]);
    }

    if let Some(ref req) = request_name {
        cmd.args(["--name", req]);
    }

    if Path::new(&expected_json).exists() {
        cmd.args(["-f", "json"]);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let actual_output = if stderr.is_empty() { &stdout } else { &stderr };

    if let Some(code) = expected_code {
        let actual_code = output.status.code().unwrap_or(-1);
        if actual_code != code {
            return Err(
                format!("Exit code mismatch! Expected: {code}, Actual: {actual_code}").into(),
            );
        }
        println!("✅ Exit code matches expected: {code}");
    }

    let expected_json_path = Path::new(&expected_json);
    if expected_json_path.exists() {
        if !output.status.success() {
            return Err(format!("Command failed: {stderr}").into());
        }
        validate_json_response(&stdout, expected_json_path).map_err(Failed::from)?;
    } else {
        let expected_path = Path::new(&expected_file);
        if expected_path.exists() {
            let expected_content = fs::read_to_string(expected_path)
                .map_err(|e| format!("Failed to read expected file {expected_file}: {e}"))?;

            let actual_trimmed = actual_output.trim();
            let expected_trimmed = expected_content.trim();

            if actual_trimmed != expected_trimmed {
                return Err(format!(
                    "Output mismatch!\nExpected:\n{expected_trimmed}\n\nActual:\n{actual_trimmed}"
                )
                .into());
            }
            println!("✅ Text output matches expected");
        } else {
            // If no expected file and no exit code check, skip
            if expected_code.is_none() {
                println!("⚠️  Skipping test {dir_name}_{file_name}: No expected file found");
                return Ok(());
            }
        }
    }

    Ok(())
}

fn extract_exit_code_from_name(test_name: &str) -> Option<i32> {
    if let Some(pos) = test_name.find("__code_") {
        let after = &test_name[pos + 7..];
        if let Some(end) = after.find("__") {
            if let Ok(code) = after[..end].parse::<i32>() {
                return Some(code);
            }
        }
    }
    None
}

fn extract_env_from_name(test_name: &str) -> Option<String> {
    if let Some(pos) = test_name.find("__env_") {
        let after = &test_name[pos + 6..];
        if let Some(end) = after.find("__") {
            return Some(after[..end].to_string());
        }
        return Some(after.to_string());
    }
    None
}

fn extract_request_from_name(test_name: &str) -> Option<String> {
    if let Some(pos) = test_name.find("__req_") {
        let after = &test_name[pos + 6..];
        if let Some(end) = after.find("__") {
            return Some(after[..end].to_string());
        }
        return Some(after.to_string());
    }
    None
}
