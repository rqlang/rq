mod common;
use common::rq_cmd;
use serde_json::Value;
use std::fs;

#[test]
fn test_auth_show_bearer_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "bearer_auth",
            "-s",
            "tests/request/run/input",
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

    if !stdout.contains("Auth Configuration: bearer_auth") {
        return Err(
            format!("Expected 'Auth Configuration: bearer_auth' in output, got: {stdout}").into(),
        );
    }

    if !stdout.contains("Type: bearer") {
        return Err(format!("Expected 'Type: bearer' in output, got: {stdout}").into());
    }

    if !stdout.contains("token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9") {
        return Err(format!("Expected token field in output, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_bearer_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "bearer_auth",
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

    if json.get("Auth Configuration").and_then(|v| v.as_str()) != Some("bearer_auth") {
        return Err(format!("Expected name 'bearer_auth', got: {json}").into());
    }

    if json.get("Type").and_then(|v| v.as_str()) != Some("bearer") {
        return Err(format!("Expected auth_type 'bearer', got: {json}").into());
    }

    let fields = json
        .get("Fields")
        .ok_or_else(|| format!("Expected 'fields' in JSON, got: {json}"))?;

    if fields.get("token").and_then(|v| v.as_str()) != Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
    {
        return Err(format!("Expected token field in fields, got: {fields}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_oauth2_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "github_oauth",
            "-s",
            "tests/request/run/input",
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

    if !stdout.contains("Auth Configuration: github_oauth") {
        return Err(format!(
            "Expected 'Auth Configuration: github_oauth' in output, got: {stdout}"
        )
        .into());
    }

    if !stdout.contains("Type: oauth2_authorization_code") {
        return Err(
            format!("Expected 'Type: oauth2_authorization_code' in output, got: {stdout}").into(),
        );
    }

    if !stdout.contains("client_id: my-github-client-id") {
        return Err(format!("Expected client_id field in output, got: {stdout}").into());
    }

    if !stdout.contains("authorization_url: https://github.com/login/oauth/authorize") {
        return Err(format!("Expected authorization_url field in output, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_oauth2_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "github_oauth",
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

    if json.get("Auth Configuration").and_then(|v| v.as_str()) != Some("github_oauth") {
        return Err(format!("Expected name 'github_oauth', got: {json}").into());
    }

    if json.get("Type").and_then(|v| v.as_str()) != Some("oauth2_authorization_code") {
        return Err(format!("Expected auth_type 'oauth2_authorization_code', got: {json}").into());
    }

    let fields = json
        .get("Fields")
        .ok_or_else(|| format!("Expected 'fields' in JSON, got: {json}"))?;

    if fields.get("client_id").and_then(|v| v.as_str()) != Some("my-github-client-id") {
        return Err(format!("Expected client_id in fields, got: {fields}").into());
    }

    if fields.get("authorization_url").and_then(|v| v.as_str())
        != Some("https://github.com/login/oauth/authorize")
    {
        return Err(format!("Expected authorization_url in fields, got: {fields}").into());
    }

    if fields.get("token_url").and_then(|v| v.as_str())
        != Some("https://github.com/login/oauth/access_token")
    {
        return Err(format!("Expected token_url in fields, got: {fields}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_with_env() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "local_auth",
            "-s",
            "tests/request/run/input",
            "-e",
            "local",
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

    if !stdout.contains("Auth Configuration: local_auth") {
        return Err(
            format!("Expected 'Auth Configuration: local_auth' in output, got: {stdout}").into(),
        );
    }

    if !stdout.contains("Environment: local") {
        return Err(format!("Expected 'Environment: local' in output, got: {stdout}").into());
    }

    if !stdout.contains("token: local-token-123") {
        return Err(format!("Expected 'token: local-token-123' in output, got: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "nonexistent_auth",
            "-s",
            "tests/request/run/input",
        ])
        .output()?;

    if output.status.success() {
        return Err("Command should have failed for nonexistent auth".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("not found") {
        return Err(format!("Expected error message about auth not found, got: {stderr}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_variable_interpolation_bearer() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_bearer_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
env local {
    token_value: "local-token-123"
}

env dev {
    token_value: "dev-token-456"
}

auth test_bearer(auth_type.bearer) {
    token: "{{token_value}}"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "test_bearer",
            "-s",
            temp_dir.to_str().unwrap(),
            "-e",
            "local",
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

    let token = json
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|t| t.as_str());

    if token != Some("local-token-123") {
        return Err(format!("Expected token 'local-token-123', got: {token:?}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_variable_interpolation_oauth2() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_oauth2_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
env localhost {
    client_id: "rq-test",
    client_secret: "YwaKzwH86RsE8Mwvrq63umcS70IgU5P1",
    auth_url: "https://auth.example.com/authorize",
    token_url: "https://auth.example.com/token"
}

auth local(auth_type.oauth2_authorization_code) {
    client_id: "{{client_id}}",
    client_secret: "{{client_secret}}",
    authorization_url: "{{auth_url}}",
    token_url: "{{token_url}}",
    redirect_uri: "http://localhost:3000/callback",
    scope: "openid profile email",
    code_challenge_method: "S256"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "local",
            "-s",
            temp_dir.to_str().unwrap(),
            "-e",
            "localhost",
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

    let fields = json
        .get("Fields")
        .ok_or_else(|| format!("Expected 'fields' in JSON, got: {json}"))?;

    if fields.get("client_id").and_then(|v| v.as_str()) != Some("rq-test") {
        return Err(format!(
            "Expected client_id 'rq-test', got: {:?}",
            fields.get("client_id")
        )
        .into());
    }

    if fields.get("client_secret").and_then(|v| v.as_str())
        != Some("YwaKzwH86RsE8Mwvrq63umcS70IgU5P1")
    {
        return Err(format!(
            "Expected client_secret 'YwaKzwH86RsE8Mwvrq63umcS70IgU5P1', got: {:?}",
            fields.get("client_secret")
        )
        .into());
    }

    if fields.get("authorization_url").and_then(|v| v.as_str())
        != Some("https://auth.example.com/authorize")
    {
        return Err(format!(
            "Expected authorization_url 'https://auth.example.com/authorize', got: {:?}",
            fields.get("authorization_url")
        )
        .into());
    }

    if fields.get("token_url").and_then(|v| v.as_str()) != Some("https://auth.example.com/token") {
        return Err(format!(
            "Expected token_url 'https://auth.example.com/token', got: {:?}",
            fields.get("token_url")
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_show_variable_interpolation_multiple_envs() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_multi_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
env local {
    token_value: "local-token-123"
}

env dev {
    token_value: "dev-token-456"
}

auth test_bearer(auth_type.bearer) {
    token: "{{token_value}}"
}
"#,
    )?;

    // Test with local environment
    let output_local = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "test_bearer",
            "-s",
            temp_dir.to_str().unwrap(),
            "-e",
            "local",
            "-o",
            "json",
        ])
        .output()?;

    let stdout_local = String::from_utf8_lossy(&output_local.stdout);
    let json_local: Value = serde_json::from_str(&stdout_local)?;

    let token_local = json_local
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|t| t.as_str());

    if token_local != Some("local-token-123") {
        return Err(format!("Expected local token 'local-token-123', got: {token_local:?}").into());
    }

    // Test with dev environment
    let output_dev = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "test_bearer",
            "-s",
            temp_dir.to_str().unwrap(),
            "-e",
            "dev",
            "-o",
            "json",
        ])
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    let stdout_dev = String::from_utf8_lossy(&output_dev.stdout);
    let json_dev: Value = serde_json::from_str(&stdout_dev)?;

    let token_dev = json_dev
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|t| t.as_str());

    if token_dev != Some("dev-token-456") {
        return Err(format!("Expected dev token 'dev-token-456', got: {token_dev:?}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_variable_interpolation_no_env() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_noenv_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
env local {
    token_value: "local-token-123"
}

auth test_bearer(auth_type.bearer) {
    token: "{{token_value}}"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "test_bearer",
            "-s",
            temp_dir.to_str().unwrap(),
            "-o",
            "json",
        ])
        .output()?;

    fs::remove_dir_all(&temp_dir).ok();

    // With strict variable resolution, this should fail because token_value is not resolved
    if output.status.success() {
        return Err("Command should have failed due to unresolved variable".into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Unresolved variable: 'token_value'") {
        return Err(format!(
            "Expected error message about unresolved variable 'token_value', got: {stderr}"
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_show_oauth2_defaults_applied() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_defaults_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
auth minimal_oauth(auth_type.oauth2_authorization_code) {
    client_id: "test-client-id",
    authorization_url: "https://auth.example.com/authorize",
    token_url: "https://auth.example.com/token"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "minimal_oauth",
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

    let fields = json
        .get("Fields")
        .ok_or_else(|| format!("Expected 'fields' in JSON, got: {json}"))?;

    // Check that defaults were applied
    if fields.get("redirect_uri").and_then(|v| v.as_str())
        != Some("vscode://rq.rq-language/oauth-callback")
    {
        return Err(format!(
            "Expected default redirect_uri 'vscode://rq.rq-language/oauth-callback', got: {:?}",
            fields.get("redirect_uri")
        )
        .into());
    }

    if fields.get("code_challenge_method").and_then(|v| v.as_str()) != Some("S256") {
        return Err(format!(
            "Expected default code_challenge_method 'S256', got: {:?}",
            fields.get("code_challenge_method")
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_show_oauth2_defaults_override() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_override_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    fs::write(
        temp_dir.join("test.rq"),
        r#"
auth custom_oauth(auth_type.oauth2_authorization_code) {
    client_id: "test-client-id",
    authorization_url: "https://auth.example.com/authorize",
    token_url: "https://auth.example.com/token",
    redirect_uri: "http://localhost:8080/custom",
    code_challenge_method: "plain"
}
"#,
    )?;

    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "custom_oauth",
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

    let fields = json
        .get("Fields")
        .ok_or_else(|| format!("Expected 'fields' in JSON, got: {json}"))?;

    // Check that custom values were preserved
    if fields.get("redirect_uri").and_then(|v| v.as_str()) != Some("http://localhost:8080/custom") {
        return Err(format!(
            "Expected custom redirect_uri 'http://localhost:8080/custom', got: {:?}",
            fields.get("redirect_uri")
        )
        .into());
    }

    if fields.get("code_challenge_method").and_then(|v| v.as_str()) != Some("plain") {
        return Err(format!(
            "Expected custom code_challenge_method 'plain', got: {:?}",
            fields.get("code_challenge_method")
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_show_interpolation_from_let() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-s",
            "tests/request/run/input",
            "-n",
            "test_auth_let_interpolation",
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

    let token = json
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Expected 'token' field in JSON, got: {json}"))?;

    if token != "token_from_let" {
        return Err(
            format!("Expected token='token_from_let' from let variable, got: {token}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_auth_show_interpolation_from_env_file() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-s",
            "tests/request/run/input",
            "-n",
            "test_auth_env_interpolation",
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

    let token = json
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Expected 'token' field in JSON, got: {json}"))?;

    if token != "secret_from_env_file" {
        return Err(
            format!("Expected token='secret_from_env_file' from .env file, got: {token}").into(),
        );
    }

    Ok(())
}

#[test]
fn test_auth_show_interpolation_combined() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-s",
            "tests/request/run/input",
            "-n",
            "test_auth_combined_interpolation",
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

    let token = json
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Expected 'token' field in JSON, got: {json}"))?;

    if token != "token_from_let-secret_from_env_file" {
        return Err(format!(
            "Expected token='token_from_let-secret_from_env_file' from combined sources, got: {token}"
        ).into());
    }

    Ok(())
}

#[test]
fn test_auth_show_bare_identifier_reference() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-s",
            "tests/request/run/input",
            "-n",
            "test_bare_reference",
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

    let token = json
        .get("Fields")
        .and_then(|f| f.get("token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Expected 'token' field in JSON, got: {json}"))?;

    if token != "test-client-123" {
        return Err(format!(
            "Expected token='test-client-123' from bare identifier reference, got: {token}"
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_auth_show_invalid_name_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
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
fn test_auth_show_name_too_long() -> Result<(), Box<dyn std::error::Error>> {
    let long_name = "a".repeat(51);
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            &long_name,
            "-s",
            "tests/request/run/input",
        ])
        .output()?;

    if output.status.code() != Some(2) {
        return Err(format!("Expected exit code 2, got: {:?}", output.status.code()).into());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("Name must be 50 characters or less") {
        return Err(format!("Expected error message about length, got: {stderr}").into());
    }

    Ok(())
}

#[test]
fn test_auth_show_env_invalid_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "valid_name",
            "-e",
            "invalid-env!",
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
fn test_error_line_number_bare_variable() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "auth",
            "show",
            "-n",
            "keycloak",
            "-s",
            "tests/auth/show/input",
            "-o",
            "json",
        ])
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        return Err("Command should have failed".into());
    }

    // In error_line_number_bare_var.rq:
    // Line 1: auth keycloak...
    // Line 2:     client_id: client_id,

    // We expect the error to point to line 2 where client_id is used as a value.
    if !stderr.contains("line 2") {
        return Err(format!("Expected error at line 2, got: {stderr}").into());
    }

    // We expect column 16 (value), not column 5 (key)
    if !stderr.contains("column 16") {
        return Err(format!("Expected error at column 16, got: {stderr}").into());
    }

    Ok(())
}
