mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_ep_list_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_ep_list_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("base.rq"),
        "ep base(url: \"http://localhost\");\n",
    )?;

    let output = rq_cmd()
        .args(["ep", "list", "-s", temp_dir.to_str().unwrap(), "-o", "json"])
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.is_empty() {
        return Err("Expected at least one endpoint".into());
    }

    let first = &items[0];
    if first.get("name").and_then(|v| v.as_str()) != Some("base") {
        return Err(format!("Expected name 'base', got: {first}").into());
    }
    if first.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field".into());
    }
    if first.get("is_template").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!("Expected is_template=true, got: {first}").into());
    }

    Ok(())
}

#[test]
fn test_ep_list_text() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_ep_list_text_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("base.rq"),
        "ep base(url: \"http://localhost\");\n",
    )?;

    let output = rq_cmd()
        .args(["ep", "list", "-s", temp_dir.to_str().unwrap()])
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
    if stdout.is_empty() {
        return Err("Expected non-empty text output".into());
    }

    Ok(())
}

#[test]
fn test_ep_list_is_template_false_for_body_ep() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_ep_list_body_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("api.rq"),
        "ep api(url: \"http://localhost\") {\n    rq get(\"/\");\n}\n",
    )?;

    let output = rq_cmd()
        .args(["ep", "list", "-s", temp_dir.to_str().unwrap(), "-o", "json"])
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.is_empty() {
        return Err("Expected at least one endpoint".into());
    }

    if items[0].get("is_template").and_then(|v| v.as_bool()) != Some(false) {
        return Err(format!("Expected is_template=false for body endpoint, got: {}", items[0]).into());
    }

    Ok(())
}

#[test]
fn test_ep_list_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_ep_list_empty_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(temp_dir.join("empty.rq"), "let x = \"hello\";\n")?;

    let output = rq_cmd()
        .args(["ep", "list", "-s", temp_dir.to_str().unwrap(), "-o", "json"])
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if !items.is_empty() {
        return Err(format!("Expected empty array, got {} items", items.len()).into());
    }

    Ok(())
}

#[test]
fn test_ep_list_file_not_found() {
    let output = rq_cmd()
        .args(["ep", "list", "-s", "non_existent_file"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}
