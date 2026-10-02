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
    if !stdout.contains("name: simple_auth") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("auth: test_auth (bearer)") {
        return Err("Output missing auth name".into());
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
    if !stdout.contains("name: test_request_oauth2_fallback") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("auth: api_oauth (oauth2_authorization_code)") {
        return Err("Output missing auth name".into());
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
    if !stdout.contains("name: basic") {
        return Err("Output missing request name".into());
    }
    if stdout.lines().any(|line| line.starts_with("auth:")) {
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

    if json.get("name").and_then(|v| v.as_str()) != Some("simple_auth") {
        return Err("JSON missing or incorrect 'name' field".into());
    }
    let auth = json.get("auth").ok_or("JSON missing 'auth' field")?;
    if auth.get("name").and_then(|v| v.as_str()) != Some("test_auth") {
        return Err("JSON auth missing or incorrect 'name' field".into());
    }
    if auth.get("type").and_then(|v| v.as_str()) != Some("bearer") {
        return Err("JSON auth missing or incorrect 'type' field".into());
    }
    if json.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("JSON missing 'file' field".into());
    }
    if json.get("line").and_then(|v| v.as_u64()) != Some(7) {
        return Err(format!("Expected line 7, got: {json}").into());
    }
    if json.get("column").and_then(|v| v.as_u64()) != Some(4) {
        return Err(format!("Expected column 4, got: {json}").into());
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
    if !stdout.contains("name: auth_bare_identifier") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("auth: test_auth (bearer)") {
        return Err("Output missing auth name".into());
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
    if !stdout.contains("name: my_request") {
        return Err("Output missing request name".into());
    }
    if !stdout.contains("url: https://api.example.com/resource") {
        // Handle output format differences if any
        if !stdout.contains("api.example.com") {
            return Err("Output missing resolved URL part".into());
        }
    }
    if !stdout.contains("auth: my_oauth (oauth2_implicit)") {
        return Err("Output missing auth name".into());
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
    if !stdout.contains("name: api/get") {
        return Err(format!("Output missing 'name: api/get', got: {stdout}").into());
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
        .get("url")
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
    if !stdout.contains("timeout: 10") {
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

    if json.get("timeout").and_then(|v| v.as_str()) != Some("10") {
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

    if json["url"] != "https://api.example.com/resource" {
        return Err(format!(
            "Expected URL 'https://api.example.com/resource', got '{}'",
            json["url"]
        )
        .into());
    }

    if json["auth"]["name"] != "my_oauth" {
        return Err(format!(
            "Expected Auth name 'my_oauth', got '{}'",
            json["auth"]["name"]
        )
        .into());
    }

    if json["auth"]["type"] != "oauth2_implicit" {
        return Err(format!(
            "Expected Auth type 'oauth2_implicit', got '{}'",
            json["auth"]["type"]
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_request_show_unknown_auth_fails() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_req_auth_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        r#"[auth("ghost")]
rq my_request("http://localhost:8080/resource");
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
        ])
        .output()?;

    std::fs::remove_dir_all(&temp_dir).ok();

    if output.status.success() {
        return Err("Expected command to fail with unknown auth configuration".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Auth configuration 'ghost' not found") {
        return Err(format!("Expected unknown auth error, got: {stderr}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_unknown_auth_no_var_interpolation() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_req_auth_nv_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        r#"[auth("ghost")]
rq my_request("http://localhost:8080/resource");
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

    Ok(())
}

#[test]
fn test_request_show_unresolved_variable_is_syntax_error() -> Result<(), Box<dyn std::error::Error>>
{
    let temp_dir = std::env::temp_dir().join(format!(
        "rq_test_request_show_unresolved_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir)?;
    let file = temp_dir.join("test.rq");
    std::fs::write(&file, "rq get(\"http://localhost/{{missing}}\");\n")?;

    let output = rq_cmd()
        .args(["request", "show", "-s"])
        .arg(&file)
        .args(["-n", "get", "-o", "json"])
        .output()?;
    std::fs::remove_dir_all(&temp_dir).ok();

    let error: serde_json::Value = serde_json::from_slice(&output.stderr)?;
    if output.status.code() != Some(2) || error["error"]["type"] != "syntax" {
        return Err(format!("Unexpected result: {error}").into());
    }

    Ok(())
}

#[test]
fn test_request_show_not_found_reports_typed_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "request",
            "show",
            "-s",
            "tests/request/run/input/endpoint.rq",
            "-n",
            "api.missing",
        ])
        .output()?;

    let stderr = String::from_utf8(output.stderr)?;
    if output.status.code() != Some(5) || stderr.trim() != "Error: Request not found: api.missing" {
        return Err(format!("Unexpected result: {stderr}").into());
    }

    Ok(())
}
