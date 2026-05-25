use libtest_mimic::{run, Arguments, Failed, Trial};
use serde_json::Value;
use std::fs;
use std::path::Path;

mod common;
use common::{json_subset, rq_cmd};

fn main() {
    let args = Arguments::from_args();

    let mut trials: Vec<Trial> = vec![
        Trial::test(
            "check_nonexistent_source",
            test_check_nonexistent_source,
        ),
        Trial::test(
            "check_endpoint_shared_url_var_deduped",
            test_check_endpoint_shared_url_var_deduped,
        ),
    ];

    trials.extend(discover_check_tests());
    run(&args, trials).exit();
}

fn test_check_nonexistent_source() -> Result<(), Failed> {
    let output = rq_cmd()
        .args(["check", "-s", "tests/check/input/nonexistent.rq"])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    if output.status.success() {
        return Err("Expected failure for nonexistent source".into());
    }
    Ok(())
}

fn test_check_endpoint_shared_url_var_deduped() -> Result<(), Failed> {
    let input = std::env::temp_dir().join(format!(
        "err_endpoint_shared_url_var_{}.rq",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "ep api(\"http://localhost/{{auth_name}}\") {\n    rq list();\n    rq get(\"/1\");\n    rq create();\n}\n",
    )
    .map_err(|e| format!("Failed to write temp file: {e}"))?;
    let input_str = input.to_string_lossy().to_string();
    let output = rq_cmd()
        .args(["check", "-s", &input_str])
        .output()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    std::fs::remove_file(&input).ok();
    if output.status.success() {
        return Err("Expected exit 1 for unresolved variable".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("stdout is not valid JSON: {e}\n{stdout}"))?;
    let errors = actual["errors"].as_array().ok_or("Missing 'errors' array")?;
    if errors.len() != 1 {
        return Err(format!(
            "Expected 1 error (deduped), got {}: {:?}",
            errors.len(),
            errors
        )
        .into());
    }
    let msg = errors[0]["message"].as_str().unwrap_or("");
    if !msg.contains("auth_name") {
        return Err(format!("Expected error about 'auth_name', got: {msg}").into());
    }
    Ok(())
}

fn discover_check_tests() -> Vec<Trial> {
    let mut trials = Vec::new();
    collect_check_tests(
        Path::new("tests/check/input"),
        Path::new(""),
        &mut trials,
    );
    trials
}

fn collect_check_tests(base: &Path, relative: &Path, trials: &mut Vec<Trial>) {
    let current = base.join(relative);
    if let Ok(entries) = fs::read_dir(&current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap();
                collect_check_tests(base, &relative.join(dir_name), trials);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rq") {
                let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let dir_str = relative.to_string_lossy().to_string();

                let expected_path = if dir_str.is_empty() {
                    format!("tests/check/expected/{file_stem}.json")
                } else {
                    format!("tests/check/expected/{dir_str}/{file_stem}.json")
                };

                if !Path::new(&expected_path).exists() {
                    continue;
                }

                let test_name = if dir_str.is_empty() {
                    file_stem.clone()
                } else {
                    let safe_dir = dir_str.replace(std::path::MAIN_SEPARATOR, "_");
                    format!("{safe_dir}_{file_stem}")
                };

                let dir_str_owned = dir_str;
                let file_stem_owned = file_stem;
                let expected_path_owned = expected_path;

                trials.push(Trial::test(test_name, move || {
                    run_check_test(&dir_str_owned, &file_stem_owned, &expected_path_owned)
                }));
            }
        }
    }
}

fn run_check_test(dir: &str, file: &str, expected_path: &str) -> Result<(), Failed> {
    let input_path = if dir.is_empty() {
        format!("tests/check/input/{file}.rq")
    } else {
        format!("tests/check/input/{dir}/{file}.rq")
    };

    let use_dir = file.contains("__dir__");
    let env_name = extract_env_from_name(file);

    let source = if use_dir {
        Path::new(&input_path)
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string()
    } else {
        input_path
    };

    let mut cmd = rq_cmd();
    cmd.args(["check", "--source", &source]);
    if let Some(ref env) = env_name {
        cmd.args(["--env", env]);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let expected_content = fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read {expected_path}: {e}"))?;
    let expected: Value = serde_json::from_str(&expected_content)
        .map_err(|e| format!("Failed to parse expected JSON in {expected_path}: {e}"))?;
    let actual: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("check output is not valid JSON: {e}\nOutput: {stdout}"))?;

    let expected_errors = expected["errors"].as_array().map(|a| a.len()).unwrap_or(0);
    let expected_code = if expected_errors > 0 { 1 } else { 0 };
    let actual_code = output.status.code().unwrap_or(-1);

    if actual_code != expected_code {
        return Err(format!(
            "Exit code mismatch! Expected: {expected_code}, Actual: {actual_code}\nOutput: {stdout}"
        )
        .into());
    }

    if !json_subset(&expected, &actual) {
        return Err(format!(
            "JSON mismatch!\nExpected subset:\n{}\nActual:\n{}",
            serde_json::to_string_pretty(&expected).unwrap(),
            serde_json::to_string_pretty(&actual).unwrap()
        )
        .into());
    }

    Ok(())
}

fn extract_env_from_name(name: &str) -> Option<String> {
    if let Some(pos) = name.find("__env_") {
        let after = &name[pos + 6..];
        if let Some(end) = after.find("__") {
            return Some(after[..end].to_string());
        }
        return Some(after.to_string());
    }
    None
}
