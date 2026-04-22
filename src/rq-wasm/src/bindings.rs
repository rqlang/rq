use crate::{WasmFs, WasmHttpClient, WasmSecretProvider};
use rq_lib::client::models::{
    AuthListEntry, EndpointEntry, EnvironmentEntry, ReferenceLocation, VariableEntry,
};
use rq_lib::error::RqError;
use rq_lib::RqClient;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

fn make_client(files: HashMap<String, String>, secrets: WasmSecretProvider) -> RqClient {
    RqClient::new(
        Arc::new(WasmFs::new(files)),
        Arc::new(secrets),
        Arc::new(WasmHttpClient),
    )
}

fn parse_files(files_json: &str) -> Result<HashMap<String, String>, JsError> {
    serde_json::from_str(files_json).map_err(|e| JsError::new(&format!("Invalid files map: {e}")))
}

fn parse_secrets(secrets_json: &str) -> WasmSecretProvider {
    #[derive(serde::Deserialize, Default)]
    struct SecretsInput {
        env_file: Option<String>,
        #[serde(default)]
        os_vars: Vec<(String, String)>,
    }
    let input: SecretsInput = serde_json::from_str(secrets_json).unwrap_or_default();
    WasmSecretProvider::new(input.env_file, input.os_vars)
}

fn rq_err(e: RqError) -> JsError {
    JsError::new(&e.to_string())
}

#[derive(Serialize)]
struct CheckError {
    file: String,
    line: usize,
    column: usize,
    message: String,
}

#[derive(Serialize)]
struct CheckResult {
    errors: Vec<CheckError>,
}

#[derive(Serialize)]
struct RequestDetailsJson {
    #[serde(rename = "Request")]
    name: String,
    #[serde(rename = "URL")]
    url: String,
    #[serde(rename = "Method")]
    method: String,
    #[serde(rename = "Headers")]
    headers: HashMap<String, String>,
    #[serde(rename = "Body", skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    auth: Option<AuthRef>,
    #[serde(rename = "RequiredVariables", skip_serializing_if = "Vec::is_empty")]
    required_variables: Vec<String>,
    file: String,
    line: usize,
    character: usize,
}

#[derive(Serialize)]
struct AuthRef {
    name: String,
    #[serde(rename = "type")]
    auth_type: String,
}

#[derive(Serialize)]
struct AuthDetailsJson {
    #[serde(rename = "Auth Configuration")]
    name: String,
    #[serde(rename = "Type")]
    auth_type: String,
    #[serde(rename = "Environment", skip_serializing_if = "Option::is_none")]
    environment: Option<String>,
    #[serde(rename = "Fields")]
    fields: HashMap<String, String>,
    file: String,
    line: usize,
    character: usize,
}

#[wasm_bindgen]
pub fn list_requests(
    files_json: &str,
    secrets_json: &str,
    source: &str,
) -> Result<String, JsError> {
    let (requests, _) = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .list_requests(Path::new(source))
        .map_err(rq_err)?;
    serde_json::to_string(&requests).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_auth(files_json: &str, secrets_json: &str, source: &str) -> Result<String, JsError> {
    let entries: Vec<AuthListEntry> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_auth(Path::new(source))
            .map_err(rq_err)?;
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_environments(
    files_json: &str,
    secrets_json: &str,
    source: &str,
) -> Result<String, JsError> {
    let entries: Vec<EnvironmentEntry> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_environments_with_locations(Path::new(source))
            .map_err(rq_err)?;
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_endpoints(
    files_json: &str,
    secrets_json: &str,
    source: &str,
) -> Result<String, JsError> {
    let entries: Vec<EndpointEntry> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_endpoints(Path::new(source))
            .map_err(rq_err)?;
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_variables(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    env: Option<String>,
) -> Result<String, JsError> {
    let entries: Vec<VariableEntry> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_variables(Path::new(source), env.as_deref())
            .map_err(rq_err)?;
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn check(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    env: Option<String>,
) -> Result<String, JsError> {
    let errors = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .check_path(Path::new(source), env.as_deref())
        .map_err(rq_err)?;

    let check_errors: Vec<CheckError> = errors
        .into_iter()
        .filter_map(|e| {
            if let RqError::Syntax(se) = e {
                se.file_path.map(|f| CheckError {
                    file: f,
                    line: se.line,
                    column: se.column,
                    message: se.message,
                })
            } else {
                None
            }
        })
        .collect();

    let result = CheckResult {
        errors: check_errors,
    };
    serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_request_details(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
    env: Option<String>,
    interpolate: bool,
    skip_required_variables: bool,
    variables_json: Option<String>,
) -> Result<String, JsError> {
    let variables: Vec<String> = match variables_json.as_deref() {
        Some(s) => serde_json::from_str(s)
            .map_err(|e| JsError::new(&format!("Invalid variables_json: {e}")))?,
        None => vec![],
    };
    let details = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .get_request_details(Path::new(source), name, env.as_deref(), interpolate, skip_required_variables, &variables)?;

    let headers: HashMap<String, String> = details.headers.into_iter().collect();
    let auth = details.auth_name.map(|n| AuthRef {
        name: n,
        auth_type: details.auth_type.unwrap_or_default(),
    });

    let view = RequestDetailsJson {
        name: details.name,
        url: details.url,
        method: details.method,
        headers,
        body: details.body,
        auth,
        required_variables: details.required_variables,
        file: details.file,
        line: details.line,
        character: details.character,
    };
    serde_json::to_string(&view).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_auth_details(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
    env: Option<String>,
    interpolate: bool,
) -> Result<String, JsError> {
    let (auth_name, auth_type, fields, file, line, character) = make_client(
        parse_files(files_json)?,
        parse_secrets(secrets_json),
    )
    .get_auth_details(Path::new(source), name, env.as_deref(), interpolate)?;

    let view = AuthDetailsJson {
        name: auth_name,
        auth_type,
        environment: env,
        fields,
        file,
        line,
        character,
    };
    serde_json::to_string(&view).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_environment(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
) -> Result<String, JsError> {
    let entry = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .get_environment(Path::new(source), name)
        .map_err(rq_err)?;
    serde_json::to_string(&entry).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_endpoint(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
) -> Result<String, JsError> {
    let entry = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .get_endpoint(Path::new(source), name)
        .map_err(rq_err)?;
    serde_json::to_string(&entry).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_variable(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
    env: Option<String>,
    interpolate: bool,
) -> Result<String, JsError> {
    let entry = make_client(parse_files(files_json)?, parse_secrets(secrets_json))
        .get_variable(Path::new(source), name, env.as_deref(), interpolate)
        .map_err(rq_err)?;
    serde_json::to_string(&entry).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_variable_refs(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
) -> Result<String, JsError> {
    let refs: Vec<ReferenceLocation> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_variable_references(Path::new(source), name)
            .map_err(rq_err)?;
    serde_json::to_string(&refs).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_endpoint_refs(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    name: &str,
) -> Result<String, JsError> {
    let refs: Vec<ReferenceLocation> =
        make_client(parse_files(files_json)?, parse_secrets(secrets_json))
            .list_endpoint_references(Path::new(source), name)
            .map_err(rq_err)?;
    serde_json::to_string(&refs).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn version() -> String {
    rq_lib::version::app_version().to_string()
}

#[wasm_bindgen]
pub async fn run_request(
    files_json: &str,
    secrets_json: &str,
    source: &str,
    request_name: &str,
    env: Option<String>,
    variables_json: Option<String>,
) -> Result<String, JsError> {
    let files = parse_files(files_json)?;
    let secrets = parse_secrets(secrets_json);
    let client = make_client(files, secrets);

    let variables: Vec<String> = match variables_json.as_deref() {
        Some(s) => serde_json::from_str(s)
            .map_err(|e| JsError::new(&format!("Invalid variables_json: {e}")))?,
        None => vec![],
    };

    let (results, _warnings) = client
        .run(
            Path::new(source),
            Some(request_name),
            env.as_deref(),
            &variables,
        )
        .await
        .map_err(rq_err)?;

    serde_json::to_string(&serde_json::json!({ "results": results }))
        .map_err(|e| JsError::new(&e.to_string()))
}
