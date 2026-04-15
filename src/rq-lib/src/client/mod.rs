pub mod models;
use crate::native;

use crate::client::models::{RequestDetails, RequestExecutionResult, RequestInfo};
use crate::error::RqError;
use crate::http::HttpClient;
use crate::logger::Logger;
use crate::syntax::{Fs, RqFile, SecretProvider, Variable, VariableValue};

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    fs: Arc<dyn Fs>,
    secrets: Arc<dyn SecretProvider>,
    http: Arc<dyn HttpClient>,
}

impl RqClient {
    pub fn new(
        fs: Arc<dyn Fs>,
        secrets: Arc<dyn SecretProvider>,
        http: Arc<dyn HttpClient>,
    ) -> Self {
        Self { fs, secrets, http }
    }

    pub async fn run(
        &self,
        source_path: &Path,
        request_name: Option<&str>,
        environment: Option<&str>,
        variables: &[String],
    ) -> Result<(Vec<RequestExecutionResult>, Vec<RqError>), RqError> {
        let (rq_files, parse_warnings) = self.get_rq_files_to_process(source_path, request_name)?;

        if rq_files.is_empty() {
            return Err(RqError::RequestNotFound(format!(
                "No .rq files found in directory: {}",
                source_path.display()
            )));
        }

        let mut all_results = Vec::new();

        for rq_file in rq_files {
            let env_vars = if let Some(env_name) = environment {
                if let Some(vars) = rq_file.environments.get(env_name) {
                    vars.clone()
                } else {
                    return Err(RqError::EnvironmentNotFound(env_name.to_string()));
                }
            } else {
                Vec::new()
            };

            let secret_vars = self.collect_secrets_for_env(source_path, environment);
            let cli_vars = Self::parse_cli_variables(variables)?;

            let filtered_requests = Self::filter_requests(rq_file.requests, request_name);

            if filtered_requests.is_empty() {
                if let Some(request_name) = request_name {
                    eprintln!("No request found with name '{request_name}'");
                } else {
                    eprintln!("No requests found in the file");
                }
                return Ok((vec![], parse_warnings));
            }

            if let Some(request_name) = request_name {
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
                                &*self.fs,
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

                let mut resolved_request = crate::syntax::resolve::resolve_variables(
                    working,
                    &context,
                    &search_paths,
                    &*self.fs,
                )?;

                if let Some(auth_name) = resolved_request.auth.as_deref() {
                    if !auth_name.trim().is_empty() {
                        if let Some(auth_provider) = rq_file.auth_providers.get(auth_name) {
                            let resolved_provider = crate::syntax::resolve::resolve_auth_provider(
                                auth_provider.clone(),
                                &context,
                                &search_paths,
                                &*self.fs,
                            )?;

                            let provider = crate::auth::get_executor(&resolved_provider.auth_type);

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

                let start_time = Instant::now();
                match self.http.execute(&resolved_request).await {
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
                        return Err(error);
                    }
                }
            }

            all_results.extend(results);
        }

        Ok((all_results, parse_warnings))
    }

    pub fn list_requests(
        &self,
        source_path: &Path,
    ) -> Result<(Vec<RequestInfo>, Vec<RqError>), RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let (rq_files, parse_errors) = if self.fs.is_file(source_path) {
            let rq_file = self.load_rq_file(source_path)?;
            (vec![rq_file], vec![])
        } else {
            let mut rq_files = Vec::new();
            let parse_errors = self.collect_rq_files_parsed(source_path, &mut rq_files)?;
            (rq_files, parse_errors)
        };

        let mut requests = Vec::new();
        for rq_file in rq_files {
            for req_with_vars in &rq_file.requests {
                let (endpoint_file, endpoint_line, endpoint_character) =
                    if let Some(ep_name) = &req_with_vars.request.endpoint {
                        if let Some(ep) = rq_file.endpoints.get(ep_name) {
                            let ep_file = ep
                                .source_path
                                .as_deref()
                                .map(crate::paths::clean_path_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| crate::paths::clean_path(&rq_file.path));
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
                    file: crate::paths::clean_path(&rq_file.path),
                    endpoint_file,
                    endpoint_line,
                    endpoint_character,
                });
            }
        }

        requests.sort_by(|a, b| a.name.cmp(&b.name));

        Ok((requests, parse_errors))
    }

    pub fn get_request_details(
        &self,
        source_path: &Path,
        request_name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<RequestDetails, RqError> {
        let (rq_files, _) = self.get_rq_files_to_process(source_path, Some(request_name))?;

        let rq_file = rq_files
            .into_iter()
            .next()
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let request_file = crate::paths::clean_path(&rq_file.path);

        let req_with_vars = rq_file
            .requests
            .into_iter()
            .find(|r| r.request.name == request_name)
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let request_line = req_with_vars.request.line;
        let request_character = req_with_vars.request.character;

        let mut loaded_variables = rq_file.file_variables.clone();
        let mut loaded_auth_providers = rq_file.auth_providers.clone();
        let mut processed_files = std::collections::HashSet::new();
        processed_files.insert(rq_file.path.clone());

        let mut files_to_process = rq_file.imported_files.clone();

        while let Some(file_path) = files_to_process.pop() {
            if processed_files.contains(&file_path) {
                continue;
            }
            processed_files.insert(file_path.clone());

            if self.fs.exists(&file_path) {
                if let Ok(content) = self.fs.read(&file_path) {
                    if let Ok(tokens) = crate::syntax::tokenize(&content) {
                        if let Ok(result) = crate::syntax::analysis::analyze(
                            &tokens,
                            file_path.clone(),
                            &content,
                            &*self.fs,
                        ) {
                            loaded_variables.extend(result.file_variables);
                            for (k, v) in result.auth_providers {
                                loaded_auth_providers.insert(k, v);
                            }
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

        let secret_vars = self.collect_secrets_for_env(source_path, environment);

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

        let resolved =
            crate::syntax::resolve::resolve_variables(working, &context, &search_paths, &*self.fs)
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
        &self,
        source_path: &Path,
    ) -> Result<Vec<crate::client::models::AuthListEntry>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let dir = if self.fs.is_file(source_path) {
            source_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
        } else {
            source_path
        };

        let mut auth_map = HashMap::new();
        self.collect_auth_entries(dir, &mut auth_map)?;

        let mut auth_list: Vec<crate::client::models::AuthListEntry> = auth_map
            .into_iter()
            .map(|(name, (auth_type, _file, _line, _character))| {
                crate::client::models::AuthListEntry { name, auth_type }
            })
            .collect();
        auth_list.sort();

        Ok(auth_list)
    }

    pub fn get_auth_details(
        &self,
        source_path: &Path,
        auth_name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<AuthDetails, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        if !self.fs.is_dir(source_path) {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        let (mut auth_provider, file_path) = self
            .find_auth_provider(source_path, auth_name)?
            .ok_or_else(|| {
                RqError::Validation(format!("Auth configuration '{auth_name}' not found"))
            })?;

        let auth_file = crate::paths::clean_path(&auth_provider.file_path);
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

        let content = self.fs.read(&file_path).map_err(RqError::Generic)?;
        let tokens =
            crate::syntax::tokenize(&content).map_err(|e| RqError::Generic(e.to_string()))?;
        let parse_result =
            crate::syntax::analysis::analyze(&tokens, file_path.clone(), &content, &*self.fs)
                .map_err(|e| RqError::Generic(e.to_string()))?;
        let imported_files = parse_result.imported_files.clone();
        let all_variables = crate::syntax::collect_all_variables(
            &parse_result,
            environment,
            &*self.secrets,
            source_path,
        )
        .map_err(|e| RqError::Generic(e.to_string()))?;

        let mut source_files = vec![file_path.clone()];
        source_files.extend(imported_files);
        let env_path = source_path.join(".env");
        if self.fs.exists(&env_path) {
            source_files.push(env_path);
        }

        let context = crate::syntax::variable_context::VariableContext::builder()
            .file_variables(all_variables)
            .build();

        let config = crate::syntax::resolve::resolve_auth_provider(
            auth_provider,
            &context,
            &source_files,
            &*self.fs,
        )?;

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

    pub fn list_environments(&self, source_path: &Path) -> Result<Vec<String>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut env_names = HashSet::new();

        if self.fs.is_file(source_path) {
            if let Ok(rq_file) = self.load_rq_file(source_path) {
                for env_name in rq_file.environments.keys() {
                    env_names.insert(env_name.clone());
                }
            }
        } else if self.fs.is_dir(source_path) {
            self.collect_environment_names(source_path, &mut env_names)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        let mut env_list: Vec<String> = env_names.into_iter().collect();
        env_list.sort();

        Ok(env_list)
    }

    pub fn list_environments_with_locations(
        &self,
        source_path: &Path,
    ) -> Result<Vec<crate::client::models::EnvironmentEntry>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut seen = HashSet::new();
        let mut entries: Vec<crate::client::models::EnvironmentEntry> = Vec::new();

        let mut paths = Vec::new();
        if self.fs.is_file(source_path) {
            paths.push(source_path.to_path_buf());
        } else if self.fs.is_dir(source_path) {
            self.collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        for path in paths {
            if let Ok(rq_file) = self.load_rq_file(&path) {
                for (name, (file, line, character)) in &rq_file.environment_locations {
                    if seen.insert(name.clone()) {
                        entries.push(crate::client::models::EnvironmentEntry {
                            name: name.clone(),
                            file: crate::paths::clean_path_str(file).to_string(),
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
        &self,
        source_path: &Path,
        name: &str,
    ) -> Result<crate::client::models::EnvironmentEntry, RqError> {
        let entries = self.list_environments_with_locations(source_path)?;
        entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| RqError::Validation(format!("Environment '{name}' not found")))
    }

    pub fn list_endpoints(
        &self,
        source_path: &Path,
    ) -> Result<Vec<crate::client::models::EndpointEntry>, RqError> {
        let mut seen = HashSet::new();
        let mut entries: Vec<crate::client::models::EndpointEntry> = Vec::new();

        let paths: Vec<PathBuf> = if self.fs.is_file(source_path) {
            let mut processed: HashSet<PathBuf> = HashSet::new();
            let mut to_process = vec![source_path.to_path_buf()];
            let mut file_paths = Vec::new();
            while let Some(path) = to_process.pop() {
                if processed.contains(&path) {
                    continue;
                }
                processed.insert(path.clone());
                let imports = self
                    .load_rq_file_lenient(&path)
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
            self.collect_paths(source_path)?
        };

        for path in &paths {
            if let Some(rq_file) = self.load_rq_file_lenient(path) {
                for (name, ep) in &rq_file.endpoints {
                    if seen.insert(name.clone()) {
                        let file = ep
                            .source_path
                            .as_deref()
                            .map(crate::paths::clean_path_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| crate::paths::clean_path(&rq_file.path));
                        entries.push(crate::client::models::EndpointEntry {
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
        &self,
        source_path: &Path,
        name: &str,
    ) -> Result<crate::client::models::EndpointEntry, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut paths = Vec::new();
        if self.fs.is_file(source_path) {
            paths.push(source_path.to_path_buf());
        } else if self.fs.is_dir(source_path) {
            self.collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }

        paths.sort();
        for path in &paths {
            if let Ok(rq_file) = self.load_rq_file(path) {
                if let Some(ep) = rq_file.endpoints.get(name) {
                    let file = ep
                        .source_path
                        .as_deref()
                        .map(crate::paths::clean_path_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| crate::paths::clean_path(&rq_file.path));
                    return Ok(crate::client::models::EndpointEntry {
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
        &self,
        source_path: &Path,
        environment: Option<&str>,
    ) -> Result<Vec<crate::client::models::VariableEntry>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut paths: Vec<PathBuf> = if self.fs.is_file(source_path) {
            let mut processed: HashSet<PathBuf> = HashSet::new();
            let mut to_process = vec![source_path.to_path_buf()];
            let mut file_paths = Vec::new();
            while let Some(path) = to_process.pop() {
                if processed.contains(&path) {
                    continue;
                }
                processed.insert(path.clone());
                let imports = self
                    .load_rq_file_lenient(&path)
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
        } else if self.fs.is_dir(source_path) {
            let mut ps = Vec::new();
            self.collect_rq_paths(source_path, &mut ps)?;
            ps
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        };

        paths.sort();
        let mut seen = HashSet::new();
        let mut entries: Vec<crate::client::models::VariableEntry> = Vec::new();

        for path in &paths {
            if let Some(rq_file) = self.load_rq_file_lenient(path) {
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
                                entries.push(crate::client::models::VariableEntry {
                                    name: var_name.clone(),
                                    value,
                                    file: crate::paths::clean_path_str(file).to_string(),
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
                        entries.push(crate::client::models::VariableEntry {
                            name: var_name.clone(),
                            value,
                            file: crate::paths::clean_path_str(file).to_string(),
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
        &self,
        source_path: &Path,
        name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
    ) -> Result<crate::client::models::VariableEntry, RqError> {
        let entries = self.list_variables(source_path, environment)?;
        let entry = entries
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| RqError::Validation(format!("Variable '{name}' not found")))?;

        if !interpolate_variables {
            return Ok(entry);
        }

        let mut paths = Vec::new();
        if self.fs.is_file(source_path) {
            paths.push(source_path.to_path_buf());
        } else if self.fs.is_dir(source_path) {
            self.collect_rq_paths(source_path, &mut paths)?;
        }
        paths.sort();

        let mut all_vars: Vec<crate::syntax::Variable> = Vec::new();
        let mut target_raw: Option<VariableValue> = None;

        for path in &paths {
            if let Ok(rq_file) = self.load_rq_file(path) {
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

        let secret_vars = self.collect_secrets_for_env(source_path, environment);

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
        if self.fs.exists(&env_path) {
            source_files.push(env_path);
        }

        let resolved =
            crate::syntax::resolve::resolve_string(&raw_value, &context, &source_files, &*self.fs)
                .map_err(|e| RqError::Generic(e.to_string()))?;

        Ok(crate::client::models::VariableEntry {
            value: resolved,
            ..entry
        })
    }

    pub fn list_variable_references(
        &self,
        source_path: &Path,
        name: &str,
    ) -> Result<Vec<crate::client::models::ReferenceLocation>, RqError> {
        let paths = self.collect_paths(source_path)?;
        let refs = crate::syntax::resolve::find_all_variable_references(&*self.fs, &paths, name);
        Ok(refs
            .into_iter()
            .map(
                |(line, character, path)| crate::client::models::ReferenceLocation {
                    file: crate::paths::clean_path(&path),
                    line,
                    character,
                },
            )
            .collect())
    }

    pub fn list_endpoint_references(
        &self,
        source_path: &Path,
        name: &str,
    ) -> Result<Vec<crate::client::models::ReferenceLocation>, RqError> {
        let paths = self.collect_paths(source_path)?;
        let refs = crate::syntax::resolve::find_all_endpoint_references(&*self.fs, &paths, name);
        Ok(refs
            .into_iter()
            .map(
                |(line, character, path)| crate::client::models::ReferenceLocation {
                    file: crate::paths::clean_path(&path),
                    line,
                    character,
                },
            )
            .collect())
    }

    pub fn check_path(&self, path: &Path, env_name: Option<&str>) -> Result<Vec<RqError>, RqError> {
        let source_path = if self.fs.is_file(path) {
            path.parent().unwrap_or(path)
        } else {
            path
        };

        if self.fs.is_file(path) {
            let mut errors = Vec::new();
            match self.load_rq_file(path) {
                Ok(rq_file) => {
                    errors.extend(self.check_variables(&rq_file, source_path, env_name));
                }
                Err(e) => {
                    errors.push(e);
                }
            }
            return Ok(errors);
        }
        if !self.fs.is_dir(path) {
            return Err(RqError::DirectoryNotFound(path.display().to_string()));
        }
        let mut rq_files = Vec::new();
        let mut errors = self.collect_rq_files_parsed(path, &mut rq_files)?;
        for rq_file in &rq_files {
            errors.extend(self.check_variables(rq_file, source_path, env_name));
        }
        Ok(errors)
    }

    fn load_rq_file(&self, path: &Path) -> Result<RqFile, RqError> {
        let canonical = self.fs.canonicalize(path).map_err(RqError::Generic)?;
        let content = self.fs.read(&canonical).map_err(RqError::Generic)?;
        RqFile::from_content(canonical, &content, &*self.fs).map_err(|e| {
            if let Some(syntax_err) = e.downcast_ref::<crate::syntax::error::SyntaxError>() {
                RqError::Syntax(syntax_err.clone())
            } else {
                RqError::Generic(e.to_string())
            }
        })
    }

    fn load_rq_file_lenient(&self, path: &Path) -> Option<RqFile> {
        let canonical = self.fs.canonicalize(path).ok()?;
        let content = self.fs.read(&canonical).ok()?;
        Some(RqFile::from_content_lenient(canonical, &content, &*self.fs))
    }

    fn collect_secrets_for_env(&self, source_path: &Path, env: Option<&str>) -> Vec<Variable> {
        let dir = if self.fs.is_dir(source_path) {
            source_path.to_path_buf()
        } else if let Some(parent) = source_path.parent() {
            if parent.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                parent.to_path_buf()
            }
        } else {
            PathBuf::from(".")
        };
        self.secrets.collect(&dir, env)
    }

    fn collect_paths(&self, source_path: &Path) -> Result<Vec<PathBuf>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }
        let mut paths = Vec::new();
        if self.fs.is_file(source_path) {
            paths.push(source_path.to_path_buf());
        } else if self.fs.is_dir(source_path) {
            self.collect_rq_paths(source_path, &mut paths)?;
        } else {
            return Err(RqError::NotADirectory(source_path.display().to_string()));
        }
        paths.sort();
        Ok(paths)
    }

    fn get_rq_files_to_process(
        &self,
        source_path: &Path,
        request_name: Option<&str>,
    ) -> Result<(Vec<RqFile>, Vec<RqError>), RqError> {
        if self.fs.is_file(source_path) {
            let rq_file = self.load_rq_file(source_path)?;
            return Ok((vec![rq_file], Vec::new()));
        }

        if !self.fs.is_dir(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        if let Some(request_name) = request_name {
            match self.find_rq_file_with_request(source_path, request_name)? {
                Some(rq_file) => Ok((vec![rq_file], Vec::new())),
                None => Err(RqError::RequestNotFound(format!(
                    "Request '{}' not found in directory: {}",
                    request_name,
                    source_path.display()
                ))),
            }
        } else {
            let mut rq_files = Vec::new();
            let parse_errors = self.collect_rq_files_parsed(source_path, &mut rq_files)?;
            Ok((rq_files, parse_errors))
        }
    }

    fn collect_rq_paths(&self, dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), RqError> {
        if !self.fs.is_dir(dir) {
            return Ok(());
        }
        for path in self.fs.read_dir(dir).map_err(RqError::Generic)? {
            if self.fs.is_dir(&path) {
                self.collect_rq_paths(&path, paths)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                paths.push(path);
            }
        }
        Ok(())
    }

    fn find_rq_file_with_request(
        &self,
        dir: &Path,
        request_name: &str,
    ) -> Result<Option<RqFile>, RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            match self.load_rq_file(&path) {
                Ok(rq_file) => {
                    if rq_file
                        .requests
                        .iter()
                        .any(|r| r.request.name == request_name)
                    {
                        return Ok(Some(rq_file));
                    }
                }
                Err(RqError::Syntax(e)) => return Err(RqError::Syntax(e)),
                Err(_) => {}
            }
        }
        Ok(None)
    }

    fn check_variables(
        &self,
        rq_file: &RqFile,
        source_path: &Path,
        env_name: Option<&str>,
    ) -> Vec<RqError> {
        let mut errors = Vec::new();

        let env_vars = if let Some(name) = env_name {
            rq_file.environments.get(name).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };

        let secret_vars = self.collect_secrets_for_env(source_path, env_name);

        let mut checked_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        let all_declared: Vec<_> = rq_file
            .file_variables
            .iter()
            .chain(env_vars.iter())
            .cloned()
            .collect();
        for v in &all_declared {
            checked_names.insert(v.name.clone());
        }
        let declared_search_paths: Vec<std::path::PathBuf> = std::iter::once(rq_file.path.clone())
            .chain(rq_file.imported_files.iter().cloned())
            .collect();
        let mut broken_var_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for e in crate::syntax::resolve::collect_declared_variable_errors(
            &all_declared,
            &secret_vars,
            &declared_search_paths,
            &*self.fs,
        ) {
            if let Some(var_name) = extract_unresolved_var_name(&e.message) {
                broken_var_names.insert(var_name);
            }
            errors.push(RqError::Syntax(e));
        }

        for req_with_vars in &rq_file.requests {
            let scoped_vars: Vec<_> = req_with_vars
                .endpoint_variables
                .iter()
                .chain(req_with_vars.request_variables.iter())
                .filter(|v| checked_names.insert(v.name.clone()))
                .cloned()
                .collect();
            let scoped_context: Vec<_> = all_declared
                .iter()
                .chain(secret_vars.iter())
                .cloned()
                .collect();
            for e in crate::syntax::resolve::collect_declared_variable_errors(
                &scoped_vars,
                &scoped_context,
                std::slice::from_ref(&rq_file.path),
                &*self.fs,
            ) {
                if extract_unresolved_var_name(&e.message)
                    .map(|n| broken_var_names.contains(&n))
                    .unwrap_or(false)
                {
                    continue;
                }
                errors.push(RqError::Syntax(e));
            }
            let context = crate::syntax::variable_context::VariableContext::builder()
                .file_variables(rq_file.file_variables.clone())
                .environment_variables(env_vars.clone())
                .secret_variables(secret_vars.clone())
                .endpoint_variables(req_with_vars.endpoint_variables.clone())
                .request_variables(req_with_vars.request_variables.clone())
                .build();

            let search_paths = Self::build_search_paths(
                &req_with_vars.request,
                &rq_file.path,
                &rq_file.imported_files,
                source_path,
            );

            for e in crate::syntax::resolve::collect_variable_errors(
                &req_with_vars.request,
                &context,
                &search_paths,
                &*self.fs,
            ) {
                if extract_unresolved_var_name(&e.message)
                    .map(|n| broken_var_names.contains(&n))
                    .unwrap_or(false)
                {
                    continue;
                }
                errors.push(RqError::Syntax(e));
            }
        }

        let base_context = crate::syntax::variable_context::VariableContext::builder()
            .file_variables(rq_file.file_variables.clone())
            .environment_variables(env_vars.clone())
            .secret_variables(secret_vars.clone())
            .build();

        for ep_def in rq_file.endpoints.values() {
            if !ep_def.has_requests {
                let ep_line_1 = ep_def.line + 1;
                let ep_col_1 = ep_def.character + 1;
                let source_path_arr = std::slice::from_ref(&rq_file.path);
                let mut error_index = 0usize;
                let mut check_ep = |s: &str| -> Option<RqError> {
                    if let Err(mut e) = crate::syntax::resolve::check_string(
                        s,
                        &base_context,
                        source_path_arr,
                        &*self.fs,
                    ) {
                        if e.line == 0 || e.line != ep_line_1 {
                            e.line = ep_line_1;
                            e.column = ep_col_1 + error_index;
                            if let Some(ref path) = ep_def.source_path {
                                e.file_path = Some(path.clone());
                            }
                        }
                        error_index += 1;
                        Some(RqError::Syntax(e))
                    } else {
                        None
                    }
                };
                let push_ep_error = |errors: &mut Vec<RqError>, e: RqError| {
                    if let RqError::Syntax(ref se) = e {
                        if extract_unresolved_var_name(&se.message)
                            .map(|n| broken_var_names.contains(&n))
                            .unwrap_or(false)
                        {
                            return;
                        }
                    }
                    errors.push(e);
                };
                if !ep_def.url.is_empty() {
                    if let Some(e) = check_ep(&ep_def.url) {
                        push_ep_error(&mut errors, e);
                    }
                }
                if let Some(ref var_name) = ep_def.headers_var {
                    let is_defined_headers = matches!(
                        base_context.as_map().get(var_name.as_str()),
                        Some(VariableValue::Headers(_))
                    );
                    if !is_defined_headers {
                        if let Some(e) = check_ep(&format!("{{{{{var_name}}}}}")) {
                            push_ep_error(&mut errors, e);
                        }
                    }
                }
                if let Some(ref qs) = ep_def.qs {
                    if let Some(e) = check_ep(qs) {
                        push_ep_error(&mut errors, e);
                    }
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        errors.retain(|e| {
            if let RqError::Syntax(se) = e {
                seen.insert((se.file_path.clone(), se.line, se.message.clone()))
            } else {
                true
            }
        });
        errors
    }

    fn collect_rq_files_parsed(
        &self,
        dir: &Path,
        rq_files: &mut Vec<RqFile>,
    ) -> Result<Vec<RqError>, RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        let mut parse_errors = Vec::new();
        for path in paths {
            match self.load_rq_file(&path) {
                Ok(rq_file) => rq_files.push(rq_file),
                Err(e) => parse_errors.push(e),
            }
        }
        Ok(parse_errors)
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
        paths.push(env_path);
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
        Self::expand_headers_var_inner(name, ctx, 0)
    }

    fn expand_headers_var_inner(
        name: &str,
        ctx: &crate::syntax::variable_context::VariableContext,
        depth: usize,
    ) -> Result<Vec<(String, String)>, String> {
        if depth > 20 {
            return Err(format!(
                "Recursion limit exceeded resolving headers variable '{name}': check for circular references"
            ));
        }
        let mut all: Vec<&crate::syntax::variable_context::Variable> = Vec::new();
        all.extend(&ctx.file_variables);
        all.extend(&ctx.environment_variables);
        all.extend(&ctx.secret_variables);
        all.extend(&ctx.endpoint_variables);
        all.extend(&ctx.request_variables);
        all.extend(&ctx.cli_variables);
        if let Some(var) = all.into_iter().rev().find(|v| v.name == name) {
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
                VariableValue::Reference(ref_name) => {
                    let resolved_name = ref_name.clone();
                    return Self::expand_headers_var_inner(&resolved_name, ctx, depth + 1);
                }
                VariableValue::SystemFunction { .. } => {
                    return Err(format!(
                        "Variable '{name}' is a function call and cannot be used as headers"
                    ));
                }
            }
        }
        Err(format!("Unresolved variable: '{name}'"))
    }

    fn parse_cli_variables(variables: &[String]) -> Result<Vec<Variable>, RqError> {
        let cli_variables: Vec<Variable> = variables
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
        requests: Vec<crate::syntax::parse_result::RequestWithVariables>,
        request_name: Option<&str>,
    ) -> Vec<crate::syntax::parse_result::RequestWithVariables> {
        if let Some(request_name) = request_name {
            requests
                .into_iter()
                .filter(|r| r.request.name == request_name)
                .collect()
        } else {
            requests
        }
    }

    fn collect_auth_entries(
        &self,
        dir: &Path,
        auth_map: &mut HashMap<String, (String, String, usize, usize)>,
    ) -> Result<(), RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = self.load_rq_file(&path) {
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
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<Option<(crate::syntax::auth::Config, PathBuf)>, RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = self.load_rq_file(&path) {
                if let Some(config) = rq_file.auth_providers.get(name) {
                    return Ok(Some((config.clone(), path)));
                }
            }
        }
        Ok(None)
    }

    fn collect_environment_names(
        &self,
        dir: &Path,
        env_names: &mut HashSet<String>,
    ) -> Result<(), RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        for path in paths {
            if let Ok(rq_file) = self.load_rq_file(&path) {
                for env_name in rq_file.environments.keys() {
                    env_names.insert(env_name.clone());
                }
            }
        }
        Ok(())
    }
}

impl Default for RqClient {
    fn default() -> Self {
        Self::new(
            Arc::new(native::NativeFs),
            Arc::new(native::NativeSecretProvider),
            Arc::new(native::ReqwestHttpClient),
        )
    }
}

fn extract_unresolved_var_name(message: &str) -> Option<String> {
    for prefix in &["Unresolved variable: '", "Variable '"] {
        if let Some(start) = message.find(prefix).map(|i| i + prefix.len()) {
            if let Some(end) = message[start..].find('\'') {
                return Some(message[start..start + end].to_string());
            }
        }
    }
    None
}
