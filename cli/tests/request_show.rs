mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_request_show_bearer() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/auth/attribute.rq",
            "-n",
            "simple_auth",
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
    if !stdout.contains("Request: simple_auth") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("name: test_auth") {
        return Err("Output missing auth name".into());
    }
    if !stdout.contains("type: bearer") {
        return Err("Output missing auth type".into());
    }

    Ok(())
}

#[test]
fn test_request_show_oauth2() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/auth",
            "-n",
            "test_request_oauth2_fallback",
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
    if !stdout.contains("Request: test_request_oauth2_fallback") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("name: api_oauth") {
        return Err("Output missing auth name".into());
    }
    if !stdout.contains("type: oauth2_authorization_code") {
        return Err("Output missing OAuth2 auth type".into());
    }

    Ok(())
}

#[test]
fn test_request_show_no_auth() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/basic.rq",
            "-n",
            "basic",
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
    if !stdout.contains("Request: basic") {
        return Err("Output missing request name".into());
    }
    if stdout.contains("Auth") {
        return Err("Output should show no auth".into());
    }

    Ok(())
}

#[test]
fn test_request_show_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/auth/attribute.rq",
            "-n",
            "simple_auth",
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

    if json.get("Request").and_then(|v| v.as_str()) != Some("simple_auth") {
        return Err("JSON missing or incorrect 'name' field".into());
    }
    let auth = json.get("Auth").ok_or("JSON missing 'auth' field")?;
    if auth.get("name").and_then(|v| v.as_str()) != Some("test_auth") {
        return Err("JSON auth missing or incorrect 'name' field".into());
    }
    if auth.get("type").and_then(|v| v.as_str()) != Some("bearer") {
        return Err("JSON auth missing or incorrect 'type' field".into());
    }

    Ok(())
}

#[test]
fn test_request_show_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/basic.rq",
            "-n",
            "nonexistent_request",
        ])
        .output()?;

    if output.status.success() {
        return Err("Command should have failed for nonexistent request".into());
    }
    if output.status.code() != Some(5) {
        return Err(format!(
            "Expected exit code 5 (NotFoundError), got: {:?}",
            output.status.code()
        )
        .into());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("not found") {
        return Err("Error message should indicate request not found".into());
    }

    Ok(())
}

#[test]
fn test_request_show_file_not_found() {
    let output = rq_cmd()
        .args(["request", "show", "-s", "non_existent_file", "-n", "req"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}

#[test]
fn test_request_show_invalid_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-n",
            "invalid-name!",
            "-s",
            "tests/request/run/input",
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
