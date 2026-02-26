use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthListEntry {
    pub name: String,
    pub auth_type: String,
}

pub struct RequestDetails {
    pub name: String,
    pub auth_name: Option<String>,
    pub auth_type: Option<String>,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub method: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestInfo {
    pub name: String,
    pub endpoint: Option<String>,
    pub file: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct RequestExecutionResult {
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub body: String,
}

pub struct RqConfig {
    pub source_path: String,
    pub request_name: Option<String>,
    pub environment: Option<String>,
    pub variables: Vec<String>,
    pub output_format: String,
}
