use rq_lib::RqClient;
use rq_wasm::bindings;
use rq_wasm::{WasmFs, WasmHttpClient, WasmSecretProvider};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const WORKSPACE: &str = "/workspace";
const API_FILE: &str = "/workspace/api.rq";
const API_SOURCE: &str = "rq greet(\"http://example.test/hi\");\n";

fn files_map(files: &[(&str, &str)]) -> HashMap<String, String> {
    files
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect()
}

fn client_rooted_at(files: &[(&str, &str)], root: &str) -> RqClient {
    RqClient::new(
        Arc::new(WasmFs::with_root(files_map(files), Path::new(root))),
        Arc::new(WasmSecretProvider::new(None, Vec::new())),
        Arc::new(WasmHttpClient),
    )
}

fn empty_workspace_client() -> RqClient {
    client_rooted_at(&[], WORKSPACE)
}

fn workspace() -> &'static Path {
    Path::new(WORKSPACE)
}

#[test]
fn list_requests_in_empty_workspace_returns_no_requests() {
    let target = empty_workspace_client()
        .list_requests(workspace())
        .expect("listing an empty workspace directory should not fail");
    assert!(target.0.is_empty());
}

#[test]
fn list_requests_in_empty_workspace_reports_no_parse_errors() {
    let target = empty_workspace_client()
        .list_requests(workspace())
        .expect("listing an empty workspace directory should not fail");
    assert!(target.1.is_empty());
}

#[test]
fn list_endpoints_in_empty_workspace_returns_no_endpoints() {
    let target = empty_workspace_client()
        .list_endpoints(workspace())
        .expect("listing an empty workspace directory should not fail");
    assert!(target.is_empty());
}

#[test]
fn list_environments_in_empty_workspace_returns_no_environments() {
    let target = empty_workspace_client()
        .list_environments_with_locations(workspace())
        .expect("listing an empty workspace directory should not fail");
    assert!(target.is_empty());
}

#[test]
fn list_variables_in_empty_workspace_returns_no_variables() {
    let target = empty_workspace_client()
        .list_variables(workspace(), None)
        .expect("listing an empty workspace directory should not fail");
    assert!(target.is_empty());
}

#[test]
fn list_auth_in_empty_workspace_returns_no_auth_configs() {
    let target = empty_workspace_client()
        .list_auth(workspace())
        .expect("listing an empty workspace directory should not fail");
    assert!(target.is_empty());
}

#[test]
fn check_in_empty_workspace_reports_no_errors() {
    let target = empty_workspace_client()
        .check_path(workspace(), None)
        .expect("checking an empty workspace directory should not fail");
    assert!(target.is_empty());
}

#[test]
fn list_requests_in_populated_workspace_still_returns_its_requests() {
    let target = client_rooted_at(&[(API_FILE, API_SOURCE)], WORKSPACE)
        .list_requests(workspace())
        .expect("listing a populated workspace should not fail");
    let names: Vec<&str> = target.0.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["greet"]);
}

#[test]
fn list_requests_still_fails_for_a_path_outside_the_workspace_root() {
    let target = client_rooted_at(&[(API_FILE, API_SOURCE)], WORKSPACE)
        .list_requests(Path::new("/somewhere-else"));
    assert!(
        target.is_err(),
        "a path outside the declared root should still be reported as missing"
    );
}

#[test]
fn list_requests_still_fails_for_a_missing_subdirectory_of_the_workspace() {
    let target = client_rooted_at(&[(API_FILE, API_SOURCE)], WORKSPACE)
        .list_requests(Path::new("/workspace/missing"));
    assert!(
        target.is_err(),
        "only the declared root is implied to exist, not arbitrary subdirectories"
    );
}

#[test]
fn get_request_details_still_fails_for_a_request_that_does_not_exist() {
    let target = client_rooted_at(&[], WORKSPACE).get_request_details(
        workspace(),
        "greet",
        None,
        false,
        false,
        &[],
    );
    assert!(
        target.is_err(),
        "an empty workspace has no requests to resolve"
    );
}

fn call_binding(label: &str, result: Result<String, wasm_bindgen::JsError>) -> String {
    match result {
        Ok(json) => json,
        Err(_) => panic!("{label} should not fail"),
    }
}

#[test]
fn binding_lists_no_requests_for_an_empty_workspace() {
    let target = call_binding(
        "list_requests",
        bindings::list_requests("{}", "{}", WORKSPACE),
    );
    assert_eq!(target, r#"{"requests":[],"parse_errors":[]}"#);
}

#[test]
fn binding_trusts_the_requested_source_as_the_workspace_root() {
    let target = call_binding(
        "list_requests",
        bindings::list_requests("{}", "{}", "/any-host-supplied-path"),
    );
    assert_eq!(target, r#"{"requests":[],"parse_errors":[]}"#);
}

#[test]
fn binding_still_lists_requests_from_a_populated_workspace() {
    let files = format!("{{{:?}:{:?}}}", API_FILE, API_SOURCE);
    let target = call_binding(
        "list_requests",
        bindings::list_requests(&files, "{}", WORKSPACE),
    );
    assert!(target.contains(r#""name":"greet""#), "got {target}");
}
