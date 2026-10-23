use rq_lib::client::models::{PreparedRequest, RequestTarget};
use rq_lib::RqClient;
use rq_wasm::{WasmFs, WasmHttpClient, WasmSecretProvider};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const WORKSPACE: &str = "/workspace";
const REQUEST_FILE: &str = "/workspace/sub/items.rq";

fn workspace_files() -> HashMap<String, String> {
    [
        (
            "/workspace/_shared.rq",
            "let base_url = \"http://example.test\";\n",
        ),
        (
            "/workspace/sub/_shared.rq",
            "import \"../_shared\";\n\nlet collection = \"shared\";\n\nauth api_auth(auth_type.bearer) {\n    token: \"{{api_key}}\",\n}\n",
        ),
        (
            REQUEST_FILE,
            "import \"_shared\";\n\nlet collection = \"local\";\n\n[auth(\"api_auth\")]\nrq list(\"{{base_url}}/{{collection}}\");\n",
        ),
    ]
    .into_iter()
    .map(|(path, source)| (path.to_string(), source.to_string()))
    .collect()
}

fn prepare() -> PreparedRequest {
    let client = RqClient::new(
        Arc::new(WasmFs::with_root(workspace_files(), Path::new(WORKSPACE))),
        Arc::new(WasmSecretProvider::new(
            Some("API_KEY=from-context-env\n".to_string()),
            Vec::new(),
        )),
        Arc::new(WasmHttpClient),
    );
    let target =
        RequestTarget::new(Path::new(WORKSPACE), "list").in_file(Some(Path::new(REQUEST_FILE)));
    client
        .prepare_request(target, None, &[])
        .expect("prepare_request failed")
}

#[test]
fn prepared_url_uses_the_local_override_and_the_parent_import() {
    let target = prepare();
    assert_eq!(target.request.url, "http://example.test/local");
}

#[test]
fn prepared_auth_header_uses_the_secrets_of_the_context() {
    let target = prepare();
    let authorization = target
        .request
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.clone());
    assert_eq!(authorization.as_deref(), Some("Bearer from-context-env"));
}

fn prepare_from_disk(
    files: &[(&str, &str)],
    workspace: &str,
    request_file: &str,
) -> PreparedRequest {
    let disk = tempfile::tempdir().expect("tempdir");
    for (file, content) in files {
        let path = disk.path().join(file);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, content).expect("write");
    }
    let root = disk.path().to_string_lossy().replace('\\', "/");
    let workspace = format!("{root}/{workspace}");
    let request_file = format!("{root}/{request_file}");
    let client = RqClient::new(
        Arc::new(WasmFs::with_root(HashMap::new(), Path::new(&workspace))),
        Arc::new(WasmSecretProvider::new(None, Vec::new())),
        Arc::new(WasmHttpClient),
    );
    let target =
        RequestTarget::new(Path::new(&workspace), "upload").in_file(Some(Path::new(&request_file)));
    client
        .prepare_request(target, None, &[])
        .expect("prepare_request failed")
}

#[test]
fn prepared_body_reads_a_file_saved_next_to_the_request() {
    let target = prepare_from_disk(
        &[
            ("api/upload.rq", "rq upload(\"http://x\", $[\"Content-Type\": \"text/plain\"], io.read_file(\"data.txt\"));\n"),
            ("api/data.txt", "from disk"),
        ],
        "api",
        "api/upload.rq",
    );
    assert_eq!(target.request.body.as_deref(), Some("from disk"));
}

#[test]
fn prepared_body_reads_a_file_outside_the_workspace() {
    let target = prepare_from_disk(
        &[
            ("api/upload.rq", "rq upload(\"http://x\", $[\"Content-Type\": \"text/plain\"], io.read_file(\"../fixtures/data.txt\"));\n"),
            ("fixtures/data.txt", "shared fixture"),
        ],
        "api",
        "api/upload.rq",
    );
    assert_eq!(target.request.body.as_deref(), Some("shared fixture"));
}
