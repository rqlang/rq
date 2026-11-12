use super::{
    error::SyntaxError,
    fs::Fs,
    parse_result::{EndpointDefinition, ParseResult, RequestScope, RequestWithVariables},
    token::Token,
    ui_color::UiColor,
    variable_context::Variable,
};
use crate::syntax::auth::Config as AuthConfig;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct RqFile {
    pub path: PathBuf,
    pub requests: Vec<RequestWithVariables>,
    pub request_scopes: Vec<RequestScope>,
    pub environments: HashMap<String, Vec<Variable>>,
    pub environment_locations: HashMap<String, (String, usize, usize)>,
    pub environment_colors: HashMap<String, UiColor>,
    pub confirm_environments: HashSet<String>,
    pub auth_providers: HashMap<String, AuthConfig>,
    pub endpoints: HashMap<String, EndpointDefinition>,
    pub file_variables: Vec<Variable>,
    pub imported_files: Vec<PathBuf>,
    pub let_variable_locations: HashMap<String, (String, usize, usize)>,
    pub env_variable_locations: HashMap<String, HashMap<String, (String, usize, usize)>>,
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
            Ok(tokens) => tokens,
            Err(error) => match Self::tokenize_before_error(content, &error) {
                Some(tokens) => tokens,
                None => return Self::empty(path),
            },
        };
        let parse_result =
            crate::syntax::analysis::analyze_lenient(&tokens, path.clone(), content, fs);
        Self::from_parse_result(path, parse_result)
    }

    fn tokenize_before_error(content: &str, error: &SyntaxError) -> Option<Vec<Token>> {
        let line_start = content.get(..error.span.start)?.rfind('\n')?;
        crate::syntax::tokenize(&content[..line_start]).ok()
    }

    fn empty(path: PathBuf) -> Self {
        Self::from_parse_result(
            path,
            ParseResult {
                requests: Vec::new(),
                request_scopes: Vec::new(),
                environments: HashMap::new(),
                environment_locations: HashMap::new(),
                environment_colors: HashMap::new(),
                confirm_environments: HashSet::new(),
                auth_providers: HashMap::new(),
                endpoints: HashMap::new(),
                file_variables: Vec::new(),
                imported_files: Vec::new(),
                let_variable_locations: HashMap::new(),
                env_variable_locations: HashMap::new(),
            },
        )
    }

    fn from_parse_result(path: PathBuf, parse_result: ParseResult) -> Self {
        Self {
            path,
            requests: parse_result.requests,
            request_scopes: parse_result.request_scopes,
            environments: parse_result.environments,
            environment_locations: parse_result.environment_locations,
            environment_colors: parse_result.environment_colors,
            confirm_environments: parse_result.confirm_environments,
            auth_providers: parse_result.auth_providers,
            endpoints: parse_result.endpoints,
            file_variables: parse_result.file_variables,
            imported_files: parse_result.imported_files,
            let_variable_locations: parse_result.let_variable_locations,
            env_variable_locations: parse_result.env_variable_locations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RqFile;
    use crate::native::NativeFs;
    use crate::syntax::parse_result::{Request, RequestScope, RequiredVariable};
    use std::path::PathBuf;

    fn parse(source: &str) -> RqFile {
        RqFile::from_content_lenient(PathBuf::from("test.rq"), source, &NativeFs)
    }

    fn requests_of(source: &str) -> Vec<Request> {
        parse(source)
            .requests
            .into_iter()
            .map(|req_with_vars| req_with_vars.request)
            .collect()
    }

    fn scopes_of(source: &str) -> Vec<RequestScope> {
        parse(source).request_scopes
    }

    fn required_names(scope: &RequestScope) -> Vec<&str> {
        scope
            .required_variables
            .iter()
            .map(|required| required.name.as_str())
            .collect()
    }

    #[test]
    fn request_scope_covers_its_attributes_and_semicolon() {
        let source = "let a = \"1\";\n[required(id)]\nrq get(id);\nlet b = \"2\";\n";
        let target = &scopes_of(source)[0];
        assert_eq!(&source[target.span.clone()], "[required(id)]\nrq get(id);");
    }

    #[test]
    fn endpoint_request_scope_records_the_endpoint_header() {
        let source = "[timeout(5)]\nep items(\"http://x\") {\n    rq get(\"/\");\n}\n";
        let target = scopes_of(source)[0].endpoint_header.clone();
        assert_eq!(
            target.map(|header| &source[header]),
            Some("[timeout(5)]\nep items(\"http://x\") {")
        );
    }

    #[test]
    fn top_level_request_scope_has_no_endpoint_header() {
        let target = scopes_of("rq get(\"http://x\");\n");
        assert_eq!(target[0].endpoint_header, None);
    }

    #[test]
    fn each_request_keeps_its_own_required_variable_location() {
        let source = "[required(id)]\nrq a(id);\n\n[required(id)]\nrq b(id);\n";
        let target: Vec<Vec<RequiredVariable>> = requests_of(source)
            .into_iter()
            .map(|request| request.required_variables)
            .collect();
        assert_eq!(
            target,
            vec![
                vec![RequiredVariable {
                    name: "id".to_string(),
                    line: 0,
                    character: 10,
                }],
                vec![RequiredVariable {
                    name: "id".to_string(),
                    line: 3,
                    character: 10,
                }],
            ]
        );
    }

    #[test]
    fn lenient_parse_resumes_at_the_request_after_an_unclosed_one() {
        let source = "rq get(\"http://x\"\n\n[required(id)]\nrq other(\"{{id}}\");\n";
        let target: Vec<(String, Vec<String>)> = requests_of(source)
            .into_iter()
            .map(|request| {
                (
                    request.name.clone(),
                    request.required_names().cloned().collect(),
                )
            })
            .collect();
        assert_eq!(target, vec![("other".to_string(), vec!["id".to_string()])]);
    }

    #[test]
    fn unparsed_request_keeps_its_scope() {
        let source = "[required(id)]\nrq get(id\n\nrq other(\"http://x\");\n";
        let target = &scopes_of(source)[0];
        assert_eq!(
            (&source[target.span.clone()], required_names(target)),
            ("[required(id)]\nrq get(id", vec!["id"])
        );
    }

    #[test]
    fn broken_request_does_not_drop_its_endpoint_siblings() {
        let source = "ep items(\"http://x\") {\n    [required(id)]\n    rq get(id);\n    rq get(\"/\");\n}\n";
        let target: Vec<String> = requests_of(source)
            .into_iter()
            .map(|request| request.name)
            .collect();
        assert_eq!(target, vec!["items/get".to_string()]);
    }

    #[test]
    fn broken_endpoint_request_keeps_its_scope_and_header() {
        let source = "ep items(\"http://x/{{id}}\") {\n    [required(id)]\n    rq get(id\n    rq list();\n}\n";
        let scopes = scopes_of(source);
        let target: Vec<(&str, Option<&str>, Vec<&str>)> = scopes
            .iter()
            .map(|scope| {
                (
                    &source[scope.span.clone()],
                    scope.endpoint_header.clone().map(|header| &source[header]),
                    required_names(scope),
                )
            })
            .collect();
        assert_eq!(
            target,
            vec![
                (
                    "[required(id)]\n    rq get(id",
                    Some("ep items(\"http://x/{{id}}\") {"),
                    vec!["id"]
                ),
                (
                    "rq list();",
                    Some("ep items(\"http://x/{{id}}\") {"),
                    vec![]
                ),
            ]
        );
    }
}
