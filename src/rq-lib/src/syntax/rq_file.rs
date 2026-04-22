use super::{
    fs::Fs,
    parse_result::{EndpointDefinition, ParseResult, RequestWithVariables},
    variable_context::Variable,
};
use crate::syntax::auth::Config as AuthConfig;
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub required_variable_locations: HashMap<String, (String, usize, usize)>,
}

impl RqFile {
    pub fn from_content(
        path: PathBuf,
        content: &str,
        fs: &dyn Fs,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let tokens = crate::syntax::tokenize(content).map_err(|mut e| {
            e.file_path = Some(path.to_string_lossy().to_string());
            e
        })?;
        let parse_result = crate::syntax::analysis::analyze(&tokens, path.clone(), content, fs)?;
        Ok(Self::from_parse_result(path, parse_result))
    }

    pub fn from_content_lenient(path: PathBuf, content: &str, fs: &dyn Fs) -> Self {
        let tokens = match crate::syntax::tokenize(content) {
            Ok(t) => t,
            Err(_) => return Self::empty(path),
        };
        let parse_result =
            crate::syntax::analysis::analyze_lenient(&tokens, path.clone(), content, fs);
        Self::from_parse_result(path, parse_result)
    }

    fn empty(path: PathBuf) -> Self {
        Self::from_parse_result(
            path,
            ParseResult {
                requests: Vec::new(),
                environments: HashMap::new(),
                environment_locations: HashMap::new(),
                auth_providers: HashMap::new(),
                endpoints: HashMap::new(),
                file_variables: Vec::new(),
                imported_files: Vec::new(),
                let_variable_locations: HashMap::new(),
                env_variable_locations: HashMap::new(),
                required_variable_locations: HashMap::new(),
            },
        )
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
            required_variable_locations: parse_result.required_variable_locations,
        }
    }
}
