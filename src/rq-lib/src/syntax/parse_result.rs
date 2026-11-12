use std::ops::Range;

use crate::syntax::auth::Config as AuthConfig;
use crate::syntax::http_method::HttpMethod;
use crate::syntax::types::{SlotBinding, ValueType};
use crate::syntax::variable_context::Variable;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthLocation {
    pub file: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointDefinition {
    pub name: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub headers_var: Option<String>,
    pub slot_bindings: Vec<SlotBinding>,
    pub qs: Option<String>,
    pub auth: Option<String>,
    pub auth_location: Option<AuthLocation>,
    pub timeout: Option<String>,
    pub description: Option<String>,
    pub confirm: bool,
    pub variables: Vec<Variable>,
    pub has_requests: bool,
    pub is_template: bool,
    pub source_path: Option<String>,
    pub related_files: Vec<String>,
    pub line: usize,
    pub character: usize,
    pub declaration_line: usize,
    pub declaration_end_line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequiredVariable {
    pub name: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub name: String,
    pub url: String,
    pub raw_url: String,
    pub method: HttpMethod,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub body_type: Option<ValueType>,
    pub headers_var: Option<String>,
    pub slot_bindings: Vec<SlotBinding>,
    pub endpoint: Option<String>,
    pub auth: Option<String>,
    pub auth_location: Option<AuthLocation>,
    pub timeout: Option<String>,
    pub description: Option<String>,
    pub confirm: bool,
    pub required_variables: Vec<RequiredVariable>,
    pub source_path: Option<String>,
    pub related_files: Vec<String>,
    pub line: usize,
    pub character: usize,
}

impl Request {
    pub fn required_names(&self) -> impl Iterator<Item = &String> {
        self.required_variables
            .iter()
            .map(|required| &required.name)
    }

    pub fn requires(&self, name: &str) -> bool {
        self.required_names().any(|required| required == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestScope {
    pub source_path: String,
    pub span: Range<usize>,
    pub endpoint_header: Option<Range<usize>>,
    pub required_variables: Vec<RequiredVariable>,
}

impl RequestScope {
    pub fn requires(&self, name: &str) -> bool {
        self.required_variables
            .iter()
            .any(|required| required.name == name)
    }
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
    pub request_scopes: Vec<RequestScope>,
    pub environments: std::collections::HashMap<String, Vec<Variable>>,
    pub environment_locations: std::collections::HashMap<String, (String, usize, usize)>,
    pub environment_colors: std::collections::HashMap<String, crate::syntax::ui_color::UiColor>,
    pub confirm_environments: std::collections::HashSet<String>,
    pub auth_providers: std::collections::HashMap<String, AuthConfig>,
    pub endpoints: std::collections::HashMap<String, EndpointDefinition>,
    pub file_variables: Vec<Variable>,
    pub imported_files: Vec<std::path::PathBuf>,
    pub let_variable_locations: std::collections::HashMap<String, (String, usize, usize)>,
    pub env_variable_locations: std::collections::HashMap<
        String,
        std::collections::HashMap<String, (String, usize, usize)>,
    >,
}
