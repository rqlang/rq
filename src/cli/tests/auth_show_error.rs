mod common;
use common::rq_cmd;

#[test]
fn test_auth_show_error_location() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args(["auth", "show", "-n", "test_auth"])
        .current_dir("tests/auth/show/input")
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        return Err(format!(
            "Command succeeded unexpectedly: {}",
            String::from_utf8_lossy(&output.stdout)
        )
        .into());
    }

    if !stderr.contains("Unresolved variable: 'missing_var'") {
        return Err(format!(
            "Expected 'Unresolved variable: 'missing_var'' in stderr, got: {stderr}"
        )
        .into());
    }

    // The error is in .env line 2: my_var={{missing_var}}
    if !stderr.contains(".env") {
        return Err(format!("Expected '.env' in stderr, got: {stderr}").into());
    }
    if !stderr.contains("line 2") {
        return Err(format!("Expected 'line 2' in stderr, got: {stderr}").into());
    }

    Ok(())
}
