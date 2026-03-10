mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_ep_show_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "show",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "api",
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

    if json.get("name").and_then(|v| v.as_str()) != Some("api") {
        return Err(format!("Expected name 'api', got: {json}").into());
    }
    if json.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field".into());
    }
    if json.get("line").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'line' field".into());
    }
    if json.get("character").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'character' field".into());
    }

    Ok(())
}

#[test]
fn test_ep_show_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "show",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "api",
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
fn test_ep_show_directory_search() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "show",
            "-s",
            "tests/request/run/input",
            "-n",
            "api",
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

    if json.get("name").and_then(|v| v.as_str()) != Some("api") {
        return Err(format!("Expected name 'api', got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_ep_show_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "show",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "nonexistent_ep",
        ])
        .output()?;

    if output.status.success() {
        return Err("Command should have failed for nonexistent endpoint".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("not found") {
        return Err(
            format!("Expected error message about endpoint not found, got: {stderr}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_ep_show_file_not_found() {
    let output = rq_cmd()
        .args(["ep", "show", "-s", "non_existent_file", "-n", "api"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}

#[test]
fn test_ep_show_invalid_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "show",
            "-n",
            "invalid-name!",
            "-s",
            "tests/request/run/input/endpoint.rq",
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
