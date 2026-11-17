use rq_wasm::bindings;
use serde_json::Value;

fn describe_in(headers_json: &str, body: &[u8], download_dir: Option<&str>) -> Value {
    bindings::binary_response_body(
        headers_json,
        body,
        "reports/monthly",
        download_dir.map(str::to_string),
    )
    .ok()
    .and_then(|output| serde_json::from_str(&output).ok())
    .unwrap_or(Value::String("error".to_string()))
}

fn describe(headers_json: &str, body: &[u8]) -> Value {
    describe_in(headers_json, body, Some("/downloads"))
}

#[test]
fn a_text_body_is_not_binary() {
    let target = describe(r#"{"content-type":"application/json"}"#, b"{}");
    assert_eq!(target, Value::Null);
}

#[test]
fn a_binary_body_gets_its_media_type_size_and_file_name() {
    let target = describe(r#"{"content-type":"application/pdf"}"#, b"%PDF-1.7");
    assert_eq!(
        (
            target["media_type"].as_str(),
            target["size"].as_u64(),
            target["file_name"].as_str()
        ),
        (
            Some("application/pdf"),
            Some(8),
            Some("reports.monthly.pdf")
        )
    );
}

#[test]
fn a_binary_body_is_downloaded_to_its_own_folder_in_the_download_dir() {
    let target = describe("{}", &[0, 0xff]);
    let path = target["path"].as_str().unwrap_or_default();
    let folder = path
        .strip_prefix("/downloads/")
        .and_then(|rest| rest.strip_suffix("/reports.monthly.bin"))
        .unwrap_or_default();
    assert_eq!(folder.len(), 36);
}

#[test]
fn without_a_download_dir_a_binary_body_has_no_path() {
    let target = describe_in("{}", &[0, 0xff], None);
    assert_eq!(
        (target["path"].is_null(), target["size"].as_u64()),
        (true, Some(2))
    );
}
