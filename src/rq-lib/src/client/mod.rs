pub mod models;
#[cfg(feature = "native")]
use crate::native;

use crate::client::models::{
    PreparedAuth, PreparedRequest, RequestDetails, RequestExecutionResult, RequestInfo,
    RequestTarget, VariableEntry,
};
use crate::error::RqError;
use crate::http::HttpClient;
use crate::lint::{lint_rq_file, LintDiagnostic, LintScope, WorkspaceCollector, WorkspaceFile};
use crate::logger::Logger;
use crate::syntax::auth::Config as AuthConfig;
use crate::syntax::parse_result::{AuthLocation, RequestWithVariables};
use crate::syntax::resolve::SourcePosition;
use crate::syntax::variable_context::{VariableContext, VariableContextBuilder};
use crate::syntax::{Fs, Request, RqFile, SecretProvider, Variable, VariableValue};

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
        self.log_source(source_path);
        let (rq_files, mut warnings) = self.get_rq_files_to_process(source_path, request_name)?;

        if rq_files.is_empty() {
            return Err(RqError::RequestNotFound(format!(
                "No .rq files found in directory: {}",
                source_path.display()
            )));
        }

        let mut all_results = Vec::new();

        for mut rq_file in rq_files {
            let filtered_requests =
                Self::filter_requests(std::mem::take(&mut rq_file.requests), request_name);

            if filtered_requests.is_empty() {
                continue;
            }

            Self::require_environment(&rq_file, environment)?;
            let file_context = self.file_context(&rq_file, source_path, environment);
            let cli_vars = Self::parse_cli_variables(variables)?;

            Logger::debug(&format!(
                "* Running {} request(s) from {}",
                filtered_requests.len(),
                crate::paths::clean_path(&rq_file.path)
            ));

            let mut results = Vec::new();

            for req_with_vars in filtered_requests {
                let prepared = self.prepare(
                    &rq_file,
                    &file_context,
                    req_with_vars,
                    cli_vars.clone(),
                    source_path,
                    environment,
                )?;
                let authorized = Self::authorize(prepared).await?;
                Self::log_request(&authorized.request);

                let start_time = Instant::now();
                match self
                    .http
                    .execute(&authorized.request, authorized.timeout)
                    .await
                {
                    Ok(response) => {
                        let elapsed = start_time.elapsed();
                        Self::log_response(&response, elapsed.as_millis());

                        let mut request_headers = HashMap::new();
                        for (key, value) in &authorized.request.headers {
                            request_headers.insert(key.clone(), value.clone());
                        }

                        results.push(RequestExecutionResult {
                            request_name: authorized.request.name.clone(),
                            method: authorized.request.method.as_str().to_string(),
                            url: authorized.request.url.clone(),
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

        if all_results.is_empty() {
            if let Some(request_name) = request_name {
                return Err(RqError::RequestNotFound(request_name.to_string()));
            }
            warnings.push(self.no_requests_warning(source_path));
        }

        Ok((all_results, warnings))
    }

    pub fn list_requests(
        &self,
        source_path: &Path,
    ) -> Result<(Vec<RequestInfo>, Vec<RqError>), RqError> {
        self.log_source(source_path);
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
        target: RequestTarget,
        environment: Option<&str>,
        interpolate_variables: bool,
        skip_required_variables: bool,
        variables: &[String],
    ) -> Result<RequestDetails, RqError> {
        let source_path = target.source;
        self.log_source(source_path);
        let (rq_file, req_with_vars) = self.load_request(target)?;
        let request_file = crate::paths::clean_path(&rq_file.path);
        let request_line = req_with_vars.request.line;
        let request_character = req_with_vars.request.character;

        Self::require_environment(&rq_file, environment)?;
        let cli_vars = Self::parse_cli_variables(variables)?;

        if !interpolate_variables {
            let auth_name = Self::auth_name_of(&req_with_vars.request);
            let auth_type = auth_name
                .and_then(|name| rq_file.auth_providers.get(name))
                .map(|config| config.auth_type.as_str().to_string());
            let auth_name = auth_name.map(str::to_string);
            return Ok(Self::request_details(
                req_with_vars.request,
                auth_name,
                auth_type,
                request_file,
                request_line,
                request_character,
            ));
        }

        let RequestWithVariables {
            request,
            endpoint_variables,
            request_variables,
        } = req_with_vars;
        let file_context = self.file_context(&rq_file, source_path, environment);
        let mut context = Self::request_context(
            &file_context,
            endpoint_variables,
            request_variables,
            cli_vars,
        );
        if skip_required_variables {
            let defined = context.as_map();
            let placeholders = Self::empty_variables(
                request
                    .required_variables
                    .iter()
                    .filter(|name| !defined.contains_key(name.as_str())),
            );
            context.cli_variables.extend(placeholders);
        }

        let search_paths = Self::build_search_paths(
            &request,
            &rq_file.path,
            &rq_file.imported_files,
            &self.env_file_path(source_path),
        );
        let resolved = self.resolve_request(request, &context, &search_paths, environment)?;
        let auth_config = Self::auth_config_of(&rq_file, &resolved)?;
        let auth_name = auth_config.map(|config| config.name.clone());
        let auth_type = auth_config.map(|config| config.auth_type.as_str().to_string());

        Ok(Self::request_details(
            resolved,
            auth_name,
            auth_type,
            request_file,
            request_line,
            request_character,
        ))
    }

    pub fn prepare_request(
        &self,
        target: RequestTarget,
        environment: Option<&str>,
        variables: &[String],
    ) -> Result<PreparedRequest, RqError> {
        let source_path = target.source;
        self.log_source(source_path);
        let (rq_file, req_with_vars) = self.load_request(target)?;
        Self::require_environment(&rq_file, environment)?;
        let file_context = self.file_context(&rq_file, source_path, environment);
        let cli_vars = Self::parse_cli_variables(variables)?;
        self.prepare(
            &rq_file,
            &file_context,
            req_with_vars,
            cli_vars,
            source_path,
            environment,
        )
    }

    pub fn list_auth(
        &self,
        source_path: &Path,
    ) -> Result<Vec<crate::client::models::AuthListEntry>, RqError> {
        self.log_source(source_path);
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let mut auth_map = HashMap::new();
        if self.fs.is_file(source_path) {
            self.collect_auth_entries_from_paths(
                &self.collect_import_closure(source_path),
                &mut auth_map,
            );
        } else {
            self.collect_auth_entries(source_path, &mut auth_map)?;
        }

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
        self.log_source(source_path);
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

        let rq_file = self.load_rq_file(&file_path)?;
        Self::require_environment(&rq_file, environment)?;
        let context = self
            .file_context(&rq_file, source_path, environment)
            .build();

        let mut source_files = vec![file_path.clone()];
        source_files.extend(rq_file.imported_files.iter().cloned());
        let env_path = self.env_file_path(source_path);
        if self.fs.exists(&env_path) {
            source_files.push(env_path);
        }

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
        self.log_source(source_path);
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
            paths = self.discover_rq_files(source_path)?;
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
            self.collect_import_closure(source_path)
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
        scope_file: Option<&Path>,
    ) -> Result<crate::client::models::EndpointEntry, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let paths = self.resolution_paths(source_path, scope_file)?;
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
    ) -> Result<Vec<VariableEntry>, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let paths = self.resolution_paths(source_path, None)?;
        Ok(self.collect_variable_entries(&paths, environment))
    }

    pub fn get_variable(
        &self,
        source_path: &Path,
        name: &str,
        environment: Option<&str>,
        interpolate_variables: bool,
        scope_file: Option<&Path>,
    ) -> Result<VariableEntry, RqError> {
        if !self.fs.exists(source_path) {
            return Err(RqError::DirectoryNotFound(
                source_path.display().to_string(),
            ));
        }

        let (rq_file, entry) =
            self.find_variable_entry(source_path, scope_file, name, environment)?;

        if !interpolate_variables {
            return Ok(entry);
        }

        let context = self
            .file_context(&rq_file, source_path, environment)
            .build();

        let raw_value = match context.declared_value(name) {
            Some(VariableValue::String(s)) => s.clone(),
            Some(VariableValue::Reference(r)) => format!("{{{{{r}}}}}"),
            _ => return Ok(entry),
        };

        let mut source_files: Vec<PathBuf> = std::iter::once(rq_file.path.clone())
            .chain(rq_file.imported_files.iter().cloned())
            .collect();
        let env_path = self.env_file_path(source_path);
        if self.fs.exists(&env_path) {
            source_files.push(env_path);
        }

        let resolved =
            crate::syntax::resolve::resolve_string(&raw_value, &context, &source_files, &*self.fs)
                .map_err(|e| RqError::Generic(e.to_string()))?;

        Ok(VariableEntry {
            value: resolved,
            ..entry
        })
    }

    pub fn list_variable_references(
        &self,
        source_path: &Path,
        name: &str,
        scope_file: Option<&Path>,
        position: Option<(usize, usize)>,
    ) -> Result<Vec<crate::client::models::ReferenceLocation>, RqError> {
        let paths = self.reference_paths(source_path, scope_file, |closure| {
            self.variable_definition_file(closure, name)
        })?;
        let at = scope_file
            .zip(position)
            .map(|(file, (line, character))| SourcePosition {
                file,
                line,
                character,
            });
        let refs = crate::syntax::resolve::find_all_variable_references(
            &*self.fs,
            &paths,
            name,
            at.as_ref(),
        );
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
        scope_file: Option<&Path>,
    ) -> Result<Vec<crate::client::models::ReferenceLocation>, RqError> {
        let paths = self.reference_paths(source_path, scope_file, |closure| {
            self.endpoint_definition_file(closure, name)
        })?;
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
        self.log_source(path);
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
                    errors.extend(self.check_auth_references(&rq_file));
                }
                Err(e) => {
                    errors.push(e);
                }
            }
            let errors = Self::dedup_syntax_errors(errors);
            Self::log_check_summary(1, &errors);
            return Ok(errors);
        }
        if !self.fs.is_dir(path) {
            return Err(RqError::DirectoryNotFound(path.display().to_string()));
        }
        let mut rq_files = Vec::new();
        let mut errors = self.collect_rq_files_parsed(path, &mut rq_files)?;
        let file_count = rq_files.len() + errors.len();
        for rq_file in &rq_files {
            errors.extend(self.check_variables(rq_file, source_path, env_name));
            errors.extend(self.check_auth_references(rq_file));
        }
        let errors = Self::dedup_syntax_errors(errors);
        Self::log_check_summary(file_count, &errors);
        Ok(errors)
    }

    pub fn check_source(
        &self,
        source: &str,
        path: &Path,
        env_name: Option<&str>,
    ) -> Result<Vec<RqError>, RqError> {
        let source_path = path.parent().unwrap_or(path);
        let mut errors = Vec::new();
        match RqFile::from_content(path.to_path_buf(), source, &*self.fs) {
            Ok(rq_file) => {
                errors.extend(self.check_variables(&rq_file, source_path, env_name));
                errors.extend(self.check_auth_references(&rq_file));
            }
            Err(e) => errors.push(Self::map_parse_error(e)),
        }
        Ok(Self::dedup_syntax_errors(errors))
    }

    pub fn lint_path(&self, path: &Path, scope: LintScope) -> Result<Vec<LintDiagnostic>, RqError> {
        let (targets, workspace) = if self.fs.is_file(path) {
            let workspace = match scope {
                LintScope::SourceOnly => Vec::new(),
                LintScope::SourceDirectory => {
                    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
                    self.read_workspace_files(parent.unwrap_or(Path::new(".")))
                }
            };
            (vec![path.to_path_buf()], workspace)
        } else if self.fs.is_dir(path) {
            (
                self.discover_rq_files(path)?,
                self.read_workspace_files(path),
            )
        } else {
            return Err(RqError::DirectoryNotFound(path.display().to_string()));
        };
        let diagnostics: Vec<LintDiagnostic> = targets
            .iter()
            .flat_map(|target| self.lint_file(target, &workspace))
            .collect();
        Logger::debug(&format!(
            "* Linted {} file(s): {} warning(s)",
            targets.len(),
            diagnostics.len()
        ));
        Ok(diagnostics)
    }

    fn load_rq_file(&self, path: &Path) -> Result<RqFile, RqError> {
        let result = self.parse_rq_file(path);
        match &result {
            Ok(rq_file) => Self::log_parsed(rq_file),
            Err(e) => Logger::debug(&format!(
                "* Failed to load {}: {e}",
                crate::paths::clean_path(path)
            )),
        }
        result
    }

    fn log_parsed(rq_file: &RqFile) {
        if !Logger::is_debug_enabled() {
            return;
        }
        let imports: Vec<String> = rq_file
            .imported_files
            .iter()
            .map(|path| crate::paths::clean_path(path))
            .collect();
        let imports = if imports.is_empty() {
            "no imports".to_string()
        } else {
            format!("imports: {}", imports.join(", "))
        };
        Logger::debug(&format!(
            "* Parsed {} ({imports})",
            crate::paths::clean_path(&rq_file.path)
        ));
    }

    fn discover_rq_files(&self, dir: &Path) -> Result<Vec<PathBuf>, RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        Logger::debug(&format!(
            "* Found {} .rq file(s) in {}",
            paths.len(),
            crate::paths::clean_path(dir)
        ));
        Ok(paths)
    }

    fn log_variable_sources(
        request: &Request,
        context: &crate::syntax::variable_context::VariableContext,
        environment: Option<&str>,
    ) {
        if !Logger::is_debug_enabled() {
            return;
        }
        let mut pending: Vec<String> = Self::request_texts(request)
            .iter()
            .flat_map(|text| crate::syntax::resolve::referenced_variable_names(text))
            .chain(request.headers_var.clone())
            .collect();
        let values = context.as_map();
        let mut seen = HashSet::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let source = match (context.source_of(&name), environment) {
                (Some("env"), Some(env_name)) => format!("env:{env_name}"),
                (Some(level), _) => level.to_string(),
                (None, _) => "unresolved".to_string(),
            };
            Logger::debug(&format!("* Variable {name} from {source}"));
            if let Some(value) = values.get(name.as_str()) {
                pending.extend(Self::referenced_names_in_value(value));
            }
        }
    }

    fn request_texts(request: &Request) -> Vec<&str> {
        let mut texts = vec![request.url.as_str()];
        for (name, value) in &request.headers {
            texts.push(name);
            texts.push(value);
        }
        texts.extend(request.body.as_deref());
        texts.extend(request.timeout.as_deref());
        texts.extend(request.auth.as_deref());
        texts
    }

    fn referenced_names_in_value(value: &VariableValue) -> Vec<String> {
        match value {
            VariableValue::String(text) | VariableValue::Json(text) => {
                crate::syntax::resolve::referenced_variable_names(text)
            }
            VariableValue::Reference(name) => vec![name.clone()],
            VariableValue::Headers(pairs) => pairs
                .iter()
                .flat_map(|(_, v)| crate::syntax::resolve::referenced_variable_names(v))
                .collect(),
            VariableValue::SystemFunction { .. } => Vec::new(),
        }
    }

    fn log_check_summary(file_count: usize, errors: &[RqError]) {
        Logger::debug(&format!(
            "* Checked {file_count} file(s): {} error(s)",
            errors.len()
        ));
    }

    fn log_environment(env_name: &str, file: &Path, vars: &[Variable]) {
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        Logger::debug(&format!(
            "* Environment '{env_name}' for {}: {}",
            crate::paths::clean_path(file),
            names.join(", ")
        ));
    }

    fn log_source(&self, source_path: &Path) {
        let kind = if self.fs.is_file(source_path) {
            "file"
        } else if self.fs.is_dir(source_path) {
            "directory"
        } else {
            "not found"
        };
        Logger::debug(&format!(
            "* Source: {} ({kind})",
            crate::paths::clean_path(source_path)
        ));
    }

    fn parse_rq_file(&self, path: &Path) -> Result<RqFile, RqError> {
        let canonical = self.fs.canonicalize(path).map_err(RqError::Generic)?;
        let content = self.fs.read(&canonical).map_err(|e| {
            RqError::Generic(format!(
                "Failed to read {}: {e}",
                crate::paths::clean_path(&canonical)
            ))
        })?;
        RqFile::from_content(canonical, &content, &*self.fs).map_err(Self::map_parse_error)
    }

    fn read_workspace_files(&self, root: &Path) -> Vec<WorkspaceFile> {
        let mut paths = Vec::new();
        self.collect_readable_rq_paths(root, &mut paths, &mut HashSet::new());
        paths
            .into_iter()
            .filter_map(|path| {
                let canonical = self.fs.canonicalize(&path).ok()?;
                let content = self.fs.read(&canonical).ok()?;
                Some(WorkspaceFile::parse(&canonical, &content, &*self.fs))
            })
            .collect()
    }

    fn collect_readable_rq_paths(
        &self,
        dir: &Path,
        paths: &mut Vec<PathBuf>,
        visited: &mut HashSet<PathBuf>,
    ) {
        if !self.mark_visited(dir, visited) {
            return;
        }
        let Ok(entries) = self.fs.read_dir(dir) else {
            return;
        };
        for path in entries {
            if self.fs.is_dir(&path) {
                if !crate::paths::is_skipped_directory(&path) {
                    self.collect_readable_rq_paths(&path, paths, visited);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                paths.push(path);
            }
        }
    }

    fn lint_file(&self, target: &Path, workspace: &[WorkspaceFile]) -> Vec<LintDiagnostic> {
        let Ok(canonical) = self.fs.canonicalize(target) else {
            return Vec::new();
        };
        let Ok(source) = self.fs.read(&canonical) else {
            return Vec::new();
        };
        let Ok(rq_file) = RqFile::from_content(canonical.clone(), &source, &*self.fs) else {
            return Vec::new();
        };
        let mut collector = WorkspaceCollector::new(&rq_file);
        for file in workspace.iter().filter(|file| file.path != canonical) {
            collector.absorb_file(file);
        }
        let (workspace_requests, workspace_endpoints) = collector.finish();
        let display_path = crate::paths::clean_path(&canonical);
        lint_rq_file(
            &rq_file,
            &source,
            &display_path,
            &workspace_requests,
            &workspace_endpoints,
        )
        .diagnostics
        .into_iter()
        .map(|diagnostic| LintDiagnostic {
            file: diagnostic
                .file
                .clone()
                .or_else(|| Some(display_path.clone())),
            ..diagnostic
        })
        .collect()
    }

    fn map_parse_error(error: Box<dyn std::error::Error>) -> RqError {
        if let Some(syntax_err) = error.downcast_ref::<crate::syntax::error::SyntaxError>() {
            RqError::Syntax(syntax_err.clone())
        } else {
            RqError::Generic(error.to_string())
        }
    }

    fn load_rq_file_lenient(&self, path: &Path) -> Option<RqFile> {
        let canonical = self.fs.canonicalize(path).ok()?;
        let content = self.fs.read(&canonical).ok()?;
        Some(RqFile::from_content_lenient(canonical, &content, &*self.fs))
    }

    fn reject_mistyped_slots(
        request: &Request,
        context: &crate::syntax::variable_context::VariableContext,
    ) -> Result<(), RqError> {
        match crate::syntax::resolve::collect_slot_type_errors(
            &request.slot_bindings,
            request.source_path.as_deref(),
            context,
        )
        .into_iter()
        .next()
        {
            Some(error) => Err(RqError::Syntax(error)),
            None => Ok(()),
        }
    }

    fn apply_request_defaults(mut request: Request) -> Result<Request, RqError> {
        if !request
            .headers
            .iter()
            .any(|(k, _)| k.to_lowercase() == "user-agent")
        {
            request.headers.push((
                "user-agent".to_string(),
                format!("rq/{}", crate::version::app_version()),
            ));
        }

        let sends_json = request.body.is_some()
            && request.body_type == Some(crate::syntax::types::ValueType::Json);
        if sends_json
            && !request
                .headers
                .iter()
                .any(|(k, _)| k.to_lowercase() == "content-type")
        {
            request
                .headers
                .push(("content-type".to_string(), "application/json".to_string()));
        }

        Ok(request)
    }

    fn collect_variable_entries(
        &self,
        paths: &[PathBuf],
        environment: Option<&str>,
    ) -> Vec<VariableEntry> {
        let mut seen = HashSet::new();
        let mut entries: Vec<VariableEntry> = paths
            .iter()
            .filter_map(|path| self.load_rq_file_lenient(path))
            .flat_map(|rq_file| Self::variable_entries_of(&rq_file, environment))
            .filter(|entry| seen.insert(entry.name.clone()))
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    fn variable_entries_of(rq_file: &RqFile, environment: Option<&str>) -> Vec<VariableEntry> {
        let mut seen = HashSet::new();
        let mut entries: Vec<VariableEntry> = Vec::new();
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
                        entries.push(VariableEntry {
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
                entries.push(VariableEntry {
                    name: var_name.clone(),
                    value,
                    file: crate::paths::clean_path_str(file).to_string(),
                    line: *line,
                    character: *character,
                    source: "let".to_string(),
                });
            }
        }
        for (var_name, (file, line, character)) in &rq_file.required_variable_locations {
            if seen.insert(var_name.clone()) {
                entries.push(VariableEntry {
                    name: var_name.clone(),
                    value: String::new(),
                    file: crate::paths::clean_path_str(file).to_string(),
                    line: *line,
                    character: *character,
                    source: "required".to_string(),
                });
            }
        }
        entries
    }

    fn find_variable_entry(
        &self,
        source_path: &Path,
        scope_file: Option<&Path>,
        name: &str,
        environment: Option<&str>,
    ) -> Result<(RqFile, VariableEntry), RqError> {
        let candidates = match scope_file {
            Some(file) if self.fs.is_file(file) => vec![file.to_path_buf()],
            _ if self.fs.is_file(source_path) => vec![source_path.to_path_buf()],
            _ => self.collect_paths(source_path)?,
        };
        candidates
            .iter()
            .filter_map(|path| self.load_rq_file_lenient(path))
            .find_map(|rq_file| {
                Self::variable_entries_of(&rq_file, environment)
                    .into_iter()
                    .find(|e| e.name == name)
                    .map(|entry| (rq_file, entry))
            })
            .ok_or_else(|| RqError::Validation(format!("Variable '{name}' not found")))
    }

    fn load_request(
        &self,
        target: RequestTarget,
    ) -> Result<(RqFile, RequestWithVariables), RqError> {
        let request_name = target.name;
        let request_source = target
            .scope_file
            .filter(|file| self.fs.is_file(file))
            .unwrap_or(target.source);
        let (rq_files, _) = self.get_rq_files_to_process(request_source, Some(request_name))?;
        let mut rq_file = rq_files
            .into_iter()
            .next()
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;
        let req_with_vars =
            Self::filter_requests(std::mem::take(&mut rq_file.requests), Some(request_name))
                .into_iter()
                .next()
                .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;
        Ok((rq_file, req_with_vars))
    }

    fn prepare(
        &self,
        rq_file: &RqFile,
        file_context: &VariableContextBuilder,
        req_with_vars: RequestWithVariables,
        cli_vars: Vec<Variable>,
        source_path: &Path,
        environment: Option<&str>,
    ) -> Result<PreparedRequest, RqError> {
        let RequestWithVariables {
            request,
            endpoint_variables,
            request_variables,
        } = req_with_vars;
        Self::require_runtime_variables(&request, &cli_vars)?;
        let context = Self::request_context(
            file_context,
            endpoint_variables,
            request_variables,
            cli_vars,
        );
        let search_paths = Self::build_search_paths(
            &request,
            &rq_file.path,
            &rq_file.imported_files,
            &self.env_file_path(source_path),
        );
        let resolved = self.resolve_request(request, &context, &search_paths, environment)?;
        let (authorized, auth) = self.apply_auth(rq_file, resolved, &context, &search_paths)?;
        let request = Self::apply_request_defaults(authorized)?;
        let timeout = crate::http::request_timeout(&request)?;
        Ok(PreparedRequest {
            request,
            timeout,
            auth,
            context,
        })
    }

    fn require_runtime_variables(request: &Request, cli_vars: &[Variable]) -> Result<(), RqError> {
        let mut seen = HashSet::new();
        let missing: Vec<&str> = request
            .required_variables
            .iter()
            .filter(|name| !cli_vars.iter().any(|v| &v.name == *name))
            .map(String::as_str)
            .filter(|name| seen.insert(*name))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(RqError::Validation(format!(
            "Required variable(s) not set: {}",
            missing.join(", ")
        )))
    }

    fn resolve_request(
        &self,
        mut request: Request,
        context: &VariableContext,
        search_paths: &[PathBuf],
        environment: Option<&str>,
    ) -> Result<Request, RqError> {
        Self::reject_mistyped_slots(&request, context)?;
        if let Some(header_var) = request.headers_var.clone() {
            let headers = std::mem::take(&mut request.headers);
            request.headers =
                Self::apply_headers_var(&header_var, headers, context).map_err(|e| {
                    let (line, col, path) = crate::syntax::resolve::find_variable_location(
                        &*self.fs,
                        search_paths,
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
        Self::log_variable_sources(&request, context, environment);
        Ok(crate::syntax::resolve::resolve_variables(
            request,
            context,
            search_paths,
            &*self.fs,
        )?)
    }

    fn apply_auth(
        &self,
        rq_file: &RqFile,
        mut request: Request,
        context: &VariableContext,
        search_paths: &[PathBuf],
    ) -> Result<(Request, Option<PreparedAuth>), RqError> {
        let Some(declared) = Self::auth_config_of(rq_file, &request)? else {
            return Ok((request, None));
        };
        let config = crate::syntax::resolve::resolve_auth_provider(
            declared.clone(),
            context,
            search_paths,
            &*self.fs,
        )?;
        Logger::debug(&format!(
            "* Applying auth '{}' ({})",
            config.name,
            config.auth_type.as_str()
        ));
        let (headers, applied) = crate::auth::get_provider(&config.auth_type).known_token(
            &config,
            context,
            std::mem::take(&mut request.headers),
        );
        request.headers = headers;
        Ok((
            request,
            Some(PreparedAuth {
                config,
                token_required: !applied,
            }),
        ))
    }

    async fn authorize(mut prepared: PreparedRequest) -> Result<PreparedRequest, RqError> {
        let Some(auth) = prepared.auth.as_ref().filter(|auth| auth.token_required) else {
            return Ok(prepared);
        };
        let request = &mut prepared.request;
        let provider = crate::auth::get_provider(&auth.config.auth_type);
        let (url, headers) = provider
            .configure(
                &auth.config,
                &prepared.context,
                request.url.clone(),
                std::mem::take(&mut request.headers),
            )
            .await
            .map_err(|e| {
                RqError::Auth(format!("Configuration '{}' failed: {e}", auth.config.name))
            })?;
        request.url = url;
        request.headers = headers;
        Ok(prepared)
    }

    fn auth_name_of(request: &Request) -> Option<&str> {
        request
            .auth
            .as_deref()
            .filter(|name| !name.trim().is_empty())
    }

    fn auth_config_of<'a>(
        rq_file: &'a RqFile,
        request: &Request,
    ) -> Result<Option<&'a AuthConfig>, RqError> {
        let Some(auth_name) = Self::auth_name_of(request) else {
            return Ok(None);
        };
        rq_file
            .auth_providers
            .get(auth_name)
            .map(Some)
            .ok_or_else(|| {
                RqError::Validation(format!("Auth configuration '{auth_name}' not found"))
            })
    }

    fn request_details(
        request: Request,
        auth_name: Option<String>,
        auth_type: Option<String>,
        file: String,
        line: usize,
        character: usize,
    ) -> RequestDetails {
        RequestDetails {
            name: request.name,
            auth_name,
            auth_type,
            url: request.url,
            headers: request.headers,
            method: request.method.as_str().to_string(),
            body: request.body,
            timeout: request.timeout,
            required_variables: request.required_variables,
            file,
            line,
            character,
        }
    }

    fn require_environment(rq_file: &RqFile, environment: Option<&str>) -> Result<(), RqError> {
        let Some(env_name) = environment else {
            return Ok(());
        };
        let vars = rq_file
            .environments
            .get(env_name)
            .ok_or_else(|| RqError::EnvironmentNotFound(env_name.to_string()))?;
        Self::log_environment(env_name, &rq_file.path, vars);
        Ok(())
    }

    fn file_context(
        &self,
        rq_file: &RqFile,
        source_path: &Path,
        environment: Option<&str>,
    ) -> VariableContextBuilder {
        let secrets = self.collect_secrets_for_env(source_path, environment);
        VariableContext::for_file(rq_file, environment, secrets)
    }

    fn request_context(
        file_context: &VariableContextBuilder,
        endpoint_variables: Vec<Variable>,
        request_variables: Vec<Variable>,
        cli_variables: Vec<Variable>,
    ) -> VariableContext {
        file_context
            .clone()
            .endpoint_variables(endpoint_variables)
            .request_variables(request_variables)
            .cli_variables(cli_variables)
            .build()
    }

    fn empty_variables<'a>(names: impl IntoIterator<Item = &'a String>) -> Vec<Variable> {
        names
            .into_iter()
            .map(|name| Variable {
                name: name.clone(),
                value: VariableValue::String(String::new()),
            })
            .collect()
    }

    fn collect_secrets_for_env(&self, source_path: &Path, env: Option<&str>) -> Vec<Variable> {
        self.secrets.collect(&self.context_dir(source_path), env)
    }

    fn context_dir(&self, source_path: &Path) -> PathBuf {
        if self.fs.is_dir(source_path) {
            return source_path.to_path_buf();
        }
        match source_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        }
    }

    fn env_file_path(&self, source_path: &Path) -> PathBuf {
        self.context_dir(source_path).join(".env")
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
            paths = self.discover_rq_files(source_path)?;
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
        self.walk_rq_paths(dir, paths, &mut HashSet::new())
    }

    fn walk_rq_paths(
        &self,
        dir: &Path,
        paths: &mut Vec<PathBuf>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<(), RqError> {
        if !self.fs.is_dir(dir) || !self.mark_visited(dir, visited) {
            return Ok(());
        }
        for path in self.fs.read_dir(dir).map_err(RqError::Generic)? {
            if self.fs.is_dir(&path) {
                if crate::paths::is_skipped_directory(&path) {
                    Logger::debug(&format!(
                        "* Skipping directory {}",
                        crate::paths::clean_path(&path)
                    ));
                    continue;
                }
                self.walk_rq_paths(&path, paths, visited)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                paths.push(path);
            }
        }
        Ok(())
    }

    fn mark_visited(&self, dir: &Path, visited: &mut HashSet<PathBuf>) -> bool {
        let key = self
            .fs
            .canonicalize(dir)
            .unwrap_or_else(|_| dir.to_path_buf());
        if visited.insert(key) {
            return true;
        }
        Logger::debug(&format!(
            "* Skipping already visited directory {}",
            crate::paths::clean_path(dir)
        ));
        false
    }

    fn find_rq_file_with_request(
        &self,
        dir: &Path,
        request_name: &str,
    ) -> Result<Option<RqFile>, RqError> {
        for path in self.discover_rq_files(dir)? {
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

        let file_context = self.file_context(rq_file, source_path, env_name);
        let base_context = file_context.clone().build();
        let secret_vars = &base_context.secret_variables;

        let mut checked_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        let all_declared: Vec<_> = base_context
            .file_variables
            .iter()
            .chain(base_context.environment_variables.iter())
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
            secret_vars,
            &declared_search_paths,
            &*self.fs,
        ) {
            if let Some(var_name) = extract_unresolved_var_name(&e.message) {
                broken_var_names.insert(var_name);
            }
            errors.push(RqError::Syntax(e));
        }

        let mut endpoint_reported: std::collections::HashSet<(Option<String>, String)> =
            std::collections::HashSet::new();

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
            let required_placeholders =
                Self::empty_variables(&req_with_vars.request.required_variables);
            let context = Self::request_context(
                &file_context,
                req_with_vars.endpoint_variables.clone(),
                req_with_vars.request_variables.clone(),
                required_placeholders,
            );

            let search_paths = Self::build_search_paths(
                &req_with_vars.request,
                &rq_file.path,
                &rq_file.imported_files,
                &self.env_file_path(source_path),
            );

            let owning_endpoint = req_with_vars
                .request
                .endpoint
                .as_ref()
                .and_then(|name| rq_file.endpoints.get(name));

            for e in crate::syntax::resolve::collect_variable_errors(
                &req_with_vars.request,
                owning_endpoint,
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
                let ep_key = (req_with_vars.request.endpoint.clone(), e.message.clone());
                if req_with_vars.request.endpoint.is_some() && !endpoint_reported.insert(ep_key) {
                    continue;
                }
                errors.push(RqError::Syntax(e));
            }
        }

        for ep_def in rq_file.endpoints.values() {
            let ep_context = file_context
                .clone()
                .endpoint_variables(ep_def.variables.clone())
                .build();
            let ep_type_errors = crate::syntax::resolve::collect_slot_type_errors(
                &ep_def.slot_bindings,
                ep_def.source_path.as_deref(),
                &ep_context,
            );
            let ep_mistyped = crate::syntax::resolve::mistyped_variable_names(
                &ep_def.slot_bindings,
                &ep_type_errors,
            );
            for e in ep_type_errors {
                errors.push(RqError::Syntax(e));
            }

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
                        if ep_mistyped
                            .iter()
                            .any(|name| e.message.contains(name.as_str()))
                        {
                            return None;
                        }
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

    fn check_auth_references(&self, rq_file: &RqFile) -> Vec<RqError> {
        let file = rq_file.path.to_string_lossy().to_string();
        let endpoint_references = rq_file
            .endpoints
            .values()
            .filter_map(|endpoint| Self::auth_reference(&endpoint.auth, &endpoint.auth_location));
        let request_references = rq_file
            .requests
            .iter()
            .filter_map(|req| Self::auth_reference(&req.request.auth, &req.request.auth_location));

        let mut references: Vec<(&str, &AuthLocation)> = endpoint_references
            .chain(request_references)
            .filter(|(auth_name, location)| {
                location.file == file && !rq_file.auth_providers.contains_key(*auth_name)
            })
            .collect();
        references.sort_by_key(|(_, location)| (location.line, location.character));

        let mut reported = std::collections::HashSet::new();
        references
            .into_iter()
            .filter(|(auth_name, location)| {
                reported.insert((location.line, location.character, *auth_name))
            })
            .map(|(auth_name, location)| {
                RqError::Syntax(crate::syntax::error::SyntaxError::with_file(
                    format!("Auth configuration '{auth_name}' not found"),
                    location.line + 1,
                    location.character + 1,
                    0..0,
                    crate::paths::clean_path(&rq_file.path),
                ))
            })
            .collect()
    }

    fn auth_reference<'a>(
        auth: &'a Option<String>,
        location: &'a Option<AuthLocation>,
    ) -> Option<(&'a str, &'a AuthLocation)> {
        let auth_name = auth.as_deref()?;
        if auth_name.trim().is_empty() || auth_name.contains("{{") {
            return None;
        }
        Some((auth_name, location.as_ref()?))
    }

    fn collect_rq_files_parsed(
        &self,
        dir: &Path,
        rq_files: &mut Vec<RqFile>,
    ) -> Result<Vec<RqError>, RqError> {
        let paths = self.discover_rq_files(dir)?;
        let mut parse_errors = Vec::new();
        for path in paths {
            match self.load_rq_file(&path) {
                Ok(rq_file) => rq_files.push(rq_file),
                Err(e) => parse_errors.push(e),
            }
        }
        Ok(parse_errors)
    }

    fn dedup_syntax_errors(errors: Vec<RqError>) -> Vec<RqError> {
        let mut seen = HashSet::new();
        errors
            .into_iter()
            .filter(|error| match error {
                RqError::Syntax(e) => {
                    seen.insert((e.file_path.clone(), e.line, e.column, e.message.clone()))
                }
                _ => true,
            })
            .collect()
    }

    fn build_search_paths(
        request: &crate::syntax::parse_result::Request,
        file_path: &Path,
        imported_files: &[PathBuf],
        env_file: &Path,
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
        paths.push(env_file.to_path_buf());
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
                    return Err(format!("Variable '{name}' is not a headers map"));
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
        variables
            .iter()
            .map(|kv| Self::parse_cli_variable(kv))
            .collect()
    }

    fn parse_cli_variable(kv: &str) -> Result<Variable, RqError> {
        let (name, value) = kv.split_once('=').ok_or_else(|| {
            RqError::Validation(format!("Invalid variable '{kv}': expected NAME=VALUE"))
        })?;
        let name = name.trim();
        if name.is_empty() {
            return Err(RqError::Validation(format!(
                "Invalid variable '{kv}': name is empty"
            )));
        }
        Ok(Variable {
            name: name.to_string(),
            value: VariableValue::String(value.to_string()),
        })
    }

    fn log_request(request: &Request) {
        if !Logger::is_debug_enabled() {
            return;
        }
        Logger::debug(&format!("> {} {}", request.method.as_str(), request.url));
        for (name, value) in &request.headers {
            Logger::debug(&format!(
                "> {name}: {}",
                Logger::mask_header_value(name, value)
            ));
        }
        if let Some(body) = &request.body {
            Logger::debug(">");
            Logger::debug(&format!("> {body}"));
        }
    }

    fn log_response(response: &crate::http::HttpResponse, elapsed_ms: u128) {
        if !Logger::is_debug_enabled() {
            return;
        }
        let reason = reqwest::StatusCode::from_u16(response.status)
            .ok()
            .and_then(|status| status.canonical_reason())
            .map(|reason| format!(" {reason}"))
            .unwrap_or_default();
        Logger::debug(&format!("< {}{reason} ({elapsed_ms} ms)", response.status));
        let mut headers: Vec<_> = response.headers.iter().collect();
        headers.sort();
        for (name, value) in headers {
            Logger::debug(&format!(
                "< {name}: {}",
                Logger::mask_header_value(name, value)
            ));
        }
    }

    fn no_requests_warning(&self, source_path: &Path) -> RqError {
        let location = if self.fs.is_file(source_path) {
            "file"
        } else {
            "directory"
        };
        RqError::Generic(format!("No requests found in the {location}"))
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

    fn collect_import_closure(&self, source_path: &Path) -> Vec<PathBuf> {
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
    }

    fn resolution_paths(
        &self,
        source_path: &Path,
        scope_file: Option<&Path>,
    ) -> Result<Vec<PathBuf>, RqError> {
        match scope_file {
            Some(file) if self.fs.is_file(file) => Ok(self.import_closure_sorted(file)),
            _ if self.fs.is_file(source_path) => Ok(self.import_closure_sorted(source_path)),
            _ => self.collect_paths(source_path),
        }
    }

    fn reference_paths(
        &self,
        source_path: &Path,
        scope_file: Option<&Path>,
        definition_file: impl Fn(&[PathBuf]) -> Option<PathBuf>,
    ) -> Result<Vec<PathBuf>, RqError> {
        let candidates = self.collect_paths(source_path)?;
        let Some(file) = scope_file.filter(|f| self.fs.is_file(f)) else {
            return Ok(candidates);
        };

        let closure = self.import_closure_sorted(file);
        let Some(anchor) = definition_file(&closure).or_else(|| self.canonical(file)) else {
            return Ok(closure);
        };

        let mut paths: Vec<PathBuf> = candidates
            .into_iter()
            .filter(|candidate| {
                self.canonical(candidate) == Some(anchor.clone())
                    || self
                        .import_closure_sorted(candidate)
                        .iter()
                        .any(|imported| self.canonical(imported) == Some(anchor.clone()))
            })
            .collect();

        if paths.is_empty() {
            return Ok(closure);
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn variable_definition_file(&self, paths: &[PathBuf], name: &str) -> Option<PathBuf> {
        for path in paths {
            let Some(rq_file) = self.load_rq_file_lenient(path) else {
                continue;
            };
            let location = rq_file
                .let_variable_locations
                .get(name)
                .or_else(|| rq_file.required_variable_locations.get(name))
                .or_else(|| {
                    rq_file
                        .env_variable_locations
                        .values()
                        .find_map(|key_map| key_map.get(name))
                });
            if let Some((file, _, _)) = location {
                return Some(PathBuf::from(file));
            }
        }
        None
    }

    fn endpoint_definition_file(&self, paths: &[PathBuf], name: &str) -> Option<PathBuf> {
        for path in paths {
            let Some(rq_file) = self.load_rq_file_lenient(path) else {
                continue;
            };
            if let Some(endpoint) = rq_file.endpoints.get(name) {
                return Some(
                    endpoint
                        .source_path
                        .as_deref()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| rq_file.path.clone()),
                );
            }
        }
        None
    }

    fn import_closure_sorted(&self, file: &Path) -> Vec<PathBuf> {
        let mut paths = self.collect_import_closure(file);
        paths.sort();
        paths.dedup();
        paths
    }

    fn canonical(&self, path: &Path) -> Option<PathBuf> {
        self.fs.canonicalize(path).ok()
    }

    fn collect_auth_entries(
        &self,
        dir: &Path,
        auth_map: &mut HashMap<String, (String, String, usize, usize)>,
    ) -> Result<(), RqError> {
        let mut paths = Vec::new();
        self.collect_rq_paths(dir, &mut paths)?;
        self.collect_auth_entries_from_paths(&paths, auth_map);
        Ok(())
    }

    fn collect_auth_entries_from_paths(
        &self,
        paths: &[PathBuf],
        auth_map: &mut HashMap<String, (String, String, usize, usize)>,
    ) {
        for path in paths {
            if let Some(rq_file) = self.load_rq_file_lenient(path) {
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

#[cfg(feature = "native")]
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

#[cfg(all(test, feature = "native"))]
mod check_source_tests {
    use super::RqClient;

    #[test]
    fn check_path_names_the_file_it_cannot_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("invalid_utf8.rq");
        std::fs::write(&file, b"rq get(\"http://localhost/\xff\");\n").expect("write");
        let target = RqClient::default()
            .check_path(&file, None)
            .expect("check_path failed");
        assert!(target[0].to_string().contains("invalid_utf8.rq"));
    }

    #[test]
    fn check_source_accepts_clean_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RqClient::default()
            .check_source(
                "rq basic(\"http://localhost:8080/get\");\n",
                &dir.path().join("draft.rq"),
                None,
            )
            .expect("check_source failed");
        assert!(target.is_empty());
    }

    #[test]
    fn check_source_reports_syntax_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RqClient::default()
            .check_source(
                "rq basic(\"http://localhost:8080/get\"\n",
                &dir.path().join("draft.rq"),
                None,
            )
            .expect("check_source failed");
        assert_eq!(target.len(), 1);
    }

    #[test]
    fn check_source_resolves_import_relative_to_draft_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("shared.rq"),
            "env local {\n    base_url: \"http://localhost:8080\",\n}\n",
        )
        .expect("write shared");
        let target = RqClient::default()
            .check_source(
                "import \"shared\";\n\nrq list(\"{{base_url}}/users\");\n",
                &dir.path().join("draft.rq"),
                Some("local"),
            )
            .expect("check_source failed");
        assert!(target.is_empty(), "expected no errors, got {target:?}");
    }

    #[test]
    fn check_source_reports_unresolvable_import() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RqClient::default()
            .check_source(
                "import \"missing\";\n\nrq list(\"http://localhost:8080/users\");\n",
                &dir.path().join("draft.rq"),
                None,
            )
            .expect("check_source failed");
        assert_eq!(target.len(), 1);
        assert!(target[0].to_string().contains("missing"));
    }

    #[test]
    fn check_source_reports_unknown_auth_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RqClient::default()
            .check_source(
                "[auth(\"ghost\")]\nrq basic(\"http://localhost:8080/get\");\n",
                &dir.path().join("draft.rq"),
                None,
            )
            .expect("check_source failed");
        assert_eq!(target.len(), 1);
        assert!(target[0]
            .to_string()
            .contains("Auth configuration 'ghost' not found"));
    }

    #[test]
    fn check_source_accepts_auth_reference_declared_in_draft() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RqClient::default()
            .check_source(
                "auth token(auth_type.bearer) {\n    token: \"secret\",\n}\n\n[auth(\"token\")]\nrq basic(\"http://localhost:8080/get\");\n",
                &dir.path().join("draft.rq"),
                None,
            )
            .expect("check_source failed");
        assert!(target.is_empty(), "expected no errors, got {target:?}");
    }

    #[test]
    fn check_source_does_not_write_draft_to_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let draft = dir.path().join("draft.rq");
        RqClient::default()
            .check_source("rq basic(\"http://localhost:8080/get\");\n", &draft, None)
            .expect("check_source failed");
        assert!(!draft.exists());
    }
}

#[cfg(all(test, feature = "native"))]
mod scope_file_tests {
    use super::RqClient;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn workspace() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("image")).expect("image dir");
        std::fs::create_dir_all(root.join("inma")).expect("inma dir");

        std::fs::write(
            root.join("_shared.rq"),
            "let base_url = \"https://api.example.com\";\n",
        )
        .expect("_shared.rq");

        std::fs::write(
            root.join("image/static.rq"),
            "import \"../_shared\";\n\nlet collection_id = \"static_collection\";\nlet only_static = \"static_only\";\n\nep inma_base(base_url);\n\nrq static(\"{{base_url}}/image/{{collection_id}}/{{only_static}}\");\n",
        )
        .expect("image/static.rq");

        std::fs::write(
            root.join("inma/_shared.rq"),
            "import \"../_shared\";\n\nlet collection_id = \"inma_collection\";\n\nep inma_base(base_url);\n",
        )
        .expect("inma/_shared.rq");

        std::fs::write(
            root.join("inma/collections.rq"),
            "import \"_shared\";\n\nep collections<inma_base>(\"/collections\") {\n    rq get(\"/{{collection_id}}\");\n}\n",
        )
        .expect("inma/collections.rq");

        dir
    }

    fn ends_with(actual: &str, expected: &str) -> bool {
        Path::new(actual).ends_with(PathBuf::from(expected))
    }

    #[test]
    fn scoped_variable_resolves_through_the_import_chain() {
        let dir = workspace();
        let scope = dir.path().join("inma/collections.rq");

        let target = RqClient::default()
            .get_variable(dir.path(), "collection_id", None, false, Some(&scope))
            .expect("get_variable failed");

        assert!(
            ends_with(&target.file, "inma/_shared.rq"),
            "expected inma/_shared.rq, got {}",
            target.file
        );
    }

    #[test]
    fn scoped_variable_follows_the_scope_file() {
        let dir = workspace();
        let scope = dir.path().join("image/static.rq");

        let target = RqClient::default()
            .get_variable(dir.path(), "collection_id", None, false, Some(&scope))
            .expect("get_variable failed");

        assert!(
            ends_with(&target.file, "image/static.rq"),
            "expected image/static.rq, got {}",
            target.file
        );
    }

    #[test]
    fn scoped_variable_lookup_rejects_an_unreachable_definition() {
        let dir = workspace();
        let scope = dir.path().join("inma/collections.rq");

        let target =
            RqClient::default().get_variable(dir.path(), "only_static", None, false, Some(&scope));

        assert!(target.is_err());
    }

    #[test]
    fn unscoped_variable_lookup_spans_the_whole_directory() {
        let dir = workspace();

        let target = RqClient::default()
            .get_variable(dir.path(), "only_static", None, false, None)
            .expect("get_variable failed");

        assert!(
            ends_with(&target.file, "image/static.rq"),
            "expected image/static.rq, got {}",
            target.file
        );
    }

    #[test]
    fn scoped_variable_references_exclude_unreachable_files() {
        let dir = workspace();
        let scope = dir.path().join("inma/collections.rq");

        let target = RqClient::default()
            .list_variable_references(dir.path(), "collection_id", Some(&scope), None)
            .expect("list_variable_references failed");

        assert!(
            !target.iter().any(|r| ends_with(&r.file, "image/static.rq")),
            "image/static.rq must not be in scope, got {target:?}"
        );
        assert!(target
            .iter()
            .any(|r| ends_with(&r.file, "inma/collections.rq")));
    }

    #[test]
    fn unscoped_variable_references_span_the_whole_directory() {
        let dir = workspace();

        let target = RqClient::default()
            .list_variable_references(dir.path(), "collection_id", None, None)
            .expect("list_variable_references failed");

        assert!(target.iter().any(|r| ends_with(&r.file, "image/static.rq")));
    }

    #[test]
    fn scoped_endpoint_resolves_through_the_import_chain() {
        let dir = workspace();
        let scope = dir.path().join("inma/collections.rq");

        let target = RqClient::default()
            .get_endpoint(dir.path(), "inma_base", Some(&scope))
            .expect("get_endpoint failed");

        assert!(
            ends_with(&target.file, "inma/_shared.rq"),
            "expected inma/_shared.rq, got {}",
            target.file
        );
    }

    #[test]
    fn scoped_endpoint_references_exclude_unreachable_files() {
        let dir = workspace();
        let scope = dir.path().join("inma/collections.rq");

        let target = RqClient::default()
            .list_endpoint_references(dir.path(), "inma_base", Some(&scope))
            .expect("list_endpoint_references failed");

        assert!(
            !target.iter().any(|r| ends_with(&r.file, "image/static.rq")),
            "image/static.rq must not be in scope, got {target:?}"
        );
        assert!(target.iter().any(|r| ends_with(&r.file, "inma/_shared.rq")));
    }
}

#[cfg(test)]
mod prepare_request_tests {
    use super::RqClient;
    use crate::syntax::http_method::HttpMethod;
    use crate::syntax::parse_result::Request;
    use crate::syntax::types::ValueType;

    fn request_with_body(body: &str, body_type: Option<ValueType>) -> Request {
        Request {
            name: "post".to_string(),
            url: "http://localhost:8080".to_string(),
            raw_url: "http://localhost:8080".to_string(),
            method: HttpMethod::POST,
            headers: Vec::new(),
            body: Some(body.to_string()),
            body_type,
            headers_var: None,
            slot_bindings: Vec::new(),
            endpoint: None,
            auth: None,
            auth_location: None,
            timeout: None,
            required_variables: Vec::new(),
            source_path: None,
            related_files: Vec::new(),
            line: 0,
            character: 0,
        }
    }

    fn content_type_of(request: Request) -> Option<String> {
        RqClient::apply_request_defaults(request)
            .expect("prepare_request failed")
            .headers
            .into_iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value)
    }

    #[test]
    fn a_json_typed_body_gets_the_json_content_type() {
        let target = content_type_of(request_with_body(
            "{\"name\": \"alice\"}",
            Some(ValueType::Json),
        ));
        assert_eq!(target, Some("application/json".to_string()));
    }

    #[test]
    fn a_string_body_that_looks_like_json_gets_no_content_type() {
        let target = content_type_of(request_with_body(
            "{\"name\": \"alice\"}",
            Some(ValueType::String),
        ));
        assert_eq!(target, None);
    }

    #[test]
    fn an_untyped_body_gets_no_content_type() {
        let target = content_type_of(request_with_body("{\"name\": \"alice\"}", None));
        assert_eq!(target, None);
    }

    #[test]
    fn a_declared_content_type_is_not_overridden() {
        let mut request = request_with_body("{\"a\": 1}", Some(ValueType::Json));
        request.headers.push((
            "Content-Type".to_string(),
            "application/merge-patch+json".to_string(),
        ));
        let target = content_type_of(request);
        assert_eq!(target, Some("application/merge-patch+json".to_string()));
    }
}

#[cfg(test)]
mod parse_cli_variables_tests {
    use super::RqClient;
    use crate::error::RqError;

    fn parse(kv: &str) -> Result<Vec<crate::syntax::Variable>, RqError> {
        RqClient::parse_cli_variables(&[kv.to_string()])
    }

    #[test]
    fn a_name_value_pair_becomes_a_variable() {
        let target = parse("token=abc=123").expect("parse failed");
        assert_eq!(target[0].name, "token");
    }

    #[test]
    fn a_variable_without_equals_is_rejected() {
        let target = parse("token");
        assert!(matches!(target, Err(RqError::Validation(_))));
    }

    #[test]
    fn a_variable_with_an_empty_name_is_rejected() {
        let target = parse(" =abc");
        assert!(matches!(target, Err(RqError::Validation(_))));
    }
}

#[cfg(all(test, feature = "native"))]
mod navigation_tests {
    use super::RqClient;

    fn endpoint_names(source: &std::path::Path) -> Vec<String> {
        RqClient::default()
            .list_endpoints(source)
            .expect("list_endpoints failed")
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }

    #[test]
    fn endpoint_with_a_body_is_not_a_template() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("api.rq"),
            "ep api(url: \"http://localhost\") {\n    rq get(\"/\");\n}\n",
        )
        .expect("api.rq");
        let target = RqClient::default()
            .list_endpoints(dir.path())
            .expect("list_endpoints failed");
        assert!(!target[0].is_template);
    }

    #[test]
    fn endpoints_of_a_file_include_its_imports() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("base.rq"),
            "ep base(url: \"http://localhost\");\n",
        )
        .expect("base.rq");
        std::fs::write(dir.path().join("main.rq"), "import \"base\";\n").expect("main.rq");
        let target = endpoint_names(&dir.path().join("main.rq"));
        assert_eq!(target, vec!["base".to_string()]);
    }

    #[test]
    fn endpoints_of_a_file_exclude_files_it_does_not_import() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("imported.rq"),
            "ep imported_ep(url: \"http://localhost\");\n",
        )
        .expect("imported.rq");
        std::fs::write(
            dir.path().join("unrelated.rq"),
            "ep unrelated_ep(url: \"http://other.localhost\");\n",
        )
        .expect("unrelated.rq");
        std::fs::write(dir.path().join("main.rq"), "import \"imported\";\n").expect("main.rq");
        let target = endpoint_names(&dir.path().join("main.rq"));
        assert_eq!(target, vec!["imported_ep".to_string()]);
    }

    #[test]
    fn variable_reference_points_at_the_variable_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("test.rq"), "rq get(\"{{base_url}}/v1\");\n")
            .expect("test.rq");
        let target = RqClient::default()
            .list_variable_references(dir.path(), "base_url", None, None)
            .expect("list_variable_references failed");
        assert_eq!((target[0].line, target[0].character), (0, 10));
    }

    #[test]
    fn variable_references_include_the_environment_declaration() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("test.rq"),
            "env local {\n    host: \"localhost\"\n}\n\nrq get(\"{{host}}/path\");\n",
        )
        .expect("test.rq");
        let target = RqClient::default()
            .list_variable_references(dir.path(), "host", None, None)
            .expect("list_variable_references failed");
        let lines: Vec<usize> = target.iter().map(|r| r.line).collect();
        assert!(lines.contains(&1) && lines.contains(&4));
    }

    const CRUD_ENDPOINT: &str = "ep foo(\"http://localhost\") {\n    rq list();\n\n    [required(resource_id)]\n    rq get(resource_id);\n\n    [required(resource_id)]\n    rq put(resource_id, body: \"{{resource_id}}\");\n}\n";

    fn variable_refs_at(
        content: &str,
        name: &str,
        position: Option<(usize, usize)>,
    ) -> Vec<(usize, usize)> {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("test.rq");
        std::fs::write(&file, content).expect("test.rq");
        RqClient::default()
            .list_variable_references(dir.path(), name, Some(&file), position)
            .expect("list_variable_references failed")
            .iter()
            .map(|r| (r.line, r.character))
            .collect()
    }

    #[test]
    fn required_variable_references_stay_in_their_request() {
        let target = variable_refs_at(CRUD_ENDPOINT, "resource_id", Some((4, 12)));
        assert_eq!(target, vec![(3, 14), (4, 11)]);
    }

    #[test]
    fn required_variable_references_resolve_from_the_attribute() {
        let target = variable_refs_at(CRUD_ENDPOINT, "resource_id", Some((6, 16)));
        assert_eq!(target, vec![(6, 14), (7, 11), (7, 33)]);
    }

    #[test]
    fn required_variable_shadows_the_outer_definition() {
        let content = format!(
            "let resource_id = \"1\";\n{CRUD_ENDPOINT}rq other(\"/{{{{resource_id}}}}\");\n"
        );
        let target = variable_refs_at(&content, "resource_id", Some((0, 5)));
        assert_eq!(target, vec![(0, 4), (10, 13)]);
    }

    #[test]
    fn variable_used_outside_any_required_request_is_global() {
        let content = format!("{CRUD_ENDPOINT}rq other(\"/{{{{resource_id}}}}\");\n");
        let target = variable_refs_at(&content, "resource_id", Some((9, 16)));
        assert_eq!(target, vec![(9, 13)]);
    }

    #[test]
    fn required_variable_in_a_truncated_request_stays_in_it() {
        let content = "[required(resource_id)]\nrq get(resource_id";
        let target = variable_refs_at(content, "resource_id", Some((1, 8)));
        assert_eq!(target, vec![(0, 10), (1, 7)]);
    }

    #[test]
    fn endpoint_references_include_the_definition_and_its_children() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("base.rq"),
            "ep base(url: \"http://localhost\");\n",
        )
        .expect("base.rq");
        std::fs::write(
            dir.path().join("child.rq"),
            "ep child<base>(url: \"http://localhost/child\") {\n    rq get(\"/\");\n}\n",
        )
        .expect("child.rq");
        let target = RqClient::default()
            .list_endpoint_references(dir.path(), "base", None)
            .expect("list_endpoint_references failed");
        let files: Vec<&str> = target.iter().map(|r| r.file.as_str()).collect();
        assert!(files.iter().any(|f| f.ends_with("base.rq")));
        assert!(files.iter().any(|f| f.ends_with("child.rq")));
    }
}

#[cfg(all(test, feature = "native"))]
mod variable_tests {
    use super::{RequestTarget, RqClient};
    use std::path::Path;

    fn write(dir: &Path, file: &str, content: &str) {
        std::fs::write(dir.join(file), content).expect("write fixture");
    }

    fn variable_names(source: &Path) -> Vec<String> {
        RqClient::default()
            .list_variables(source, None)
            .expect("list_variables failed")
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }

    #[test]
    fn environment_value_takes_precedence_over_let() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "test.rq",
            "let host = \"file-level\";\n\nenv dev {\n    host: \"env-level\"\n}\n",
        );
        let target = RqClient::default()
            .get_variable(dir.path(), "host", Some("dev"), true, None)
            .expect("get_variable failed");
        assert_eq!(
            (target.value.as_str(), target.source.as_str()),
            ("env-level", "env:dev")
        );
    }

    #[test]
    fn unresolved_reference_fails_when_interpolating() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "test.rq",
            "let api_url = \"{{base_url}}/v1\";\n",
        );
        let target = RqClient::default().get_variable(dir.path(), "api_url", None, true, None);
        assert!(target.is_err_and(|e| e.to_string().contains("Unresolved variable")));
    }

    #[test]
    fn unresolved_reference_is_kept_without_interpolation() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "test.rq",
            "let api_url = \"{{base_url}}/v1\";\n",
        );
        let target = RqClient::default()
            .get_variable(dir.path(), "api_url", None, false, None)
            .expect("get_variable failed");
        assert!(target.value.contains("{{base_url}}"));
    }

    #[test]
    fn variables_of_a_file_include_its_imports() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "shared.rq",
            "let shared_url = \"http://shared.localhost\";\n",
        );
        write(dir.path(), "main.rq", "import \"shared\";\n");
        let target = variable_names(&dir.path().join("main.rq"));
        assert_eq!(target, vec!["shared_url".to_string()]);
    }

    #[test]
    fn variables_of_a_file_exclude_files_it_does_not_import() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "imported.rq",
            "let imported_url = \"http://imported\";\n",
        );
        write(
            dir.path(),
            "unrelated.rq",
            "let unrelated_url = \"http://unrelated\";\n",
        );
        write(dir.path(), "main.rq", "import \"imported\";\n");
        let target = variable_names(&dir.path().join("main.rq"));
        assert_eq!(target, vec!["imported_url".to_string()]);
    }

    #[test]
    fn variables_before_a_syntax_error_are_still_listed() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "partial.rq",
            "let base_url = \"http://localhost\";\nlet b =\n",
        );
        let target = variable_names(dir.path());
        assert!(target.contains(&"base_url".to_string()));
    }

    fn write_imported_target(dir: &Path) {
        write(
            dir,
            "shared.rq",
            "let host = \"http://localhost\";\nlet target = \"imported\";\n",
        );
    }

    fn write_local_override(dir: &Path) {
        write_imported_target(dir);
        write(
            dir,
            "main.rq",
            "import \"shared\";\n\nlet target = \"local\";\nlet path = \"/{{target}}\";\n\nrq probe(\"{{host}}/{{target}}\");\n",
        );
    }

    #[test]
    fn request_details_use_the_local_override_of_an_imported_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_local_override(dir.path());
        let target = RqClient::default()
            .get_request_details(
                RequestTarget::new(&dir.path().join("main.rq"), "probe"),
                None,
                true,
                false,
                &[],
            )
            .expect("get_request_details failed");
        assert_eq!(target.url, "http://localhost/local");
    }

    #[test]
    fn request_details_use_an_import_declared_after_the_local_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_imported_target(dir.path());
        write(
            dir.path(),
            "main.rq",
            "let target = \"local\";\n\nimport \"shared\";\n\nrq probe(\"{{host}}/{{target}}\");\n",
        );
        let target = RqClient::default()
            .get_request_details(
                RequestTarget::new(&dir.path().join("main.rq"), "probe"),
                None,
                true,
                false,
                &[],
            )
            .expect("get_request_details failed");
        assert_eq!(target.url, "http://localhost/imported");
    }

    #[test]
    fn variable_lookup_returns_the_local_override_of_an_imported_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_local_override(dir.path());
        let main = dir.path().join("main.rq");
        let target = RqClient::default()
            .get_variable(dir.path(), "target", None, true, Some(&main))
            .expect("get_variable failed");
        assert_eq!(target.value, "local");
    }

    #[test]
    fn variable_lookup_points_at_the_local_override_of_an_imported_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_local_override(dir.path());
        let main = dir.path().join("main.rq");
        let target = RqClient::default()
            .get_variable(dir.path(), "target", None, false, Some(&main))
            .expect("get_variable failed");
        assert_eq!((target.file.ends_with("main.rq"), target.line), (true, 2));
    }

    #[test]
    fn variable_interpolation_uses_the_local_override_of_an_imported_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_local_override(dir.path());
        let main = dir.path().join("main.rq");
        let target = RqClient::default()
            .get_variable(dir.path(), "path", None, true, Some(&main))
            .expect("get_variable failed");
        assert_eq!(target.value, "/local");
    }

    #[test]
    fn auth_details_use_the_local_override_of_an_imported_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "shared.rq", "let token = \"imported\";\n");
        write(
            dir.path(),
            "main.rq",
            "import \"shared\";\n\nlet token = \"local\";\n\nauth probe_auth(auth_type.bearer) {\n    token: \"{{token}}\",\n}\n",
        );
        let (_, _, fields, _, _, _) = RqClient::default()
            .get_auth_details(dir.path(), "probe_auth", None, true)
            .expect("get_auth_details failed");
        assert_eq!(fields.get("token").map(String::as_str), Some("local"));
    }
}

#[cfg(all(test, feature = "native"))]
mod prepare_tests {
    use super::{native, RequestTarget, RqClient};
    use crate::error::RqError;
    use crate::http::{HttpClient, HttpResponse};
    use crate::syntax::Request;
    use std::collections::HashMap;
    use std::future::Future;
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Default)]
    struct CapturingHttp {
        sent: Mutex<Vec<(Request, Option<Duration>)>>,
    }

    impl HttpClient for CapturingHttp {
        fn execute<'a>(
            &'a self,
            request: &'a Request,
            timeout: Option<Duration>,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RqError>> + Send + 'a>> {
            self.sent
                .lock()
                .expect("lock")
                .push((request.clone(), timeout));
            Box::pin(async {
                Ok(HttpResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body: String::new(),
                })
            })
        }
    }

    fn write(dir: &Path, file: &str, content: &str) {
        std::fs::write(dir.join(file), content).expect("write fixture");
    }

    fn prepare(source: &Path, name: &str, variables: &[&str]) -> crate::client::PreparedRequest {
        let variables: Vec<String> = variables.iter().map(|v| v.to_string()).collect();
        RqClient::default()
            .prepare_request(RequestTarget::new(source, name), None, &variables)
            .expect("prepare_request failed")
    }

    fn authorization(request: &Request) -> Option<&str> {
        request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .map(|(_, v)| v.as_str())
    }

    #[tokio::test]
    async fn run_sends_exactly_the_prepared_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "shared.rq",
            "let host = \"http://localhost\";\nlet target = \"imported\";\n",
        );
        write(
            dir.path(),
            "main.rq",
            "import \"shared\";\n\nlet target = \"local\";\n\nauth token_auth(auth_type.bearer) {\n    token: \"{{target}}-token\",\n}\n\n[auth(\"token_auth\")]\n[timeout(5)]\nep api(\"{{host}}/{{target}}\", $[\"X-Scope\": \"{{target}}\"]) {\n    rq create(\"/items\", body: ${\"name\": \"{{target}}\"});\n}\n",
        );
        let main = dir.path().join("main.rq");
        let capture = Arc::new(CapturingHttp::default());
        let client = RqClient::new(
            Arc::new(native::NativeFs),
            Arc::new(native::NativeSecretProvider),
            capture.clone(),
        );
        client
            .run(&main, Some("api/create"), None, &[])
            .await
            .expect("run failed");
        let prepared = prepare(&main, "api/create", &[]);
        let target = capture.sent.lock().expect("lock").clone();
        assert_eq!(target, vec![(prepared.request, prepared.timeout)]);
    }

    #[test]
    fn prepared_request_applies_the_bearer_token_of_its_auth() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "main.rq",
            "let token = \"abc\";\n\nauth token_auth(auth_type.bearer) {\n    token: \"{{token}}\",\n}\n\n[auth(\"token_auth\")]\nrq probe(\"http://localhost/probe\");\n",
        );
        let target = prepare(&dir.path().join("main.rq"), "probe", &[]);
        assert_eq!(authorization(&target.request), Some("Bearer abc"));
    }

    #[test]
    fn prepared_request_applies_a_runtime_auth_token_to_an_oauth2_auth() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "main.rq",
            "auth login(auth_type.oauth2_authorization_code) {\n    client_id: \"app\",\n    authorization_url: \"http://localhost/authorize\",\n    token_url: \"http://localhost/token\",\n}\n\n[auth(\"login\")]\nrq probe(\"http://localhost/probe\");\n",
        );
        let target = prepare(&dir.path().join("main.rq"), "probe", &["auth_token=xyz"]);
        assert_eq!(
            (
                authorization(&target.request),
                target.auth.map(|auth| auth.token_required)
            ),
            (Some("Bearer xyz"), Some(false))
        );
    }

    #[test]
    fn prepared_request_without_a_known_token_asks_for_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "main.rq",
            "auth service(auth_type.oauth2_client_credentials) {\n    client_id: \"app\",\n    client_secret: \"secret\",\n    token_url: \"http://localhost/token\",\n}\n\n[auth(\"service\")]\nrq probe(\"http://localhost/probe\");\n",
        );
        let target = prepare(&dir.path().join("main.rq"), "probe", &[]);
        assert_eq!(
            (
                authorization(&target.request),
                target.auth.map(|auth| auth.token_required)
            ),
            (None, Some(true))
        );
    }

    #[test]
    fn prepared_auth_fields_resolve_with_runtime_variables() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "main.rq",
            "auth service(auth_type.oauth2_client_credentials) {\n    client_id: \"{{client}}\",\n    client_secret: \"secret\",\n    token_url: \"http://localhost/token\",\n}\n\n[auth(\"service\")]\nrq probe(\"http://localhost/probe\");\n",
        );
        let target = prepare(
            &dir.path().join("main.rq"),
            "probe",
            &["client=runtime-app"],
        );
        let client_id = target
            .auth
            .and_then(|auth| auth.config.fields.get("client_id").map(|t| t.value.clone()));
        assert_eq!(client_id.as_deref(), Some("runtime-app"));
    }

    #[test]
    fn prepared_request_uses_the_auth_declared_in_its_own_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("a")).expect("mkdir");
        std::fs::create_dir(dir.path().join("b")).expect("mkdir");
        let auth_file = |token: &str| {
            format!("auth shared_auth(auth_type.bearer) {{\n    token: \"{token}\",\n}}\n\n[auth(\"shared_auth\")]\nrq probe(\"http://localhost/probe\");\n")
        };
        write(&dir.path().join("a"), "main.rq", &auth_file("from-a"));
        write(&dir.path().join("b"), "main.rq", &auth_file("from-b"));
        let target = prepare(&dir.path().join("b").join("main.rq"), "probe", &[]);
        assert_eq!(authorization(&target.request), Some("Bearer from-b"));
    }

    #[test]
    fn prepared_request_reads_secrets_from_the_context_not_the_request_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
        write(dir.path(), ".env", "PROBE_SECRET=from-context\n");
        write(&dir.path().join("sub"), ".env", "PROBE_SECRET=from-sub\n");
        write(
            &dir.path().join("sub"),
            "main.rq",
            "rq probe(\"http://localhost/{{probe_secret}}\");\n",
        );
        let request_file = dir.path().join("sub").join("main.rq");
        let target = RqClient::default()
            .prepare_request(
                RequestTarget::new(dir.path(), "probe").in_file(Some(&request_file)),
                None,
                &[],
            )
            .expect("prepare_request failed");
        assert_eq!(target.request.url, "http://localhost/from-context");
    }

    #[test]
    fn scope_file_selects_the_request_among_files_with_the_same_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "a.rq", "rq probe(\"http://localhost/a\");\n");
        write(dir.path(), "b.rq", "rq probe(\"http://localhost/b\");\n");
        let request_file = dir.path().join("b.rq");
        let target = RqClient::default()
            .prepare_request(
                RequestTarget::new(dir.path(), "probe").in_file(Some(&request_file)),
                None,
                &[],
            )
            .expect("prepare_request failed");
        assert_eq!(target.request.url, "http://localhost/b");
    }

    #[test]
    fn prepared_request_rejects_a_missing_required_variable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "main.rq",
            "[required(item_id)]\nrq probe(\"http://localhost/{{item_id}}\");\n",
        );
        let target = RqClient::default().prepare_request(
            RequestTarget::new(&dir.path().join("main.rq"), "probe"),
            None,
            &[],
        );
        assert!(target.is_err_and(|e| e.to_string().contains("Required variable(s) not set")));
    }
}
