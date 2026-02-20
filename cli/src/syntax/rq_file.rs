use super::{parse_result::RequestWithVariables, variable_context::Variable};
use crate::syntax::auth::Config as AuthConfig;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct RqFile {
    pub path: PathBuf,
    pub requests: Vec<RequestWithVariables>,
    pub environments: HashMap<String, Vec<Variable>>,
    pub auth_providers: HashMap<String, AuthConfig>,
    pub file_variables: Vec<Variable>,
    pub imported_files: Vec<PathBuf>,
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

        Ok(Self {
            path: canonical,
            requests: parse_result.requests,
            environments: parse_result.environments,
            auth_providers: parse_result.auth_providers,
            file_variables: parse_result.file_variables,
            imported_files: parse_result.imported_files,
        })
    }
}
