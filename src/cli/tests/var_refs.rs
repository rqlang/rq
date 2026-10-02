mod common;
use common::rq_cmd;
use serde_json::Value;

#[test]
fn test_var_refs_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "base_url",
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
        return Err("Expected at least one reference to 'base_url'".into());
    }

    let first = &items[0];
    if first.get("file").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'file' field in reference".into());
    }
    if first.get("line").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'line' field in reference".into());
    }
    if first.get("column").and_then(|v| v.as_u64()).is_none() {
        return Err("Missing 'column' field in reference".into());
    }

    Ok(())
}

#[test]
fn test_var_refs_text() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "base_url",
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
fn test_var_refs_across_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_var_refs_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("a.rq"),
        "let host = \"example.com\";\nrq get(\"{{host}}/a\");\n",
    )?;
    std::fs::write(temp_dir.join("b.rq"), "rq get(\"{{host}}/b\");\n")?;

    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "host",
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

    if items.len() < 2 {
        return Err(format!(
            "Expected at least 2 references to 'host' across files, got {}",
            items.len()
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_var_refs_no_results() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/fixtures/request_show_vars.rq",
            "-n",
            "nonexistent_var",
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
fn test_var_refs_env_declaration_included() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_refs_env_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(
        temp_dir.join("test.rq"),
        "env local {\n    host: \"localhost\"\n}\n\nrq get(\"{{host}}/path\");\n",
    )?;

    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "host",
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
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    let items = json.as_array().ok_or("Expected JSON array")?;

    let has_env_decl = items.iter().any(|r| r["line"].as_u64() == Some(2));
    if !has_env_decl {
        return Err(
            format!("Expected env block declaration (line 2) in refs, got: {items:?}").into(),
        );
    }

    let has_usage = items.iter().any(|r| r["line"].as_u64() == Some(5));
    if !has_usage {
        return Err(format!("Expected usage (line 5) in refs, got: {items:?}").into());
    }

    Ok(())
}

#[test]
fn test_var_refs_interpolation_column_position() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir =
        std::env::temp_dir().join(format!("rq_test_var_refs_charpos_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    std::fs::write(temp_dir.join("test.rq"), "rq get(\"{{base_url}}/v1\");\n")?;

    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            temp_dir.to_str().unwrap(),
            "-n",
            "base_url",
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
        return Err("Expected at least one reference".into());
    }

    let column = items[0]["column"].as_u64().ok_or("Missing column field")?;
    if column != 11 {
        return Err(format!(
            "Expected column=11 (start of 'base_url'), got column={column}. \
             Hint: column points to '{{{{' instead of the variable name."
        )
        .into());
    }

    Ok(())
}

#[test]
fn test_var_refs_file_not_found() {
    let output = rq_cmd()
        .args(["var", "refs", "-s", "non_existent_file", "-n", "myvar"])
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist"));
}

#[test]
fn test_var_refs_invalid_name() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/fixtures/request_show_vars.rq",
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

#[test]
fn test_var_refs_json_file_is_absolute() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/request/run/input/environments__env_local__.rq",
            "-n",
            "base_url",
            "-o",
            "json",
        ])
        .output()?;

    let json: Value = serde_json::from_slice(&output.stdout)?;
    let file = json[0]["file"].as_str().ok_or("Missing 'file' field")?;
    if !std::path::Path::new(file).is_absolute() {
        return Err(format!("Expected an absolute path, got: {file}").into());
    }

    Ok(())
}

#[test]
fn test_var_refs_text_lists_file_line_column() -> Result<(), Box<dyn std::error::Error>> {
    let output = rq_cmd()
        .args([
            "var",
            "refs",
            "-s",
            "tests/request/run/input/environments__env_local__.rq",
            "-n",
            "base_url",
        ])
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let first = stdout
        .lines()
        .next()
        .ok_or("Expected at least one reference")?;
    if !first.ends_with("environments__env_local__.rq:3:5") {
        return Err(format!("Unexpected reference line: {first}").into());
    }

    Ok(())
}
