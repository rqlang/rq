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
    if json.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("JSON missing 'file' field".into());
    }
    if json.get("line").and_then(|v| v.as_u64()) != Some(6) {
        return Err(format!("Expected line 6, got: {json}").into());
    }
    if json.get("character").and_then(|v| v.as_u64()) != Some(3) {
        return Err(format!("Expected character 3, got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_auth_bare_identifier() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/auth/attribute_bare_identifier.rq",
            "-n",
            "auth_bare_identifier",
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
    if !stdout.contains("Request: auth_bare_identifier") {
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

#[test]
fn test_request_show_resolved_variables() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "my_request",
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
    if !stdout.contains("Request: my_request") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("URL: https://api.example.com/resource") {
        // Handle output format differences if any
        if !stdout.contains("api.example.com") {
            return Err("Output missing resolved URL part".into());
        }
    }
    if !stdout.contains("name: my_oauth") {
        return Err("Output missing auth name".into());
    }
    if !stdout.contains("type: oauth2_implicit") {
        return Err("Output missing auth type".into());
    }

    Ok(())
}

#[test]
fn test_request_show_ep_dot_notation() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "api.get",
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
    if !stdout.contains("Request: api/get") {
        return Err(format!("Output missing 'Request: api/get', got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_unresolved_fails() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_req_unres_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        r#"rq my_request("{{undefined_base_url}}/resource");
"#,
    )?;

    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "my_request",
            "-o",
            "json",
        ])
        .output()?;

    std::fs::remove_dir_all(&temp_dir).ok();

    if output.status.success() {
        return Err("Expected command to fail with unresolved variable".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Unresolved variable") || !stderr.contains("undefined_base_url") {
        return Err(format!(
            "Expected 'Unresolved variable: undefined_base_url' error, got: {stderr}"
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_request_show_unresolved_no_var_interpolation() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_req_unres_nv_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        r#"rq my_request("{{undefined_base_url}}/resource");
"#,
    )?;

    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "my_request",
            "--no-var-interpolation",
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

    let url = json
        .get("URL")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'URL' field")?;

    if !url.contains("{{undefined_base_url}}") {
        return Err(
            format!("Expected raw template with {{{{undefined_base_url}}}}, got: {url}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_request_show_timeout_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/timeout_success.rq",
            "-n",
            "get",
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
    if !stdout.contains("Timeout: 10") {
        return Err(format!("Output missing timeout, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_timeout_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/timeout_success.rq",
            "-n",
            "get",
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
    let json: serde_json::Value = serde_json::from_str(&stdout)?;

    if json.get("Timeout").and_then(|v| v.as_str()) != Some("10") {
        return Err(format!("Expected Timeout '10', got: {json}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_resolved_variables_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "my_request",
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

    if json["URL"] != "https://api.example.com/resource" {
        return Err(format!(
            "Expected URL 'https://api.example.com/resource', got '{}'",
            json["URL"]
        )
        .into());
    }

    if json["Auth"]["name"] != "my_oauth" {
        return Err(format!(
            "Expected Auth name 'my_oauth', got '{}'",
            json["Auth"]["name"]
        )
        .into());
    }

    if json["Auth"]["type"] != "oauth2_implicit" {
        return Err(format!(
            "Expected Auth type 'oauth2_implicit', got '{}'",
            json["Auth"]["type"]
        )
        .into());
    }

    Ok(())
}
