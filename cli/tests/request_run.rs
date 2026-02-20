// Integration tests for rq CLI
use libtest_mimic::{run, Arguments, Failed, Trial};
use std::fs;
use std::path::Path;
use std::process::Command;

mod common;
use common::{ensure_built, validate_json_response};

fn main() {
    let args = Arguments::from_args();

    ensure_built();

    let mut trials: Vec<Trial> = vec![
        // Fixture tests (Manual setup)
        Trial::test("request_secrets_env_vars", test_request_secrets),
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
    ];

    // Discover tests from organized directories
    let discovered_tests = discover_directory_tests();
    trials.extend(discovered_tests);

    run(&args, trials).exit();
}

// --- Fixture Tests ---

fn test_request_secrets() -> Result<(), Failed> {
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
        .args([
            "request",
            "run",
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
    let output = Command::new("target/debug/rq")
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
    let output = Command::new("target/debug/rq")
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
    let output = Command::new("target/debug/rq")
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
    let output = Command::new("target/debug/rq")
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

    let mut cmd = Command::new("target/debug/rq");

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

    if let Some(ref env) = env_name {
        cmd.args(["--environment", env]);
    }

    if let Some(ref req) = request_name {
        cmd.args(["--name", req]);
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
