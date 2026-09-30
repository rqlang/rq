use crate::lint::{
    request_declared_in, request_own_headers, LintContext, LintDiagnostic, LintRule,
};
use crate::syntax::parse_result::{Request, RequestWithVariables};
use crate::syntax::types::{type_of_slot, ParameterSlot, SlotValue, TypeLookup, ValueType};
use crate::syntax::variable_context::{Variable, VariableValue};
use std::collections::HashMap;

pub struct Rule;

const CONTENT_TYPE: &str = "content-type";
const APPLICATION_JSON: &str = "application/json";

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "redundant_content_type_on_json_body"
    }

    fn description(&self) -> &'static str {
        "rq sends `Content-Type: application/json` by itself when the body is JSON. Declaring \
         the header by hand repeats a decision the tool already makes."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for req_with_vars in &ctx.rq_file.requests {
            let request = &req_with_vars.request;
            if !request_declared_in(request, &ctx.rq_file.path) {
                continue;
            }
            let Some(body) = json_body_description(req_with_vars, ctx) else {
                continue;
            };
            if !declares_json_content_type(request, ctx) {
                continue;
            }
            let bare = bare_request_name(&request.name);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "redundant_content_type_on_json_body",
                message: format!(
                    "Request `{bare}` sets `Content-Type: application/json` by hand alongside \
                     {body}. rq adds that header itself whenever the body it sends is JSON, so \
                     the entry is redundant — drop it and let the tool derive the type from the \
                     body. Declare the header only to send something other than \
                     `application/json`, which is the one case rq leaves alone."
                ),
                line: request.line + 1,
                column: request.character + 1,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Remove the `\"Content-Type\": \"application/json\"` entry from `rq {bare}(...)`."
                )),
            });
        }
    }
}

fn json_body_description(
    req_with_vars: &RequestWithVariables,
    ctx: &LintContext,
) -> Option<String> {
    let binding = req_with_vars
        .request
        .slot_bindings
        .iter()
        .find(|binding| binding.slot == ParameterSlot::Body)?;
    let scoped = scoped_variables(req_with_vars, ctx);
    if type_of_slot(&binding.value, &scoped) != TypeLookup::Known(ValueType::Json) {
        return None;
    }
    match &binding.value {
        SlotValue::Literal(_) => Some("a `${...}` JSON body".to_string()),
        SlotValue::Function(name) => Some(format!("a JSON fixture loaded with `{name}`")),
        SlotValue::Variable(name) => {
            if defined_in_an_environment(name, ctx) {
                return None;
            }
            match scoped.get(name.as_str()) {
                Some(VariableValue::SystemFunction { .. }) => {
                    Some(format!("the JSON fixture held by `{name}`"))
                }
                _ => Some(format!("the JSON body held by `{name}`")),
            }
        }
    }
}

fn scoped_variables<'a>(
    req_with_vars: &'a RequestWithVariables,
    ctx: &'a LintContext,
) -> HashMap<&'a str, &'a VariableValue> {
    let scopes: [&[Variable]; 3] = [
        &ctx.rq_file.file_variables,
        &req_with_vars.endpoint_variables,
        &req_with_vars.request_variables,
    ];
    let mut map = HashMap::new();
    for scope in scopes {
        for variable in scope {
            map.insert(variable.name.as_str(), &variable.value);
        }
    }
    map
}

fn defined_in_an_environment(name: &str, ctx: &LintContext) -> bool {
    ctx.rq_file
        .environments
        .values()
        .any(|variables| variables.iter().any(|v| v.name == name))
}

fn declares_json_content_type(request: &Request, ctx: &LintContext) -> bool {
    own_headers(request, ctx).iter().any(|(key, value)| {
        key.trim().eq_ignore_ascii_case(CONTENT_TYPE)
            && value.trim().eq_ignore_ascii_case(APPLICATION_JSON)
    })
}

fn own_headers(request: &Request, ctx: &LintContext) -> Vec<(String, String)> {
    match request
        .endpoint
        .as_ref()
        .and_then(|name| ctx.rq_file.endpoints.get(name))
    {
        Some(endpoint) => request_own_headers(request, endpoint),
        None => request.headers.clone(),
    }
}

fn bare_request_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, Some("draft.rq"), None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "redundant_content_type_on_json_body")
            .collect()
    }

    #[test]
    fn flags_a_content_type_next_to_a_json_literal_body() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], ${\"name\": \"alice\"});\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 1);
        assert!(target[0].message.contains("`${...}` JSON body"));
    }

    #[test]
    fn flags_a_content_type_next_to_a_json_fixture() {
        let src = "rq post(\"http://x/users\", $[\"content-type\": \"application/json\"], io.read_json(\"users-post.json\"));\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("io.read_json"));
    }

    #[test]
    fn flags_a_json_body_held_by_a_variable() {
        let src = "let payload = ${\"name\": \"alice\"};\nrq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], payload);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0]
            .message
            .contains("the JSON body held by `payload`"));
    }

    #[test]
    fn flags_a_json_fixture_held_by_a_variable() {
        let src = "let payload = io.read_json(\"users-post.json\");\nrq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], payload);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0]
            .message
            .contains("the JSON fixture held by `payload`"));
    }

    #[test]
    fn does_not_flag_a_variable_holding_a_plain_string() {
        let src = "let payload = \"plain text\";\nrq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], payload);\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_variable_holding_a_non_json_fixture() {
        let src = "let payload = io.read_file(\"users-post.xml\");\nrq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], payload);\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_variable_an_environment_can_override() {
        let src = "env local {\n    payload: \"from-env\",\n}\n\nlet payload = ${\"k\": 1};\nrq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], payload);\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "an environment can replace the body at run time, so the header may be needed: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_an_unknown_variable() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], body: \"{{payload}}\");\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_json_body_without_the_header() {
        let src = "rq post(\"http://x/users\", body: ${\"name\": \"alice\"});\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_content_type_that_overrides_the_derived_one() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/merge-patch+json\"], ${\"name\": \"alice\"});\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "rq leaves a non-JSON content type alone, so the header is required: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_a_content_type_carrying_parameters() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/json; charset=utf-8\"], ${\"k\": 1});\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "the charset parameter changes the header rq would send: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_a_fixture_that_is_not_json() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], io.read_file(\"users-post.xml\"));\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "rq only derives the header when the body it sends is JSON: {target:?}"
        );
    }

    #[test]
    fn does_not_flag_a_content_type_inherited_from_the_endpoint() {
        let src = "ep users(\"http://x/users\", headers: $[\"Content-Type\": \"application/json\"]) {\n    rq post(body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert!(
            target.is_empty(),
            "the endpoint header may serve children without a JSON body: {target:?}"
        );
    }

    #[test]
    fn flags_a_request_declaring_the_header_inside_an_endpoint() {
        let src = "ep users(\"http://x/users\") {\n    rq post(headers: $[\"Content-Type\": \"application/json\"], body: ${});\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 2);
    }

    #[test]
    fn flags_a_json_fixture_on_a_request_inside_an_endpoint() {
        let src = "ep users(\"http://x/users\") {\n    rq post(headers: $[\"Content-Type\": \"application/json\"], body: io.read_json(\"users-post.json\"));\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 2);
    }

    #[test]
    fn does_not_flag_a_string_body() {
        let src = "rq post(\"http://x/users\", $[\"Content-Type\": \"application/json\"], \"plain text\");\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
