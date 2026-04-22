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
    pub required_variables: Vec<String>,
    pub file: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestInfo {
    pub name: String,
    pub endpoint: Option<String>,
    pub file: String,
    pub endpoint_file: Option<String>,
    pub endpoint_line: Option<usize>,
    pub endpoint_character: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentEntry {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointEntry {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub character: usize,
    pub is_template: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VariableEntry {
    pub name: String,
    pub value: String,
    pub file: String,
    pub line: usize,
    pub character: usize,
    pub source: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceLocation {
    pub file: String,
    pub line: usize,
    pub character: usize,
}
