mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_var_show_let_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "base_url",
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
    let json: Value = serde_json::from_str(&stdout)?;

    if json.get("name").and_then(|v| v.as_str()) != Some("base_url") {
        return Err(format!("Expected name 'base_url', got: {json}").into());
    }
    if json.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field".into());
    }
    if json.get("line").and_then(|v| v.as_u64()) != Some(0) {
        return Err(format!("Expected line 0 (first line), got: {json}").into());
    }
    if json.get("source").and_then(|v| v.as_str()) != Some("let") {
        return Err(format!("Expected source 'let', got: {json}").into());
    }
    if json.get("value").and_then(|v| v.as_str()) != Some("\"https://api.example.com\"") {
        return Err(format!("Expected value '\"https://api.example.com\"', got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_var_show_let_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "base_url",
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
    if stdout.is_empty() {
        return Err("Expected non-empty text output".into());
    }

    Ok(())
}

#[test]
fn test_var_show_env_variable_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-s",
            "tests/env/list/input/simple.rq",
            "-n",
            "base_url",
            "-e",
            "local",
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
    let json: Value = serde_json::from_str(&stdout)?;

    if json.get("name").and_then(|v| v.as_str()) != Some("base_url") {
        return Err(format!("Expected name 'base_url', got: {json}").into());
    }
    if json.get("source").and_then(|v| v.as_str()) != Some("env:local") {
        return Err(format!("Expected source 'env:local', got: {json}").into());
    }
    if json.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field".into());
    }
    if json.get("line").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'line' field".into());
    }
    if json.get("value").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'value' field".into());
    }

    Ok(())
}

#[test]
fn test_var_show_env_takes_precedence_over_let() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_prec_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        r#"let host = "file-level";

env dev {
    host: "env-level"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "host",
            "-e",
            "dev",
            "-o",
            "json",
        ])
        .output()?;

    std::fs::remove_dir_all(&temp_dir).ok();

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout)?;

    if json.get("source").and_then(|v| v.as_str()) != Some("env:dev") {
        return Err(format!("Expected env var to take precedence, got: {json}").into());
    }
    if json.get("value").and_then(|v| v.as_str()) != Some("\"env-level\"") {
        return Err(format!("Expected env value, got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_var_list_let_variables() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            "tests/fixtures/request_show_vars.rq",
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
    let json: Value = serde_json::from_str(&stdout)?;
    let items = json.as_array().ok_or("Expected JSON array")?;

    if !items.iter().any(|v| v["name"].as_str() == Some("base_url")) {
        return Err(format!("Expected 'base_url' in list, got: {items:?}").into());
    }
    if !items
        .iter()
        .any(|v| v["name"].as_str() == Some("auth_provider_name"))
    {
        return Err(format!("Expected 'auth_provider_name' in list, got: {items:?}").into());
    }

    Ok(())
}

#[test]
fn test_var_list_with_env() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            "tests/env/list/input/simple.rq",
            "-e",
            "local",
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
    let json: Value = serde_json::from_str(&stdout)?;
    let items = json.as_array().ok_or("Expected JSON array")?;

    if !items.iter().any(|v| v["name"].as_str() == Some("base_url")) {
        return Err(format!("Expected 'base_url' in env list, got: {items:?}").into());
    }

    Ok(())
}

#[test]
fn test_var_show_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "nonexistent_var",
        ])
        .output()?;

    if output.status.success() {
        return Err("Command should have failed for nonexistent variable".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("not found") {
        return Err(format!("Expected 'not found' error, got: {stderr}").into());
    }

    Ok(())
}

#[test]
fn test_var_show_file_not_found() {
    let output = rq_cmd()
        .args(["var", "show", "-s", "non_existent_file", "-n", "myvar"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}

#[test]
fn test_var_show_invalid_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "show",
            "-n",
            "invalid-name!",
            "-s",
            "tests/fixtures/request_show_vars.rq",
        ])
        .output()?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Name must match pattern") {
        return Err(format!("Expected error message about invalid pattern, got: {stderr}").into());
    }

    Ok(())
}
