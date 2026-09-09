use super::LintRule;

mod absolute_import_path;
mod base_ep_extension;
mod duplicated_ep_base;
mod duplicated_ep_config;
mod duplicated_noun_in_ep;
mod duplicated_request_qs;
mod empty_url_string;
mod hardcoded_secret;
mod json_body_as_string;
mod let_default_instead_of_required;
mod manual_auth_header;
mod missing_body_on_write;
mod multiple_endpoints_per_file;
mod query_param_as_header;
mod redundant_content_type_on_json_body;
mod single_brace_interpolation;
mod top_level_rq_should_be_ep;

pub fn all() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(json_body_as_string::Rule),
        Box::new(redundant_content_type_on_json_body::Rule),
        Box::new(missing_body_on_write::Rule),
        Box::new(duplicated_noun_in_ep::Rule),
        Box::new(base_ep_extension::Rule),
        Box::new(absolute_import_path::Rule),
        Box::new(let_default_instead_of_required::Rule),
        Box::new(top_level_rq_should_be_ep::Rule),
        Box::new(single_brace_interpolation::Rule),
        Box::new(empty_url_string::Rule),
        Box::new(multiple_endpoints_per_file::Rule),
        Box::new(duplicated_ep_base::Rule),
        Box::new(manual_auth_header::Rule),
        Box::new(hardcoded_secret::Rule),
        Box::new(duplicated_request_qs::Rule),
        Box::new(query_param_as_header::Rule),
        Box::new(duplicated_ep_config::Rule),
    ]
}
