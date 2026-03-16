use super::rq_client_models::{RequestDetails, RequestExecutionResult, RequestInfo, RqConfig};
use crate::core::error::RqError;
use crate::core::logger::Logger;
use crate::syntax::{RqFile, Variable, VariableValue};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type AuthDetails = (
    String,
    String,
    HashMap<String, String>,
    String,
    usize,
    usize,
);

pub struct RqClient {
    config: RqConfig,
}

impl RqClient {
    pub fn new(config: RqConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<Vec<RequestExecutionResult>, RqError> {
        let source_path = Path::new(&self.config.source_path);

        let rq_files =
            Self::get_rq_files_to_process(source_path, self.config.request_name.as_deref())?;

        if rq_files.is_empty() {
            return Err(RqError::RequestNotFound(format!(
                "No .rq files found in directory: {}",
                source_path.display()
            )));
        }

        let mut all_results = Vec::new();

        for rq_file in rq_files {
            let env_vars = if let Some(env_name) = &self.config.environment {
                if let Some(vars) = rq_file.environments.get(env_name) {
                    vars.clone()
                } else {
                    return Err(RqError::EnvironmentNotFound(env_name.clone()));
                }
            } else {
                Vec::new()
            };

            let secret_vars = self.load_secrets(source_path)?;
            let cli_vars = self.parse_cli_variables()?;

            let filtered_requests = self.filter_requests(rq_file.requests);

            if filtered_requests.is_empty() {
                if let Some(ref request_name) = self.config.request_name {
                    eprintln!("No request found with name '{request_name}'");
                } else {
                    eprintln!("No requests found in the file");
                }
                return Ok(vec![]);
            }

            if let Some(ref request_name) = self.config.request_name {
                Logger::debug(&format!(
                    "Found {} request(s) with name '{request_name}':",
                    filtered_requests.len()
                ));
            } else {
                Logger::debug(&format!(
                    "Found {} request(s) in total:",
                    filtered_requests.len()
                ));
            }

            let mut results = Vec::new();

            for (i, req_with_vars) in filtered_requests.into_iter().enumerate() {
                Logger::debug(&format!("Request {}: {:?}", i + 1, req_with_vars.request));

                let context = crate::syntax::variable_context::VariableContext::builder()
                    .file_variables(rq_file.file_variables.clone())
                    .environment_variables(env_vars.clone())
                    .secret_variables(secret_vars.clone())
                    .endpoint_variables(req_with_vars.endpoint_variables)
                    .request_variables(req_with_vars.request_variables)
                    .cli_variables(cli_vars.clone())
                    .build();

                let mut working = req_with_vars.request;

                let search_paths = Self::build_search_paths(
                    &working,
                    &rq_file.path,
                    &rq_file.imported_files,
                    source_path,
                );

                if let Some(header_var) = working.headers_var.clone() {
                    let headers = std::mem::take(&mut working.headers);
                    working.headers = Self::apply_headers_var(&header_var, headers, &context)
                        .map_err(|e| {
                            let (line, col, path) = crate::syntax::resolve::find_variable_location(
                                &search_paths,
                                &header_var,
                            );
                            RqError::Syntax(crate::syntax::error::SyntaxError::with_file(
                                e,
                                line,
                                col,
                                0..0,
                                path.display().to_string(),
                            ))
                        })?;
                }

                let mut resolved_request =
                    crate::syntax::resolve_variables(working, &context, &search_paths)?;

                if let Some(auth_name) = resolved_request.auth.as_deref() {
                    if !auth_name.trim().is_empty() {
                        if let Some(auth_provider) = rq_file.auth_providers.get(auth_name) {
                            let resolved_provider = crate::syntax::resolve_auth_provider(
                                auth_provider.clone(),
                                &context,
                                &search_paths,
                            )?;

                            let provider = resolved_provider.auth_type.get_config();

                            match provider
                                .configure(
                                    &resolved_provider,
                                    &context,
                                    resolved_request.url.clone(),
                                    resolved_request.headers.clone(),
                                )
                                .await
                            {
                                Ok((modified_url, modified_headers)) => {
                                    resolved_request.url = modified_url;
                                    resolved_request.headers = modified_headers;
                                }
                                Err(e) => {
                                    return Err(RqError::Auth(format!(
                                        "Configuration '{auth_name}' failed: {e}"
                                    )));
                                }
                            }
                        } else {
                            return Err(RqError::Validation(format!(
                                "Auth configuration '{auth_name}' not found"
                            )));
                        }
                    }
                }

                if self.config.output_format != "json" {
                    println!(
                        "Request: \"{}\" {} {}",
                        resolved_request.name,
                        resolved_request.method.as_str(),
                        resolved_request.url
                    );
                }

                let start_time = Instant::now();
                match super::http::execute_request(&resolved_request).await {
                    Ok(response) => {
                        let elapsed = start_time.elapsed();

                        let mut request_headers = HashMap::new();
                        for (key, value) in &resolved_request.headers {
                            request_headers.insert(key.clone(), value.clone());
                        }

                        results.push(RequestExecutionResult {
                            request_name: resolved_request.name.clone(),
                            method: resolved_request.method.as_str().to_string(),
                            url: resolved_request.url.clone(),
                            status: response.status,
                            elapsed_ms: elapsed.as_millis() as u64,
                            request_headers,
                            response_headers: response.headers.clone(),
                            body: response.body.clone(),
                        });
                    }
                    Err(error) => {
                        return Err(RqError::Generic(error));
                    }
                }
            }

            all_results.extend(results);
        }

        Ok(all_results)
    }

    pub fn list_requests(source_path: &Path) -> Result<Vec<RequestInfo>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let rq_files = Self::get_rq_files_to_process(source_path, None)?;

        let mut requests = Vec::new();
        for rq_file in rq_files {
            for req_with_vars in &rq_file.requests {
                let (endpoint_file, endpoint_line, endpoint_character) =
                    if let Some(ep_name) = &req_with_vars.request.endpoint {
                        if let Some(ep) = rq_file.endpoints.get(ep_name) {
                            let ep_file = ep
                                .source_path
                                .as_deref()
                                .map(crate::core::paths::clean_path_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| crate::core::paths::clean_path(&rq_file.path));
                            (Some(ep_file), Some(ep.line), Some(ep.character))
                        } else {
                            (None, None, None)
                        }
                    } else {
                        (None, None, None)
                    };
                requests.push(RequestInfo {
                    name: req_with_vars.request.name.clone(),
                    endpoint: req_with_vars.request.endpoint.clone(),
                    file: crate::core::paths::clean_path(&rq_file.path),
                    endpoint_file,
                    endpoint_line,
                    endpoint_character,
                });
            }
        }

        requests.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(requests)
    }

    pub fn get_request_details(
        source_path: &Path,
        request_name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<RequestDetails, RqError> {
        let rq_files = Self::get_rq_files_to_process(source_path, Some(request_name))?;

        let rq_file = rq_files
            .into_iter()
            .next()
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let request_file = crate::core::paths::clean_path(&rq_file.path);

        let req_with_vars = rq_file
            .requests
            .into_iter()
            .find(|r| r.request.name == request_name)
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let request_line = req_with_vars.request.line;
        let request_character = req_with_vars.request.character;

        // Context Setup
        // Check loaded variables and auth providers from imports
        let mut loaded_variables = rq_file.file_variables.clone();
        let mut loaded_auth_providers = rq_file.auth_providers.clone();
        let mut processed_files = std::collections::HashSet::new();
        processed_files.insert(rq_file.path.clone());

        let mut files_to_process = rq_file.imported_files.clone();

        // Simple iterative import loading (handling one level of depth or basic loops)
        // In a real implementation this should be robust against cycles and depth
        while let Some(file_path) = files_to_process.pop() {
            if processed_files.contains(&file_path) {
                continue;
            }
            processed_files.insert(file_path.clone());

            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    if let Ok(tokens) = crate::syntax::tokenize(&content) {
                        if let Ok(result) =
                            crate::syntax::analyze(&tokens, file_path.clone(), &content)
                        {
                            loaded_variables.extend(result.file_variables);
                            for (k, v) in result.auth_providers {
                                loaded_auth_providers.insert(k, v);
                            }
                            // Add nested imports
                            for import in result.imported_files {
                                if !processed_files.contains(&import) {
                                    files_to_process.push(import);
                                }
                            }
                        }
                    }
                }
            }
        }

        let env_vars = if let Some(env_name) = environment {
            if let Some(vars) = rq_file.environments.get(env_name) {
                vars.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let secret_vars = {
            crate::syntax::variables::load_secrets(source_path, environment)
                .map_err(|e| RqError::Generic(e.to_string()))?
        };

        let context = crate::syntax::variable_context::VariableContext::builder()
            .file_variables(loaded_variables)
            .environment_variables(env_vars)
            .secret_variables(secret_vars)
            .endpoint_variables(req_with_vars.endpoint_variables.clone())
            .request_variables(req_with_vars.request_variables.clone())
            .build();

        let mut working = req_with_vars.request;

        if !interpolate_variables {
            let (auth_name, auth_type) = if let Some(auth_name) = working.auth.as_deref() {
                if auth_name.trim().is_empty() {
                    (None, None)
                } else if let Some(auth_provider) = loaded_auth_providers.get(auth_name) {
                    (
                        Some(auth_name.to_string()),
                        Some(auth_provider.auth_type.as_str().to_string()),
                    )
                } else {
                    (Some(auth_name.to_string()), None)
                }
            } else {
                (None, None)
            };
            return Ok(RequestDetails {
                name: working.name,
                auth_name,
                auth_type,
                url: working.url,
                headers: working.headers,
                method: working.method.as_str().to_string(),
                body: working.body,
                file: request_file,
                line: request_line,
                character: request_character,
            });
        }

        let search_paths = Self::build_search_paths(
            &working,
            &rq_file.path,
            &rq_file.imported_files,
            source_path,
        );

        if let Some(header_var) = working.headers_var.clone() {
            let headers = std::mem::take(&mut working.headers);
            working.headers =
                Self::apply_headers_var(&header_var, headers, &context).map_err(|e| {
                    RqError::Validation(format!(
                        "Failed to expand headers variable '{header_var}': {e}"
                    ))
                })?;
        }

        let resolved = crate::syntax::resolve_variables(working, &context, &search_paths)
            .map_err(|e| RqError::Generic(e.to_string()))?;

        let (auth_name, auth_type) = if let Some(auth_name) = resolved.auth.as_deref() {
            if auth_name.trim().is_empty() {
                (None, None)
            } else if let Some(auth_provider) = loaded_auth_providers.get(auth_name) {
                (
                    Some(auth_name.to_string()),
                    Some(auth_provider.auth_type.as_str().to_string()),
                )
            } else {
                (Some(auth_name.to_string()), None)
            }
        } else {
            (None, None)
        };

        Ok(RequestDetails {
            name: resolved.name,
            auth_name,
            auth_type,
            url: resolved.url,
            headers: resolved.headers,
            method: resolved.method.as_str().to_string(),
            body: resolved.body,
            file: request_file,
            line: request_line,
            character: request_character,
        })
    }

    pub fn list_auth(
        source_path: &Path,
    ) -> Result<Vec<super::rq_client_models::AuthListEntry>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        if !source_path.is_dir() {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        let mut auth_map = HashMap::new();
        Self::collect_auth_entries(source_path, &mut auth_map)?;

        let mut auth_list: Vec<super::rq_client_models::AuthListEntry> = auth_map
            .into_iter()
            .map(|(name, (auth_type, _file, _line, _character))| {
                super::rq_client_models::AuthListEntry { name, auth_type }
            })
            .collect();
        auth_list.sort();

        Ok(auth_list)
    }

    pub fn get_auth_details(
        source_path: &Path,
        auth_name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<AuthDetails, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        if !source_path.is_dir() {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        let (mut auth_provider, file_path) = Self::find_auth_provider(source_path, auth_name)?
            .ok_or_else(|| {
                RqError::Validation(format!("Auth configuration '{auth_name}' not found"))
            })?;

        let auth_file = crate::core::paths::clean_path(&auth_provider.file_path);
        let auth_line = auth_provider.line;
        let auth_character = auth_provider.character;

        auth_provider.apply_defaults();

        if !interpolate_variables {
            let auth_type_str = auth_provider.auth_type.as_str().to_string();
            let fields: HashMap<String, String> = auth_provider
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect();
            return Ok((
                auth_name.to_string(),
                auth_type_str,
                fields,
                auth_file,
                auth_line,
                auth_character,
            ));
        }

        let (all_variables, imported_files) =
            crate::syntax::load_all_variables(&file_path, source_path, environment)
                .map_err(|e| RqError::Generic(e.to_string()))?;

        let mut source_files = vec![file_path.clone()];
        source_files.extend(imported_files);
        let env_path = source_path.join(".env");
        if env_path.exists() {
            source_files.push(env_path);
        }

        let context = crate::syntax::variable_context::VariableContext::builder()
            .file_variables(all_variables)
            .build();

        let config = crate::syntax::resolve_auth_provider(auth_provider, &context, &source_files)?;

        let auth_type_str = match config.auth_type {
            crate::syntax::auth::AuthType::Bearer => "bearer",
            crate::syntax::auth::AuthType::OAuth2AuthorizationCode => "oauth2_authorization_code",
            crate::syntax::auth::AuthType::OAuth2ClientCredentials => "oauth2_client_credentials",
            crate::syntax::auth::AuthType::OAuth2Implicit => "oauth2_implicit",
        };

        Ok((
            config.name.clone(),
            auth_type_str.to_string(),
            config
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect(),
            auth_file,
            auth_line,
            auth_character,
        ))
    }

    pub fn list_environments(source_path: &Path) -> Result<Vec<String>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut env_names = HashSet::new();

        if source_path.is_file() {
            if let Ok(rq_file) = RqFile::from_path(source_path) {
                for env_name in rq_file.environments.keys() {
                    env_names.insert(env_name.clone());
                }
            }
        } else if source_path.is_dir() {
            Self::collect_environment_names(source_path, &mut env_names)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        let mut env_list: Vec<String> = env_names.into_iter().collect();
        env_list.sort();

        Ok(env_list)
    }

    pub fn list_environments_with_locations(
        source_path: &Path,
    ) -> Result<Vec<super::rq_client_models::EnvironmentEntry>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut seen = HashSet::new();
        let mut entries: Vec<super::rq_client_models::EnvironmentEntry> = Vec::new();

        let mut paths = Vec::new();
        if source_path.is_file() {
            paths.push(source_path.to_path_buf());
        } else if source_path.is_dir() {
            Self::collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        for path in paths {
            if let Ok(rq_file) = RqFile::from_path(&path) {
                for (name, (file, line, character)) in &rq_file.environment_locations {
                    if seen.insert(name.clone()) {
                        entries.push(super::rq_client_models::EnvironmentEntry {
                            name: name.clone(),
                            file: crate::core::paths::clean_path_str(file).to_string(),
                            line: *line,
                            character: *character,
                        });
                    }
                }
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn get_environment(
        source_path: &Path,
        name: &str,
    ) -> Result<super::rq_client_models::EnvironmentEntry, RqError> {
        let entries = Self::list_environments_with_locations(source_path)?;
        entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| RqError::Validation(format!("Environment '{name}' not found")))
    }

    pub fn list_endpoints(
        source_path: &Path,
    ) -> Result<Vec<super::rq_client_models::EndpointEntry>, RqError> {
        let mut seen = HashSet::new();
        let mut entries: Vec<super::rq_client_models::EndpointEntry> = Vec::new();

        let paths: Vec<PathBuf> = if source_path.is_file() {
            let mut processed: HashSet<PathBuf> = HashSet::new();
            let mut to_process = vec![source_path.to_path_buf()];
            let mut file_paths = Vec::new();
            while let Some(path) = to_process.pop() {
                if processed.contains(&path) {
                    continue;
                }
                processed.insert(path.clone());
                let imports = RqFile::from_path_lenient(&path)
                    .map(|f| f.imported_files)
                    .unwrap_or_default();
                for import in imports {
                    if !processed.contains(&import) {
                        to_process.push(import);
                    }
                }
                file_paths.push(path);
            }
            file_paths
        } else {
            Self::collect_paths(source_path)?
        };

        for path in &paths {
            if let Some(rq_file) = RqFile::from_path_lenient(path) {
                for (name, ep) in &rq_file.endpoints {
                    if seen.insert(name.clone()) {
                        let file = ep
                            .source_path
                            .as_deref()
                            .map(crate::core::paths::clean_path_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| crate::core::paths::clean_path(&rq_file.path));
                        entries.push(super::rq_client_models::EndpointEntry {
                            name: name.clone(),
                            file,
                            line: ep.line,
                            character: ep.character,
                            is_template: ep.is_template,
                        });
                    }
                }
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn get_endpoint(
        source_path: &Path,
        name: &str,
    ) -> Result<super::rq_client_models::EndpointEntry, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut paths = Vec::new();
        if source_path.is_file() {
            paths.push(source_path.to_path_buf());
        } else if source_path.is_dir() {
            Self::collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        paths.sort();
        for path in &paths {
            if let Ok(rq_file) = RqFile::from_path(path) {
                if let Some(ep) = rq_file.endpoints.get(name) {
                    let file = ep
                        .source_path
                        .as_deref()
                        .map(crate::core::paths::clean_path_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| crate::core::paths::clean_path(&rq_file.path));
                    return Ok(super::rq_client_models::EndpointEntry {
                        name: name.to_string(),
                        file,
                        line: ep.line,
                        character: ep.character,
                        is_template: ep.is_template,
                    });
                }
            }
        }

        Err(RqError::Validation(format!("Endpoint '{name}' not found")))
    }

    pub fn list_variables(
        source_path: &Path,
        environment: Option<&str>,
    ) -> Result<Vec<super::rq_client_models::VariableEntry>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut paths: Vec<PathBuf> = if source_path.is_file() {
            let mut processed: HashSet<PathBuf> = HashSet::new();
            let mut to_process = vec![source_path.to_path_buf()];
            let mut file_paths = Vec::new();
            while let Some(path) = to_process.pop() {
                if processed.contains(&path) {
                    continue;
                }
                processed.insert(path.clone());
                let imports = RqFile::from_path_lenient(&path)
                    .map(|f| f.imported_files)
                    .unwrap_or_default();
                file_paths.push(path);
                for import in imports {
                    if !processed.contains(&import) {
                        to_process.push(import);
                    }
                }
            }
            file_paths
        } else if source_path.is_dir() {
            let mut ps = Vec::new();
            Self::collect_rq_paths(source_path, &mut ps)?;
            ps
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        };

        paths.sort();
        let mut seen = HashSet::new();
        let mut entries: Vec<super::rq_client_models::VariableEntry> = Vec::new();

        for path in &paths {
            if let Some(rq_file) = RqFile::from_path_lenient(path) {
                let let_values: HashMap<&str, &VariableValue> = rq_file
                    .file_variables
                    .iter()
                    .map(|v| (v.name.as_str(), &v.value))
                    .collect();

                if let Some(env_name) = environment {
                    if let Some(key_map) = rq_file.env_variable_locations.get(env_name) {
                        let env_values: HashMap<&str, &VariableValue> = rq_file
                            .environments
                            .get(env_name)
                            .map(|vars| vars.iter().map(|v| (v.name.as_str(), &v.value)).collect())
                            .unwrap_or_default();

                        for (var_name, (file, line, character)) in key_map {
                            if seen.insert(var_name.clone()) {
                                let value = env_values
                                    .get(var_name.as_str())
                                    .map(|v| v.display())
                                    .unwrap_or_default();
                                entries.push(super::rq_client_models::VariableEntry {
                                    name: var_name.clone(),
                                    value,
                                    file: crate::core::paths::clean_path_str(file).to_string(),
                                    line: *line,
                                    character: *character,
                                    source: format!("env:{env_name}"),
                                });
                            }
                        }
                    }
                }
                for (var_name, (file, line, character)) in &rq_file.let_variable_locations {
                    if seen.insert(var_name.clone()) {
                        let value = let_values
                            .get(var_name.as_str())
                            .map(|v| v.display())
                            .unwrap_or_default();
                        entries.push(super::rq_client_models::VariableEntry {
                            name: var_name.clone(),
                            value,
                            file: crate::core::paths::clean_path_str(file).to_string(),
                            line: *line,
                            character: *character,
                            source: "let".to_string(),
                        });
                    }
                }
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn get_variable(
        source_path: &Path,
        name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<super::rq_client_models::VariableEntry, RqError> {
        let entries = Self::list_variables(source_path, environment)?;
        let entry = entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| RqError::Validation(format!("Variable '{name}' not found")))?;

        if !interpolate_variables {
            return Ok(entry);
        }

        let mut paths = Vec::new();
        if source_path.is_file() {
            paths.push(source_path.to_path_buf());
        } else if source_path.is_dir() {
            Self::collect_rq_paths(source_path, &mut paths)?;
        }
        paths.sort();

        let mut all_vars: Vec<crate::syntax::Variable> = Vec::new();
        let mut target_raw: Option<VariableValue> = None;

        for path in &paths {
            if let Ok(rq_file) = RqFile::from_path(path) {
                for var in &rq_file.file_variables {
                    if target_raw.is_none() && var.name == name {
                        target_raw = Some(var.value.clone());
                    }
                    all_vars.push(var.clone());
                }
                if let Some(env_name) = environment {
                    if let Some(env_vars) = rq_file.environments.get(env_name) {
                        for var in env_vars {
                            if var.name == name {
                                target_raw = Some(var.value.clone());
                            }
                            all_vars.push(var.clone());
                        }
                    }
                }
            }
        }

        let secret_vars = crate::syntax::variables::load_secrets(source_path, environment)
            .map_err(|e| RqError::Generic(e.to_string()))?;

        let context = crate::syntax::variable_context::VariableContext::builder()
            .file_variables(all_vars)
            .secret_variables(secret_vars)
            .build();

        let raw_value = match target_raw {
            Some(VariableValue::String(s)) => s,
            Some(VariableValue::Reference(r)) => format!("{{{{{r}}}}}"),
            _ => return Ok(entry),
        };

        let mut source_files = paths;
        let env_path = source_path.join(".env");
        if env_path.exists() {
            source_files.push(env_path);
        }

        let resolved = crate::syntax::resolve_string(&raw_value, &context, &source_files)
            .map_err(|e| RqError::Generic(e.to_string()))?;

        Ok(super::rq_client_models::VariableEntry {
            value: resolved,
            ..entry
        })
    }

    pub fn list_variable_references(
        source_path: &Path,
        name: &str,
    ) -> Result<Vec<super::rq_client_models::ReferenceLocation>, RqError> {
        let paths = Self::collect_paths(source_path)?;
        let refs = crate::syntax::find_all_variable_references(&paths, name);
        Ok(refs
            .into_iter()
            .map(
                |(line, character, path)| super::rq_client_models::ReferenceLocation {
                    file: crate::core::paths::clean_path(&path),
                    line,
                    character,
                },
            )
            .collect())
    }

    pub fn list_endpoint_references(
        source_path: &Path,
        name: &str,
    ) -> Result<Vec<super::rq_client_models::ReferenceLocation>, RqError> {
        let paths = Self::collect_paths(source_path)?;
        let refs = crate::syntax::find_all_endpoint_references(&paths, name);
        Ok(refs
            .into_iter()
            .map(
                |(line, character, path)| super::rq_client_models::ReferenceLocation {
                    file: crate::core::paths::clean_path(&path),
                    line,
                    character,
                },
            )
            .collect())
    }

    fn collect_paths(source_path: &Path) -> Result<Vec<PathBuf>, RqError> {
        if !source_path.exists() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }
        let mut paths = Vec::new();
        if source_path.is_file() {
            paths.push(source_path.to_path_buf());
        } else if source_path.is_dir() {
            Self::collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }
        paths.sort();
        Ok(paths)
    }

    fn get_rq_files_to_process(
        source_path: &Path,
        request_name: Option<&str>,
    ) -> Result<Vec<RqFile>, RqError> {
        if source_path.is_file() {
            return Ok(vec![RqFile::from_path(source_path).map_err(|e| {
                if let Some(syntax_err) = e.downcast_ref::<crate::syntax::error::SyntaxError>() {
                    RqError::Syntax(syntax_err.clone())
                } else {
                    RqError::Generic(e.to_string())
                }
            })?]);
        }

        if !source_path.is_dir() {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        if let Some(request_name) = request_name {
            match Self::find_rq_file_with_request(source_path, request_name)? {
                Some(rq_file) => Ok(vec![rq_file]),
                None => Err(RqError::RequestNotFound(format!(
                    "Request '{}' not found in directory: {}",
                    request_name,
                    source_path.display()
                ))),
            }
        } else {
            let mut rq_files = Vec::new();
            Self::collect_rq_files_parsed(source_path, &mut rq_files)?;
            Ok(rq_files)
        }
    }

    fn collect_rq_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), RqError> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::collect_rq_paths(&path, paths)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                paths.push(path);
            }
        }
        Ok(())
    }

    fn find_rq_file_with_request(
        dir: &Path,
        request_name: &str,
    ) -> Result<Option<RqFile>, RqError> {
        let mut paths = Vec::new();
        Self::collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            match RqFile::from_path(&path) {
                Ok(rq_file) => {
                    if rq_file
                        .requests
                        .iter()
                        .any(|r| r.request.name == request_name)
                    {
                        return Ok(Some(rq_file));
                    }
                }
                Err(e) => {
                    if let Some(syntax_err) = e.downcast_ref::<crate::syntax::error::SyntaxError>()
                    {
                        return Err(RqError::Syntax(syntax_err.clone()));
                    }
                }
            }
        }
        Ok(None)
    }

    fn collect_rq_files_parsed(dir: &Path, rq_files: &mut Vec<RqFile>) -> Result<(), RqError> {
        let mut paths = Vec::new();
        Self::collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            match RqFile::from_path(&path) {
                Ok(rq_file) => rq_files.push(rq_file),
                Err(e) => eprintln!(
                    "Warning: Failed to parse {}: {}",
                    crate::core::paths::clean_path(&path),
                    e
                ),
            }
        }
        Ok(())
    }

    fn build_search_paths(
        request: &crate::syntax::parse_result::Request,
        file_path: &Path,
        imported_files: &[PathBuf],
        base_dir: &Path,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(src) = &request.source_path {
            paths.push(PathBuf::from(src));
        }
        paths.push(file_path.to_path_buf());
        paths.extend_from_slice(imported_files);
        for rf in &request.related_files {
            paths.push(PathBuf::from(rf));
        }
        let env_path = base_dir.join(".env");
        if env_path.exists() {
            paths.push(env_path);
        }
        paths
    }

    fn apply_headers_var(
        header_var: &str,
        existing_headers: Vec<(String, String)>,
        ctx: &crate::syntax::variable_context::VariableContext,
    ) -> Result<Vec<(String, String)>, String> {
        let mut merged = Self::expand_headers_var(header_var, ctx)?;
        for (ck, cv) in existing_headers {
            if let Some(i) = merged
                .iter()
                .position(|(ek, _)| ek.eq_ignore_ascii_case(&ck))
            {
                merged[i] = (ck, cv);
            } else {
                merged.push((ck, cv));
            }
        }
        Ok(merged)
    }

    fn expand_headers_var(
        name: &str,
        ctx: &crate::syntax::variable_context::VariableContext,
    ) -> Result<Vec<(String, String)>, String> {
        let mut all: Vec<&crate::syntax::variable_context::Variable> = Vec::new();
        all.extend(&ctx.file_variables);
        all.extend(&ctx.environment_variables);
        all.extend(&ctx.secret_variables);
        all.extend(&ctx.endpoint_variables);
        all.extend(&ctx.request_variables);
        all.extend(&ctx.cli_variables);
        if let Some(var) = all.into_iter().find(|v| v.name == name) {
            match &var.value {
                VariableValue::Json(_) | VariableValue::String(_) => {
                    return Err(format!(
                        "Variable '{name}' is not a headers object or array"
                    ));
                }
                VariableValue::Array(arr) => {
                    let mut out = Vec::new();
                    for item in arr {
                        if let Some((k, v)) = item.split_once(':') {
                            out.push((
                                k.trim().trim_matches('"').to_string(),
                                v.trim().trim_matches('"').to_string(),
                            ));
                        }
                    }
                    return Ok(out);
                }
                VariableValue::Headers(h) => {
                    return Ok(h.clone());
                }
                VariableValue::Reference(_) => {
                    // Should we resolve reference?
                    // For now, just ignore or error?
                    // The original code did nothing for Reference.
                    return Ok(Vec::new());
                }
                VariableValue::SystemFunction { .. } => {
                    return Ok(Vec::new());
                }
            }
        }
        Err(format!("Unresolved variable: '{name}'"))
    }

    fn load_secrets(&self, source_path: &Path) -> Result<Vec<Variable>, RqError> {
        let selected_env = self.config.environment.as_deref();
        crate::syntax::load_secrets(source_path, selected_env)
            .map_err(|e| RqError::Generic(e.to_string()))
    }

    fn parse_cli_variables(&self) -> Result<Vec<Variable>, RqError> {
        let cli_variables: Vec<Variable> = self
            .config
            .variables
            .iter()
            .filter_map(|kv| {
                if let Some(eq) = kv.find('=') {
                    let name = kv[..eq].trim();
                    let value = kv[eq + 1..].to_string();
                    if name.is_empty() {
                        eprintln!("Ignoring CLI variable with empty name: {kv}");
                        None
                    } else {
                        Some(Variable {
                            name: name.to_string(),
                            value: VariableValue::String(value),
                        })
                    }
                } else {
                    eprintln!("Ignoring CLI variable without '=': {kv}");
                    None
                }
            })
            .collect();

        Ok(cli_variables)
    }

    fn filter_requests(
        &self,
        requests: Vec<crate::syntax::parse_result::RequestWithVariables>,
    ) -> Vec<crate::syntax::parse_result::RequestWithVariables> {
        if let Some(ref request_name) = self.config.request_name {
            requests
                .into_iter()
                .filter(|r| r.request.name == *request_name)
                .collect()
        } else {
            requests
        }
    }

    fn collect_auth_entries(
        dir: &Path,
        auth_map: &mut HashMap<String, (String, String, usize, usize)>,
    ) -> Result<(), RqError> {
        let mut paths = Vec::new();
        Self::collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = RqFile::from_path(&path) {
                for (auth_name, provider) in rq_file.auth_providers.iter() {
                    auth_map.insert(
                        auth_name.clone(),
                        (
                            provider.auth_type.as_str().to_string(),
                            provider.file_path.to_string_lossy().to_string(),
                            provider.line,
                            provider.character,
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn find_auth_provider(
        dir: &Path,
        name: &str,
    ) -> Result<Option<(crate::syntax::auth::Config, PathBuf)>, RqError> {
        let mut paths = Vec::new();
        Self::collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = RqFile::from_path(&path) {
                if let Some(config) = rq_file.auth_providers.get(name) {
                    return Ok(Some((config.clone(), path)));
                }
            }
        }
        Ok(None)
    }

    fn collect_environment_names(
        dir: &Path,
        env_names: &mut HashSet<String>,
    ) -> Result<(), RqError> {
        let mut paths = Vec::new();
        Self::collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = RqFile::from_path(&path) {
                for env_name in rq_file.environments.keys() {
                    env_names.insert(env_name.clone());
                }
            }
        }
        Ok(())
    }
}
