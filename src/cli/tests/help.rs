mod common;
use common::rq_cmd;
use std::fs;
use std::path::PathBuf;

fn verify_help(args: &[&str], expected_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    verify_help_in_dir(args, expected_file, ".")
}

fn verify_help_in_dir(
    args: &[&str],
    expected_file: &str,
    dir: impl AsRef<std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing help for args: {args:?}");
    let rq_output = rq_cmd().args(args).current_dir(dir).output()?;
    let stdout = String::from_utf8_lossy(&rq_output.stdout);
    let stderr = String::from_utf8_lossy(&rq_output.stderr);

    if !rq_output.status.success() {
        return Err(format!(
            "Command failed for args {args:?}. stderr: {stderr}, stdout: {stdout}"
        )
        .into());
    }

    let mut expected_path = PathBuf::from("tests/help/expected");
    expected_path.push(expected_file);
    let expected_content = fs::read_to_string(&expected_path)
        .map_err(|e| format!("Failed to read expected file {expected_path:?}: {e}"))?;

    // Trim whitespace to avoid issues with newlines
    let stdout_trimmed = stdout.trim();
    let expected_trimmed = expected_content.trim();

    if stdout_trimmed == expected_trimmed {
        Ok(())
    } else {
        Err(format!(
            "Unexpected output for args {args:?}.\nExpected:\n---\n{expected_trimmed}\n---\nActual:\n---\n{stdout_trimmed}\n---"
        )
        .into())
    }
}

#[test]
fn test_help_root() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["help"], "root.txt")?;
    verify_help(&["--help"], "root.txt")
}

#[test]
fn test_env_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["env", "--help"], "env.txt")?;
    verify_help(&["env", "help"], "env.txt")
}

#[test]
fn test_env_list_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["env", "list", "--help"], "env_list.txt")
}

#[test]
fn test_auth_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["auth", "--help"], "auth.txt")
}

#[test]
fn test_auth_list_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["auth", "list", "--help"], "auth_list.txt")
}

#[test]
fn test_auth_show_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["auth", "show", "--help"], "auth_show.txt")
}

#[test]
fn test_request_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["request", "--help"], "request.txt")
}

#[test]
fn test_request_list_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["request", "list", "--help"], "request_list.txt")
}

#[test]
fn test_request_show_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["request", "show", "--help"], "request_show.txt")
}

#[test]
fn test_request_run_help() -> Result<(), Box<dyn std::error::Error>> {
    verify_help(&["request", "run", "--help"], "request_run.txt")
}

#[test]
fn test_no_args_no_rq_files_shows_help() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("rq_test_no_args_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;
    let result = verify_help_in_dir(&[], "root.txt", &temp_dir);
    fs::remove_dir_all(&temp_dir).ok();
    result
}
