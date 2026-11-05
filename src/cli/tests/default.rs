mod common;
use common::rq_cmd;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_default_run_with_source() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing default command: rq -s <path>");

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/get"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let rq_content = format!("rq basic(\"{}/get\");", mock_server.uri());
    let rq_path = format!("{}/test_default_basic.rq", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&rq_path, rq_content)?;

    let rq_output = rq_cmd()
        .args(["-s", &rq_path, "-n", "basic"])
        .current_dir(".")
        .output()?;

    let stdout = String::from_utf8_lossy(&rq_output.stdout);
    let stderr = String::from_utf8_lossy(&rq_output.stderr);

    if stderr.contains("Syntax error") {
        return Err(format!("Syntax error: {stderr}").into());
    }

    if !rq_output.status.success() {
        return Err(format!("Command failed. stderr: {stderr}, stdout: {stdout}").into());
    }

    if stdout.contains("basic  GET ") {
        Ok(())
    } else {
        Err(format!("Unexpected output. stdout: {stdout}").into())
    }
}

#[tokio::test]
async fn test_default_run_with_all_args() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing default command: rq -s <path> -n <name> -e <env>");

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/env-test"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let rq_content = format!(
        r#"
env local {{
    base_url: "{}",
    auth_token: "local-token-123",
}}

rq env_local_demo("{{{{base_url}}}}/api/env-test");
"#,
        mock_server.uri()
    );
    let rq_path = format!("{}/test_default_env.rq", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&rq_path, rq_content)?;

    let rq_output = rq_cmd()
        .args(["-s", &rq_path, "-n", "env_local_demo", "-e", "local"])
        .current_dir(".")
        .output()?;

    let stdout = String::from_utf8_lossy(&rq_output.stdout);
    let stderr = String::from_utf8_lossy(&rq_output.stderr);

    if stderr.contains("Syntax error") {
        return Err(format!("Syntax error: {stderr}").into());
    }

    if !rq_output.status.success() {
        return Err(format!("Command failed. stderr: {stderr}, stdout: {stdout}").into());
    }

    Ok(())
}

#[tokio::test]
async fn test_default_run_with_variables() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing default command: rq -s <path> -v KEY=VALUE");

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/get"))
        .and(query_param("color", "blue"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let rq_content = format!(
        r#"
let color = "red";
rq cli_variable_override("{}/get?color={{{{color}}}}");
"#,
        mock_server.uri()
    );
    let rq_path = format!("{}/test_default_vars.rq", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&rq_path, rq_content)?;

    let rq_output = rq_cmd()
        .args([
            "-s",
            &rq_path,
            "-n",
            "cli_variable_override",
            "-v",
            "color=blue",
        ])
        .current_dir(".")
        .output()?;

    let stdout = String::from_utf8_lossy(&rq_output.stdout);
    let stderr = String::from_utf8_lossy(&rq_output.stderr);

    if stderr.contains("Syntax error") {
        return Err(format!("Syntax error: {stderr}").into());
    }

    if !rq_output.status.success() {
        return Err(format!("Command failed. stderr: {stderr}, stdout: {stdout}").into());
    }

    Ok(())
}

#[test]
fn test_default_run_without_name_lists_requests() -> Result<(), Box<dyn std::error::Error>> {
    let rq_path = format!("{}/test_default_no_name.rq", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&rq_path, "rq list(\"http://localhost/list\");\n")?;

    let rq_output = rq_cmd().args(["-s", &rq_path]).output()?;

    let stderr = String::from_utf8_lossy(&rq_output.stderr);
    let expected = format!("Error: --name is required. Requests in {rq_path}: list");
    assert_eq!(
        (rq_output.status.code(), stderr.trim()),
        (Some(1), expected.as_str())
    );
    Ok(())
}
