use super::auth::Config as AuthProvider;
use super::http_method::HttpMethod;
use super::variable_context::Variable;

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointDefinition {
    pub name: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub headers_var: Option<String>,
    pub qs: Option<String>,
    pub auth: Option<String>,
    pub timeout: Option<String>,
    pub variables: Vec<Variable>,
    pub has_requests: bool,
    pub source_path: Option<String>,
    pub related_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub name: String,
    pub url: String,
    pub raw_url: String,
    pub method: HttpMethod,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub headers_var: Option<String>,
    pub endpoint: Option<String>,
    pub auth: Option<String>,
    pub timeout: Option<String>,
    pub source_path: Option<String>,
    pub related_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestWithVariables {
    pub request: Request,
    pub endpoint_variables: Vec<Variable>,
    pub request_variables: Vec<Variable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub requests: Vec<RequestWithVariables>,
    pub environments: std::collections::HashMap<String, Vec<Variable>>,
    pub auth_providers: std::collections::HashMap<String, AuthProvider>,
    pub endpoints: std::collections::HashMap<String, EndpointDefinition>,
    pub file_variables: Vec<Variable>,
    pub imported_files: Vec<std::path::PathBuf>,
}
