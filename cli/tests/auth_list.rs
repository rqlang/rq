mod common;
use common::rq_cmd;
use serde_json::Value;
use std::fs;

#[test]
fn test_auth_list_text_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["auth", "list", "-s", "tests/request/run/input"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    if !stdout.contains("bearer_auth") {
        return Err(format!("Expected 'bearer_auth' in output, got: {stdout}").into());
    }

    if !stdout.contains("github_oauth") {
        return Err(format!("Expected 'github_oauth' in output, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_auth_list_json_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "list",
            "-s",
            "tests/request/run/input",
            "-o",
            "json",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let json: Value = serde_json::from_str(&stdout)?;

    if !json.is_array() {
        return Err(format!("Expected JSON array, got: {json}").into());
    }

    let items_array = json.as_array().unwrap();
    if !items_array
        .iter()
        .any(|v| v["name"].as_str() == Some("bearer_auth"))
    {
        return Err(format!("Expected 'bearer_auth' in items array, got: {items_array:?}").into());
    }

    Ok(())
}

#[test]
fn test_auth_list_json_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "list",
            "-s",
            "tests/request/run/input",
            "--output",
            "JSON",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let json: Value = serde_json::from_str(&stdout)?;

    if !json.is_array() {
        return Err(format!("Expected JSON array, got: {json}").into());
    }
    
    let items_array = json.as_array().unwrap();
    if !items_array
        .iter()
        .any(|v| v["name"].as_str() == Some("bearer_auth"))
    {
        return Err(format!("Expected 'bearer_auth' in items array, got: {items_array:?}").into());
    }
    
    Ok(())
}

#[test]
fn test_auth_list_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_empty_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    let output = rq_cmd()
        .args(["auth", "list", "-s", temp_dir.to_str().unwrap()])
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    if !stdout.contains("No auth configurations found") {
        return Err(
            format!("Expected 'No auth configurations found' in output, got: {stdout}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_auth_list_nonexistent_directory() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "list",
            "-s",
            "/nonexistent/path/that/does/not/exist",
        ])
        .output()?;

    if output.status.success() {
        return Err("Command should have failed for nonexistent directory".into());
    }

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

#[test]
fn test_auth_list_default_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_default_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
auth default_auth(auth_type.bearer) {
    token: "token123"
}
rq test("http://localhost:8080/test");
"#,
    )?;

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let binary_path = format!("{manifest_dir}/target/debug/rq");
    common::ensure_built();

    let output = std::process::Command::new(&binary_path)
        .args(["auth", "list"])
        .current_dir(&temp_dir)
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    if !stdout.contains("default_auth") {
        return Err(format!("Expected 'default_auth' in output, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_auth_list_multiple_auth_configs() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_multiple_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
auth auth_a(auth_type.bearer) {
    token: "token_a"
}
auth auth_b(auth_type.bearer) {
    token: "token_b"
}
auth auth_c(auth_type.oauth2_authorization_code) {
    client_id: "client123",
    authorization_url: "https://auth.example.com/authorize",
    token_url: "https://auth.example.com/token"
}
rq test("http://localhost:8080/test");
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "list",
            "-s",
            temp_dir.to_str().unwrap(),
            "-o",
            "json",
        ])
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let json: Value = serde_json::from_str(&stdout)?;

    let items = json.as_array().expect("Expected JSON array");

    if items.len() != 3 {
        return Err(format!("Expected 3 auth configs, got: {}", items.len()).into());
    }

    if items[0]["name"] != "auth_a" || items[1]["name"] != "auth_b" || items[2]["name"] != "auth_c" {
        return Err(format!(
            "Expected alphabetical order [auth_a, auth_b, auth_c], got: {items:?}"
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_list_recursive_search() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_recursive_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    let subdir = temp_dir.join("subdir");
    fs::create_dir(&subdir)?;

    fs::write(
        temp_dir.join("root.rq"),
        r#"
auth root_auth(auth_type.bearer) {
    token: "token_root"
}
rq test("http://localhost:8080/test");
"#,
    )?;

    fs::write(
        subdir.join("sub.rq"),
        r#"
auth sub_auth(auth_type.bearer) {
    token: "token_sub"
}
rq test("http://localhost:8080/test");
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "list",
            "-s",
            temp_dir.to_str().unwrap(),
            "-o",
            "json",
        ])
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let json: Value = serde_json::from_str(&stdout)?;

    let items = json.as_array().expect("Expected JSON array");

    if items.len() != 2 {
        return Err(format!("Expected 2 auth configs, got: {}", items.len()).into());
    }

    let has_root = items.iter().any(|v| v["name"].as_str() == Some("root_auth"));
    let has_sub = items.iter().any(|v| v["name"].as_str() == Some("sub_auth"));

    if !has_root || !has_sub {
        return Err(format!("Expected both root_auth and sub_auth, got: {items:?}").into());
    }

    Ok(())
}

#[test]
fn test_auth_list_invalid_output() {
    let output = rq_cmd()
        .args([
            "auth",
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
