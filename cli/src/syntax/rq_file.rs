use super::{
    parse_result::{EndpointDefinition, ParseResult, RequestWithVariables},
    variable_context::Variable,
};
use crate::syntax::auth::Config as AuthConfig;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct RqFile {
    pub path: PathBuf,
    pub requests: Vec<RequestWithVariables>,
    pub environments: HashMap<String, Vec<Variable>>,
    pub environment_locations: HashMap<String, (String, usize, usize)>,
    pub auth_providers: HashMap<String, AuthConfig>,
    pub endpoints: HashMap<String, EndpointDefinition>,
    pub file_variables: Vec<Variable>,
    pub imported_files: Vec<PathBuf>,
    pub let_variable_locations: HashMap<String, (String, usize, usize)>,
    pub env_variable_locations: HashMap<String, HashMap<String, (String, usize, usize)>>,
}

impl RqFile {
    pub fn from_path(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize {}: {e}", path.display()))?;

        let content = fs::read_to_string(&canonical)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

        let tokens = crate::syntax::tokenize(&content).map_err(|mut e| {
            e.file_path = Some(canonical.to_string_lossy().to_string());
            e
        })?;
        let parse_result = crate::syntax::analyze(&tokens, canonical.clone(), &content)?;

        Ok(Self::from_parse_result(canonical, parse_result))
    }

    pub fn from_path_lenient(path: &Path) -> Option<Self> {
        let canonical = path.canonicalize().ok()?;
        let content = fs::read_to_string(&canonical).ok()?;
        let tokens = crate::syntax::tokenize(&content).ok()?;
        let parse_result = crate::syntax::analyze_lenient(&tokens, canonical.clone(), &content);
        Some(Self::from_parse_result(canonical, parse_result))
    }

    fn from_parse_result(path: PathBuf, parse_result: ParseResult) -> Self {
        Self {
            path,
            requests: parse_result.requests,
            environments: parse_result.environments,
            environment_locations: parse_result.environment_locations,
            auth_providers: parse_result.auth_providers,
            endpoints: parse_result.endpoints,
            file_variables: parse_result.file_variables,
            imported_files: parse_result.imported_files,
            let_variable_locations: parse_result.let_variable_locations,
            env_variable_locations: parse_result.env_variable_locations,
        }
    }
}
