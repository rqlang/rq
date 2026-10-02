#![allow(dead_code)]
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn rq_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rq"))
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
