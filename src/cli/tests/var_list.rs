mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_var_list_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_list_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("vars.rq"),
        "let base_url = \"http://localhost\";\n",
    )?;

    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            temp_dir.to_str().unwrap(),
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.is_empty() {
        return Err("Expected at least one variable".into());
    }

    if items[0].get("name").and_then(|v| v.as_str()) != Some("base_url") {
        return Err(format!("Expected name 'base_url', got: {}", items[0]).into());
    }
    if items[0].get("source").and_then(|v| v.as_str()) != Some("let") {
        return Err(format!("Expected source 'let', got: {}", items[0]).into());
    }

    Ok(())
}

#[test]
fn test_var_list_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_list_empty_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("api.rq"),
        "ep api(url: \"http://localhost\");\n",
    )?;

    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            temp_dir.to_str().unwrap(),
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if !items.is_empty() {
        return Err(format!("Expected empty array, got {} items", items.len()).into());
    }

    Ok(())
}

#[test]
fn test_var_list_file_includes_imports() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_list_imports_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("shared.rq"),
        "let shared_url = \"http://shared.localhost\";\n",
    )?;
    std::fs::write(temp_dir.join("main.rq"), "import \"shared\";\n")?;

    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            temp_dir.join("main.rq").to_str().unwrap(),
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.is_empty() {
        return Err("Expected variable from imported file".into());
    }
    if items[0].get("name").and_then(|v| v.as_str()) != Some("shared_url") {
        return Err(format!("Expected name 'shared_url', got: {}", items[0]).into());
    }

    Ok(())
}

#[test]
fn test_var_list_file_excludes_unimported() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_list_scope_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("imported.rq"),
        "let imported_url = \"http://imported.localhost\";\n",
    )?;
    std::fs::write(
        temp_dir.join("unrelated.rq"),
        "let unrelated_url = \"http://unrelated.localhost\";\n",
    )?;
    std::fs::write(temp_dir.join("main.rq"), "import \"imported\";\n")?;

    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            temp_dir.join("main.rq").to_str().unwrap(),
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.len() != 1 {
        return Err(format!("Expected 1 variable, got {}: {stdout}", items.len()).into());
    }
    if items[0].get("name").and_then(|v| v.as_str()) != Some("imported_url") {
        return Err(format!("Expected 'imported_url', got: {}", items[0]).into());
    }

    Ok(())
}

#[test]
fn test_var_list_partial_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_list_partial_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("partial.rq"),
        "let base_url = \"http://localhost\";\nlet b =\n",
    )?;

    let output = rq_cmd()
        .args([
            "var",
            "list",
            "-s",
            temp_dir.join("partial.rq").to_str().unwrap(),
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
    let items = json.as_array().ok_or("Expected JSON array")?;

    if items.is_empty() {
        return Err("Expected at least one variable despite syntax error".into());
    }
    if items[0].get("name").and_then(|v| v.as_str()) != Some("base_url") {
        return Err(format!("Expected 'base_url', got: {}", items[0]).into());
    }

    Ok(())
}

#[test]
fn test_var_list_file_not_found() {
    let output = rq_cmd()
        .args(["var", "list", "-s", "non_existent_file"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}
