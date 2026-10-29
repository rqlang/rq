use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::syntax::auth::Config;
use crate::syntax::ui_color::UiColor;
use crate::syntax::variable_context::{VariableContext, VariableValue};
use crate::syntax::Request;

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
    pub timeout: Option<String>,
    pub description: Option<String>,
    pub required_variables: Vec<String>,
    pub confirmation_required: bool,
    pub file: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestInfo {
    pub name: String,
    pub method: String,
    pub url: String,
    pub description: Option<String>,
    pub endpoint: Option<String>,
    pub endpoint_description: Option<String>,
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
    pub color: Option<UiColor>,
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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub request: Request,
    pub timeout: Option<Duration>,
    pub auth: Option<PreparedAuth>,
    pub confirmation_required: bool,
    pub agent_confirmation_required: bool,
    pub(crate) context: VariableContext,
}

impl PreparedRequest {
    pub fn secret_values(&self) -> Vec<String> {
        let mut values: Vec<String> = self
            .context
            .secret_variables
            .iter()
            .filter_map(|variable| match &variable.value {
                VariableValue::String(value) | VariableValue::Json(value) if !value.is_empty() => {
                    Some(value.clone())
                }
                _ => None,
            })
            .collect();
        values.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        values.dedup();
        values
    }
}

#[derive(Debug, Clone)]
pub struct PreparedAuth {
    pub config: Config,
    pub token_required: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestTarget<'a> {
    pub source: &'a Path,
    pub scope_file: Option<&'a Path>,
    pub name: &'a str,
}

impl<'a> RequestTarget<'a> {
    pub fn new(source: &'a Path, name: &'a str) -> Self {
        Self {
            source,
            scope_file: None,
            name,
        }
    }

    pub fn in_file(self, scope_file: Option<&'a Path>) -> Self {
        Self { scope_file, ..self }
    }
}
