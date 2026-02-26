use super::rq_client_models::{RequestDetails, RequestExecutionResult, RequestInfo, RqConfig};
use crate::core::error::RqError;
use crate::core::logger::Logger;
use crate::syntax::{RqFile, Variable, VariableValue};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type AuthDetails = (String, String, HashMap<String, String>);

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

                let context = crate::syntax::variable_context::VariableContext {
                    file_variables: rq_file.file_variables.clone(),
                    environment_variables: env_vars.clone(),
                    secret_variables: secret_vars.clone(),
                    endpoint_variables: req_with_vars.endpoint_variables,
                    request_variables: req_with_vars.request_variables,
                    cli_variables: cli_vars.clone(),
                };

                let mut working = req_with_vars.request;

                let mut search_paths = Vec::new();
                if let Some(src) = &working.source_path {
                    search_paths.push(PathBuf::from(src));
                }
                search_paths.push(rq_file.path.clone());
                search_paths.extend(rq_file.imported_files.clone());
                for rf in &working.related_files {
                    search_paths.push(PathBuf::from(rf));
                }

                let env_path = source_path.join(".env");
                if env_path.exists() {
                    search_paths.push(env_path);
                }

                if let Some(header_var) = &working.headers_var {
                    match Self::expand_headers_var(header_var, &context) {
                        Ok(expanded) => {
                            let mut merged = expanded;
                            for (ck, cv) in working.headers.iter() {
                                if let Some(i) = merged
                                    .iter()
                                    .position(|(ek, _)| ek.eq_ignore_ascii_case(ck))
                                {
                                    merged[i] = (ck.clone(), cv.clone());
                                } else {
                                    merged.push((ck.clone(), cv.clone()));
                                }
                            }
                            working.headers = merged;
                        }
                        Err(e) => {
                            let (line, col, path) = crate::syntax::resolve::find_variable_location(
                                &search_paths,
                                header_var,
                            );
                            return Err(RqError::Syntax(
                                crate::syntax::error::SyntaxError::with_file(
                                    e,
                                    line,
                                    col,
                                    0..0,
                                    path.display().to_string(),
                                ),
                            ));
                        }
                    }
                }
                let mut resolved_request =
                    crate::syntax::resolve_variables(working, &context, &search_paths)?;

                if let Some(auth_name_raw) = &resolved_request.auth {
                    let auth_name =
                        crate::syntax::resolve_string(auth_name_raw, &context, &search_paths)?;

                    if let Some(auth_provider) = rq_file.auth_providers.get(&auth_name) {
                        let mut resolved_provider = auth_provider.clone();
                        for (_, token) in resolved_provider.fields.iter_mut() {
                            token.value = crate::syntax::resolve_string(
                                &token.value,
                                &context,
                                &search_paths,
                            )?;
                        }

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
                                return Err(RqError::Auth(crate::syntax::error::AuthError::new(
                                    format!("Configuration '{auth_name}' failed: {e}"),
                                )));
                            }
                        }
                    } else {
                        let msg = format!("Auth configuration '{auth_name}' not found");
                        return Err(RqError::Validation(msg));
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
                        return Err(RqError::Generic(error.to_string()));
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
                requests.push(RequestInfo {
                    name: req_with_vars.request.name.clone(),
                    endpoint: req_with_vars.request.endpoint.clone(),
                    file: crate::core::paths::clean_path(&rq_file.path),
                });
            }
        }

        requests.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(requests)
    }

    pub fn get_request_details(
        source_path: &Path,
        request_name: &str,
        _environment: Option<&str>,
    ) -> Result<RequestDetails, RqError> {
        let rq_files = Self::get_rq_files_to_process(source_path, Some(request_name))?;

        let rq_file = rq_files
            .into_iter()
            .next()
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let req_with_vars = rq_file
            .requests
            .iter()
            .find(|r| r.request.name == request_name)
            .ok_or_else(|| RqError::RequestNotFound(request_name.to_string()))?;

        let (auth_name, auth_type) = if let Some(auth_name) = &req_with_vars.request.auth {
            if let Some(auth_provider) = rq_file.auth_providers.get(auth_name) {
                (
                    Some(auth_name.clone()),
                    Some(auth_provider.auth_type.as_str().to_string()),
                )
            } else {
                (Some(auth_name.clone()), None)
            }
        } else {
            (None, None)
        };

        Ok(RequestDetails {
            name: req_with_vars.request.name.clone(),
            auth_name,
            auth_type,
            url: req_with_vars.request.url.clone(),
            headers: req_with_vars.request.headers.clone(),
            method: req_with_vars.request.method.as_str().to_string(),
            body: req_with_vars.request.body.clone(),
        })
    }

    pub fn list_auth(source_path: &Path) -> Result<Vec<super::rq_client_models::AuthListEntry>, RqError> {
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
            .map(|(name, auth_type)| super::rq_client_models::AuthListEntry { name, auth_type })
            .collect();
        auth_list.sort();

        Ok(auth_list)
    }

    pub fn get_auth_details(
        source_path: &Path,
        auth_name: &str,
        environment: Option<&str>,
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

        auth_provider.apply_defaults();

        let (all_variables, imported_files) =
            crate::syntax::load_all_variables(&file_path, source_path, environment)
                .map_err(|e| RqError::Generic(e.to_string()))?;

        let mut source_files = vec![file_path.clone()];
        source_files.extend(imported_files);
        let env_path = source_path.join(".env");
        if env_path.exists() {
            source_files.push(env_path);
        }

        let config =
            crate::syntax::resolve_auth_provider(auth_provider, &all_variables, &source_files)?;

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

    fn find_rq_file_with_request(
        dir: &Path,
        request_name: &str,
    ) -> Result<Option<RqFile>, RqError> {
        if !dir.is_dir() {
            return Ok(None);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(rq_file) = Self::find_rq_file_with_request(&path, request_name)? {
                    return Ok(Some(rq_file));
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
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
                        if let Some(syntax_err) =
                            e.downcast_ref::<crate::syntax::error::SyntaxError>()
                        {
                            return Err(RqError::Syntax(syntax_err.clone()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn collect_rq_files_parsed(dir: &Path, rq_files: &mut Vec<RqFile>) -> Result<(), RqError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_rq_files_parsed(&path, rq_files)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                match RqFile::from_path(&path) {
                    Ok(rq_file) => rq_files.push(rq_file),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse {}: {}",
                            crate::core::paths::clean_path(&path),
                            e
                        );
                    }
                }
            }
        }

        Ok(())
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

    fn collect_auth_entries(dir: &Path, auth_map: &mut HashMap<String, String>) -> Result<(), RqError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_auth_entries(&path, auth_map)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                if let Ok(rq_file) = RqFile::from_path(&path) {
                    for (auth_name, provider) in rq_file.auth_providers.iter() {
                        auth_map.insert(auth_name.clone(), provider.auth_type.as_str().to_string());
                    }
                }
            }
        }

        Ok(())
    }

    fn find_auth_provider(
        dir: &Path,
        name: &str,
    ) -> Result<Option<(crate::syntax::auth::Config, PathBuf)>, RqError> {
        if !dir.is_dir() {
            return Ok(None);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(config) = Self::find_auth_provider(&path, name)? {
                    return Ok(Some(config));
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                if let Ok(rq_file) = RqFile::from_path(&path) {
                    if let Some(config) = rq_file.auth_providers.get(name) {
                        return Ok(Some((config.clone(), path)));
                    }
                }
            }
        }

        Ok(None)
    }

    fn collect_environment_names(
        dir: &Path,
        env_names: &mut HashSet<String>,
    ) -> Result<(), RqError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_environment_names(&path, env_names)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rq") {
                if let Ok(rq_file) = RqFile::from_path(&path) {
                    for env_name in rq_file.environments.keys() {
                        env_names.insert(env_name.clone());
                    }
                }
            }
        }

        Ok(())
    }
}
