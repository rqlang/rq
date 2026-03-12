mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_ep_refs_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "refs",
            "-s",
            "tests/request/run/input/endpoint_inheritance",
            "-n",
            "base",
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

    if items.is_empty() {
        return Err("Expected at least one reference to 'base'".into());
    }

    let first = &items[0];
    if first.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field in reference".into());
    }
    if first.get("line").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'line' field in reference".into());
    }
    if first.get("character").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'character' field in reference".into());
    }

    Ok(())
}

#[test]
fn test_ep_refs_multiple_files() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "refs",
            "-s",
            "tests/request/run/input/endpoint_inheritance",
            "-n",
            "base",
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

    if items.len() < 2 {
        return Err(format!(
            "Expected at least 2 references to 'base' across files, got {}",
            items.len()
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_ep_refs_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "refs",
            "-s",
            "tests/request/run/input/endpoint_inheritance",
            "-n",
            "base",
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
fn test_ep_refs_no_results() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "refs",
            "-s",
            "tests/request/run/input/endpoint_inheritance",
            "-n",
            "nonexistent_ep",
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

    if !items.is_empty() {
        return Err(format!("Expected empty array, got {} items", items.len()).into());
    }

    Ok(())
}

#[test]
fn test_ep_refs_file_not_found() {
    let output = rq_cmd()
        .args(["ep", "refs", "-s", "non_existent_file", "-n", "base"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}

#[test]
fn test_ep_refs_invalid_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "ep",
            "refs",
            "-s",
            "tests/request/run/input/endpoint_inheritance",
            "-n",
            "invalid-name!",
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
