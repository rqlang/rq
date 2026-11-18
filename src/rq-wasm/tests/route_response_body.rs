use rq_wasm::bindings;
use serde_json::Value;

const OUTPUT_FILE: &str =
    r#"{"path":"/workspace/out/report.json","is_folder":false,"applies_to_every_body":true}"#;
const OUTPUT_FOLDER: &str =
    r#"{"path":"/workspace/downloads","is_folder":true,"applies_to_every_body":false}"#;

fn route_with(
    headers_json: &str,
    body: &[u8],
    status: u16,
    output_json: Option<&str>,
    download_dir: Option<&str>,
) -> Value {
    bindings::route_response_body(
        headers_json,
        body,
        "reports/monthly",
        status,
        output_json.map(str::to_string),
        download_dir.map(str::to_string),
    )
    .ok()
    .and_then(|output| serde_json::from_str(&output).ok())
    .unwrap_or(Value::String("error".to_string()))
}

fn route(headers_json: &str, body: &[u8]) -> Value {
    route_with(headers_json, body, 200, None, Some("/downloads"))
}

#[test]
fn a_text_body_has_no_file() {
    let target = route(r#"{"content-type":"application/json"}"#, b"{}");
    assert_eq!(target, serde_json::json!({}));
}

#[test]
fn a_binary_body_gets_its_media_type_size_and_file_name() {
    let target = &route(r#"{"content-type":"application/pdf"}"#, b"%PDF-1.7")["file"];
    assert_eq!(
        (
            target["media_type"].as_str(),
            target["size"].as_u64(),
            target["file_name"].as_str(),
            target["binary"].as_bool()
        ),
        (
            Some("application/pdf"),
            Some(8),
            Some("reports.monthly.pdf"),
            Some(true)
        )
    );
}

#[test]
fn a_binary_body_is_downloaded_to_its_own_folder_in_the_download_dir() {
    let target = route("{}", &[0, 0xff]);
    let path = target["file"]["path"].as_str().unwrap_or_default();
    let folder = path
        .strip_prefix("/downloads/")
        .and_then(|rest| rest.strip_suffix("/reports.monthly.bin"))
        .unwrap_or_default();
    assert_eq!(folder.len(), 36);
}

#[test]
fn without_a_download_dir_a_binary_body_has_no_path() {
    let target = &route_with("{}", &[0, 0xff], 200, None, None)["file"];
    assert_eq!(
        (target["path"].is_null(), target["size"].as_u64()),
        (true, Some(2))
    );
}

#[test]
fn a_request_output_takes_a_text_body() {
    let target = &route_with(
        r#"{"content-type":"application/json"}"#,
        b"{}",
        200,
        Some(OUTPUT_FILE),
        Some("/downloads"),
    )["file"];
    assert_eq!(
        (target["path"].as_str(), target["binary"].as_bool()),
        (Some("/workspace/out/report.json"), Some(false))
    );
}

#[test]
fn an_endpoint_output_folder_takes_a_binary_body() {
    let target = &route_with("{}", &[0, 0xff], 200, Some(OUTPUT_FOLDER), None)["file"];
    assert_eq!(
        target["path"].as_str().map(|path| path.replace('\\', "/")),
        Some("/workspace/downloads/reports.monthly.bin".to_string())
    );
}

#[test]
fn an_error_response_is_not_written_to_the_output_and_says_why() {
    let target = route_with(
        r#"{"content-type":"application/json"}"#,
        b"{}",
        404,
        Some(OUTPUT_FILE),
        Some("/downloads"),
    );
    assert_eq!(
        target,
        serde_json::json!({"output_skipped": {"path": "/workspace/out/report.json", "reason": "the response status is 404"}})
    );
}
