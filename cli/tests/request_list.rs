mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_request_list_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["request", "list", "-s", "tests/request/run/input"])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !stdout.contains("name: basic") {
        return Err("Output missing expected request 'basic'".into());
    }
    if !stdout.contains("file:") {
        return Err("Output missing 'file:' entries".into());
    }
    if stdout.contains("items:") {
        return Err("Output should not contain 'items:' header".into());
    }

    Ok(())
}

#[test]
fn test_request_list_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "list",
            "-s",
            "tests/request/run/input",
            "--output",
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

    let items = json.as_array().ok_or("JSON missing 'items' array")?;

    if items.is_empty() {
        return Err("Items array is empty".into());
    }

    let first = items.first().ok_or("No items in array")?;
    if first.get("name").and_then(|v| v.as_str()).is_none() {
        return Err("Item missing 'name' field".into());
    }
    if first.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Item missing 'file' field".into());
    }

    Ok(())
}

#[test]
fn test_request_list_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["request", "list", "-s", "tests/request/run/input"])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !stdout.contains("endpoint: api") {
        return Err("Output missing endpoint context 'endpoint: api'".into());
    }
    if !stdout.contains("name: api/get") {
        return Err("Output missing nested request 'api/get'".into());
    }

    Ok(())
}

#[test]
fn test_request_list_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join("rq_test_empty_req");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)?;

    let output = rq_cmd()
        .args(["request", "list", "-s", temp_dir.to_str().unwrap()])
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
    if !stdout.contains("No requests found") && !stdout.contains("Requests found:\n\n") {
        return Err("Output doesn't indicate empty result correctly".into());
    }

    Ok(())
}

#[test]
fn test_request_list_file_not_found() {
    let output = rq_cmd()
        .args(["request", "list", "-s", "non_existent_file"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}
// ...
#[test]
fn test_request_list_invalid_output() {
    let output = rq_cmd()
        .args([
            "request",
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
