#![allow(dead_code)]
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub fn rq_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rq"))
}

pub fn output_within(mut cmd: Command, timeout: Duration) -> Result<Output, String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute: {e}"))?;
    let started = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if started.elapsed() > timeout {
            child.kill().ok();
            return Err(format!("Command did not finish within {timeout:?}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().map_err(|e| e.to_string())
}

#[cfg(unix)]
pub fn symlink_loop_dir(name: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
    let dir = root.join("sub");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(
        dir.join("users.rq"),
        "ep users(\"http://localhost:8080/users\") {\n    rq list(\"\");\n}\n",
    )
    .map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(".", dir.join("self")).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink("..", dir.join("up")).map_err(|e| e.to_string())?;
    Ok(root)
}

pub fn json_subset(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(exp_map), Value::Object(act_map)) => {
            for (k, v) in exp_map {
                if let Some(act_v) = act_map.get(k) {
                    if !json_subset(v, act_v) {
                        return false;
                    }
                } else {
                    return false; // Key missing in actual
                }
            }
            true
        }
        (Value::Array(exp_arr), Value::Array(act_arr)) => {
            if exp_arr.len() != act_arr.len() {
                return false;
            }
            for (e, a) in exp_arr.iter().zip(act_arr.iter()) {
                if !json_subset(e, a) {
                    return false;
                }
            }
            true
        }
        (Value::String(s), _) if s == "{{*}}" => true,
        (Value::String(s), Value::String(a)) if s.starts_with("{{regex:") && s.ends_with("}}") => {
            let pattern = &s[8..s.len() - 2];
            if let Ok(re) = regex::Regex::new(pattern) {
                re.is_match(a)
            } else {
                false
            }
        }
        _ => expected == actual,
    }
}

pub fn validate_json_response(stdout: &str, expected_path: &Path) -> Result<(), String> {
    let expected_content = fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read expected file: {e}"))?;
    let expected_json: Value = serde_json::from_str(&expected_content)
        .map_err(|e| format!("Failed to parse expected JSON: {e}"))?;
    let envelope: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Failed to parse run output as JSON: {e}\n{stdout}"))?;

    let actual_jsons: Vec<Value> = envelope["results"]
        .as_array()
        .ok_or_else(|| format!("Missing 'results' array in run output: {stdout}"))?
        .iter()
        .filter_map(|result| result["body"].as_str())
        .filter_map(|body| serde_json::from_str::<Value>(body).ok())
        .collect();

    if actual_jsons.is_empty() {
        return Err("No JSON response found in output".to_string());
    }

    let actual_json = if actual_jsons.len() == 1 {
        actual_jsons[0].clone()
    } else {
        Value::Array(actual_jsons)
    };

    if !json_subset(&expected_json, &actual_json) {
        return Err(format!(
            "JSON mismatch!\nExpected subset:\n{}\nActual:\n{}",
            serde_json::to_string_pretty(&expected_json).unwrap(),
            serde_json::to_string_pretty(&actual_json).unwrap()
        ));
    }

    Ok(())
}

pub fn validate_pure_json_response(stdout: &str, expected_path: &Path) -> Result<(), String> {
    let expected_content = fs::read_to_string(expected_path)
        .map_err(|e| format!("Failed to read expected file: {e}"))?;
    let expected_json: Value = serde_json::from_str(&expected_content)
        .map_err(|e| format!("Failed to parse expected JSON: {e}"))?;
    let actual_json: Value = serde_json::from_str(stdout)
        .map_err(|e| format!("Failed to parse actual JSON response: {e}"))?;

    if !json_subset(&expected_json, &actual_json) {
        return Err(format!(
            "JSON mismatch!\nExpected subset:\n{}\nActual:\n{}",
            serde_json::to_string_pretty(&expected_json).unwrap(),
            serde_json::to_string_pretty(&actual_json).unwrap()
        ));
    }

    Ok(())
}
