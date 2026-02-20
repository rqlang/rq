use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;

#[tokio::test]
async fn test_auth_bearer_integration() {
    // 1. Start Mock Server
    let mock_server = MockServer::start().await;

    // 2. Setup expectation: Protected Endpoint
    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", "Bearer my-secret-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success",
            "user": "authenticated"
        })))
        .mount(&mock_server)
        .await;

    // 3. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_bearer.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    let rq_file_content = template_content.replace("{{MOCK_URL}}", &mock_server.uri());

    let rq_path = "target/test_auth_bearer.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 4. Run `rq`
    let output = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(output.status.success(), "rq failed to execute: {stderr}");
    assert!(
        stdout.contains("status: 200")
            || stdout.contains("\"status\": \"success\"")
            || stdout.contains("success"),
        "Expected 200 OK or success body, got:\n{stdout}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_client_credentials_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Token Endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("client_id=test-client"))
        .and(body_string_contains("client_secret=test-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mocked_access_token_xyz",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Protected Endpoint (Expects the token returned above)
    Mock::given(method("GET"))
        .and(path("/api/resource"))
        .and(header("Authorization", "Bearer mocked_access_token_xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": "secure_data"
        })))
        .mount(&mock_server)
        .await;

    // 3. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_cc.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    let rq_file_content = template_content.replace("{{MOCK_URL}}", &mock_server.uri());

    let rq_path = "target/test_auth_cc.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 4. Run `rq`
    let output = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(
        output.status.success(),
        "rq failed to execute oauth2 cc flow: {stderr}"
    );
    assert!(
        stdout.contains("status: 200") || stdout.contains("secure_data"),
        "Expected success body, got:\n{stdout}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_auth_code_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Protected Endpoint (Expects the manually supplied token)
    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", "Bearer fallback-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success"
        })))
        .mount(&mock_server)
        .await;

    // 2. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_ac.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    let rq_file_content = template_content.replace("{{MOCK_URL}}", &mock_server.uri());
    let rq_path = "target/test_auth_ac.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 3. CASE A: Expect Failure (No token provided)
    let output_fail = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    assert!(!output_fail.status.success(), "Should fail correctly");
    let stderr_fail = String::from_utf8_lossy(&output_fail.stderr);
    assert!(
        stderr_fail.contains("requires interactive authentication"),
        "Expected not implemented error, got: {stderr_fail}"
    );

    // 4. CASE B: Expect Success (Fallback variable provided)
    let output_ok = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .arg("-v")
        .arg("auth_token=fallback-token-123")
        .output()
        .expect("Failed to execute rq binary");

    let stdout_ok = String::from_utf8_lossy(&output_ok.stdout);
    let stderr_ok = String::from_utf8_lossy(&output_ok.stderr);

    assert!(
        output_ok.status.success(),
        "Should succeed with fallback variable: {stderr_ok}"
    );
    assert!(
        stdout_ok.contains("status: 200"),
        "Expected 200 OK, got: {stdout_ok}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_client_credentials_variables() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mocked_variable_token",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/resource"))
        .and(header("Authorization", "Bearer mocked_variable_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": "secure_data"
        })))
        .mount(&mock_server)
        .await;

    let template_path = "tests/fixtures/templates/auth_oauth2_cc_vars.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    // We replace MOCK_URL but put it into a VARIABLE, not directly into the text
    // effectively testing: token_url: "{{MOCK_URL}}/token"
    let content_with_var_ref = template_content.replace("{{MOCK_URL}}", "{{mock_url_var}}");

    // Append environment definition to the .rq file itself
    let rq_content = format!(
        "{}\n\nenv test {{\n    mock_url_var: \"{}\"\n}}",
        content_with_var_ref,
        mock_server.uri()
    );

    let rq_path = "target/test_auth_cc_vars.rq";
    std::fs::write(rq_path, rq_content).unwrap();

    let output = common::rq_cmd()
        .arg("-s")
        .arg("test_auth_cc_vars.rq")
        .arg("-e")
        .arg("test")
        .current_dir("target")
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(
        output.status.success(),
        "rq failed with variables: {stderr}"
    );
    assert!(
        stdout.contains("status: 200"),
        "Expected success with variables"
    );
}

#[tokio::test]
async fn test_auth_oauth2_client_credentials_cert_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Token Endpoint
    // We expect the client to send client_id but NO client_secret because we are using cert
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("client_id=test-client"))
        .and(body_string_contains("client_assertion_type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mocked_access_token_cert",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Protected Endpoint
    Mock::given(method("GET"))
        .and(path("/api/resource"))
        .and(header("Authorization", "Bearer mocked_access_token_cert"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": "secure_data_via_cert"
        })))
        .mount(&mock_server)
        .await;

    // 3. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_cc_cert.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    // Use relative path from the .rq file (which will be in target/) to the cert
    // target/ is 1 level deep from root. tests/ is in root.
    // So ../tests/fixtures/certs/client.p12
    let cert_path_str = "../tests/fixtures/certs/client.p12";

    let rq_file_content = template_content
        .replace("{{MOCK_URL}}", &mock_server.uri())
        .replace("{{CERT_PATH}}", cert_path_str);

    let rq_path = "target/test_auth_cc_cert.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 4. Run `rq`
    let output = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(output.status.success(), "rq failed to execute: {stderr}");
    assert!(
        stdout.contains("status: 200") || stdout.contains("secure_data_via_cert"),
        "Expected 200 OK or success body, got:\n{stdout}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_client_credentials_pfx_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Token Endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("client_id=test-client"))
        .and(body_string_contains("client_assertion_type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mocked_access_token_pfx",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Protected Endpoint
    Mock::given(method("GET"))
        .and(path("/api/resource"))
        .and(header("Authorization", "Bearer mocked_access_token_pfx"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": "secure_data_via_pfx"
        })))
        .mount(&mock_server)
        .await;

    // 3. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_cc_cert.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    // Use .pfx file
    let cert_path_str = "../tests/fixtures/certs/client.pfx";

    let rq_file_content = template_content
        .replace("{{MOCK_URL}}", &mock_server.uri())
        .replace("{{CERT_PATH}}", cert_path_str);

    let rq_path = "target/test_auth_cc_pfx.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 4. Run `rq`
    let output = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(output.status.success(), "rq failed to execute: {stderr}");
    assert!(
        stdout.contains("status: 200") || stdout.contains("secure_data_via_pfx"),
        "Expected 200 OK or success body, got:\n{stdout}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_client_credentials_pem_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Token Endpoint
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("client_id=test-client"))
        .and(body_string_contains("client_assertion_type"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mocked_access_token_pem",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    // 2. Mock Protected Endpoint
    Mock::given(method("GET"))
        .and(path("/api/resource"))
        .and(header("Authorization", "Bearer mocked_access_token_pem"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": "secure_data_via_pem"
        })))
        .mount(&mock_server)
        .await;

    // 3. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_cc_cert.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    // Use .pem file
    let cert_path_str = "../tests/fixtures/certs/client.pem";

    let rq_file_content = template_content
        .replace("{{MOCK_URL}}", &mock_server.uri())
        .replace("{{CERT_PATH}}", cert_path_str);

    let rq_path = "target/test_auth_cc_pem.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 4. Run `rq`
    let output = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert!(output.status.success(), "rq failed to execute: {stderr}");
    assert!(
        stdout.contains("status: 200") || stdout.contains("secure_data_via_pem"),
        "Expected 200 OK or success body, got:\n{stdout}"
    );
}

#[tokio::test]
async fn test_auth_oauth2_implicit_integration() {
    let mock_server = MockServer::start().await;

    // 1. Mock Protected Endpoint (Expects the manually supplied token)
    Mock::given(method("GET"))
        .and(path("/protected"))
        .and(header("Authorization", "Bearer implicit-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success"
        })))
        .mount(&mock_server)
        .await;

    // 2. Create .rq file from template
    let template_path = "tests/fixtures/templates/auth_oauth2_implicit.rq.template";
    let template_content =
        std::fs::read_to_string(template_path).expect("Failed to read template file");

    let rq_file_content = template_content.replace("{{MOCK_URL}}", &mock_server.uri());
    let rq_path = "target/test_auth_implicit.rq";
    std::fs::write(rq_path, rq_file_content).unwrap();

    // 3. CASE A: Expect Failure (No token provided)
    let output_fail = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .output()
        .expect("Failed to execute rq binary");

    assert!(!output_fail.status.success(), "Should fail correctly");
    let stderr_fail = String::from_utf8_lossy(&output_fail.stderr);
    assert!(
        stderr_fail.contains("requires interactive authentication"),
        "Expected not implemented error, got: {stderr_fail}"
    );

    // 4. CASE B: Expect Success (Fallback variable provided)
    let output_ok = common::rq_cmd()
        .arg("-s")
        .arg(rq_path)
        .arg("-v")
        .arg("auth_token=implicit-token-123")
        .output()
        .expect("Failed to execute rq binary");

    let stdout_ok = String::from_utf8_lossy(&output_ok.stdout);
    let stderr_ok = String::from_utf8_lossy(&output_ok.stderr);

    assert!(
        output_ok.status.success(),
        "Should succeed with fallback variable: {stderr_ok}"
    );
    assert!(
        stdout_ok.contains("status: 200"),
        "Expected 200 OK, got: {stdout_ok}"
    );
}
