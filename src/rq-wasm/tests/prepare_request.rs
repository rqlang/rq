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
