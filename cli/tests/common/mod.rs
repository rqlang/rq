#![allow(dead_code)]
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Once;

static BUILD: Once = Once::new();

pub fn ensure_built() {
    BUILD.call_once(|| {
        println!("Building project...");
        let build_output = Command::new("cargo")
            .args(["build"])
            .output()
            .expect("Failed to execute cargo build");

        if !build_output.status.success() {
            panic!(
                "Failed to build project: {}",
                String::from_utf8_lossy(&build_output.stderr)
            );
        }
        println!("Project built successfully");
    });
}

pub fn rq_cmd() -> Command {
    ensure_built();
    let mut path = std::env::current_dir().unwrap();
    path.push("target/debug/rq");
    Command::new(path)
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

    let mut actual_jsons = Vec::new();
    let mut current_slice = stdout;

    while let Some(status_pos) = current_slice.find("body:") {
        let after_status = &current_slice[status_pos..];
        if let Some(brace_rel) = after_status.find('{') {
            let slice = &after_status[brace_rel..];

            let mut depth = 0usize;
            let mut in_string = false;
            let mut escape = false;
            let mut end_index: Option<usize> = None;

            for (i, ch) in slice.char_indices() {
                if in_string {
                    if escape {
                        escape = false;
                        continue;
                    }
                    match ch {
                        '\\' => escape = true,
                        '"' => in_string = false,
                        _ => {}
                    }
                    continue;
                }
                match ch {
                    '"' => in_string = true,
                    '{' => depth += 1,
                    '}' => {
                        if depth > 0 {
                            depth -= 1;
                            if depth == 0 {
                                end_index = Some(i + 1);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(end) = end_index {
                let json_body = slice[..end].trim();
                if let Ok(val) = serde_json::from_str::<Value>(json_body) {
                    actual_jsons.push(val);
                }
                // Advance slice past this JSON object
                current_slice = &slice[end..];
            } else {
                break; // Malformed JSON or end of string
            }
        } else {
            break; // No JSON body found
        }
    }

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
