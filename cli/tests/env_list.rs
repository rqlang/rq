mod common;
use common::{rq_cmd, validate_pure_json_response};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn test_env_list_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join("rq_test_empty_env");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)?;

    let output = rq_cmd()
        .args(["env", "list", "-s", temp_dir.to_str().unwrap()])
        .output()?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("No environments found") {
        return Err(format!("Expected 'No environments found', got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_env_list_nonexistent_directory() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["env", "list", "-s", "tests/nonexistent_dir_12345"])
        .output()?;

    if output.status.success() {
        return Err("Expected command to fail for nonexistent directory".into());
    }
    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Path does not exist") {
        return Err(
            format!("Expected error message about nonexistent directory, got: {stderr}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_env_list_default_directory() -> Result<(), Box<dyn std::error::Error>> {
    // Change to tests/request/run/input directory and run without -s flag
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let binary_path = format!("{manifest_dir}/target/debug/rq");
    common::ensure_built();

    let output = std::process::Command::new(&binary_path)
        .args(["env", "list"])
        .current_dir("tests/request/run/input")
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("local") {
        return Err(format!(
            "Expected 'local' environment in default directory output, got: {stdout}"
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_env_list_recursive_search() -> Result<(), Box<dyn std::error::Error>> {
    let nested_dir = Path::new("tests/temp_nested/subdir");
    if nested_dir.parent().unwrap().exists() {
        std::fs::remove_dir_all(nested_dir.parent().unwrap()).ok();
    }
    std::fs::create_dir_all(nested_dir)?;

    let nested_file = nested_dir.join("nested.rq");
    let nested_content = r#"
env nested_env { value: "nested" }
rq test("http://localhost/test");
"#;
    fs::write(&nested_file, nested_content)?;

    let output = rq_cmd()
        .args(["env", "list", "-s", "tests/temp_nested", "-o", "json"])
        .output()?;

    let _ = std::fs::remove_dir_all("tests/temp_nested");

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout)?;
    let items = json.as_array().ok_or("Expected items array in JSON")?;

    if !items.iter().any(|v| v.as_str() == Some("nested_env")) {
        return Err(format!("Expected 'nested_env' to be found, got: {items:?}").into());
    }

    Ok(())
}

#[test]
fn test_env_list_json_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "env",
            "list",
            "-s",
            "tests/env/list/input/simple.rq",
            "--output",
            "JSON",
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout)?;
    if !json.is_array() {
        return Err(format!("Expected JSON format even with uppercase 'JSON', got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_env_list_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "env",
            "list",
            "-s",
            "tests/env/list/input/multiple.rq",
            "-o",
            "json",
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_pure_json_response(&stdout, Path::new("tests/env/list/expected/multiple.json"))?;

    Ok(())
}

#[test]
fn test_env_list_invalid_output() {
    let output = rq_cmd()
        .args([
            "env",
            "list",
            "-s",
            "tests/request/run/input",
            "--output",
            "invalid",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'invalid'"));
    assert!(stderr.contains("--output <OUTPUT>"));
    assert!(stderr.contains("[possible values: text, json]"));
}
