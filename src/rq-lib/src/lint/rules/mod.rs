use super::LintRule;

mod absolute_import_path;
mod base_ep_extension;
mod duplicated_noun_in_ep;
mod json_body_as_string;
mod let_default_instead_of_required;
mod missing_body_on_write;
mod top_level_rq_should_be_ep;

pub fn all() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(json_body_as_string::Rule),
        Box::new(missing_body_on_write::Rule),
        Box::new(duplicated_noun_in_ep::Rule),
        Box::new(base_ep_extension::Rule),
        Box::new(absolute_import_path::Rule),
        Box::new(let_default_instead_of_required::Rule),
        Box::new(top_level_rq_should_be_ep::Rule),
    ]
}
