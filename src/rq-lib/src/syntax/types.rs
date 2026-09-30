use super::functions;
use super::variable_context::VariableValue;
use std::collections::HashMap;

const MAX_REFERENCE_DEPTH: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    String,
    Json,
    Headers,
}

impl ValueType {
    pub fn label(&self) -> &'static str {
        match self {
            ValueType::String => "a string",
            ValueType::Json => "a JSON value",
            ValueType::Headers => "a headers map",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSlot {
    Url,
    Headers,
    HeaderValue,
    Body,
    QueryString,
}

impl ParameterSlot {
    pub fn label(&self) -> &'static str {
        match self {
            ParameterSlot::Url => "url",
            ParameterSlot::Headers => "headers",
            ParameterSlot::HeaderValue => "header value",
            ParameterSlot::Body => "body",
            ParameterSlot::QueryString => "qs",
        }
    }

    pub fn accepts(&self) -> &'static [ValueType] {
        match self {
            ParameterSlot::Url | ParameterSlot::QueryString | ParameterSlot::HeaderValue => {
                &[ValueType::String]
            }
            ParameterSlot::Headers => &[ValueType::Headers],
            ParameterSlot::Body => &[ValueType::String, ValueType::Json],
        }
    }

    pub fn expectation(&self) -> String {
        let labels: Vec<&str> = self.accepts().iter().map(|t| t.label()).collect();
        match labels.len() {
            1 => labels[0].to_string(),
            _ => format!(
                "{} or {}",
                labels[..labels.len() - 1].join(", "),
                labels[labels.len() - 1]
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue {
    Literal(ValueType),
    Variable(String),
    Function(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotBinding {
    pub slot: ParameterSlot,
    pub value: SlotValue,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeLookup {
    Known(ValueType),
    Undeclared,
    Invalid(String),
}

pub fn function_return_type(full_name: &str) -> Option<ValueType> {
    let (namespace, name) = full_name.split_once('.')?;
    functions::get_function(namespace, name).map(|f| f.return_type())
}

pub fn type_of_slot(value: &SlotValue, variables: &HashMap<&str, &VariableValue>) -> TypeLookup {
    match value {
        SlotValue::Literal(value_type) => TypeLookup::Known(*value_type),
        SlotValue::Function(full_name) => match function_return_type(full_name) {
            Some(value_type) => TypeLookup::Known(value_type),
            None => TypeLookup::Undeclared,
        },
        SlotValue::Variable(name) => type_of_variable(name, variables, 0),
    }
}

pub fn type_of_variable(
    name: &str,
    variables: &HashMap<&str, &VariableValue>,
    depth: usize,
) -> TypeLookup {
    if depth > MAX_REFERENCE_DEPTH {
        return TypeLookup::Invalid(format!(
            "Variable '{name}' has a circular reference in its variable chain"
        ));
    }
    let Some(value) = variables.get(name) else {
        return TypeLookup::Undeclared;
    };
    match value {
        VariableValue::String(_) => TypeLookup::Known(ValueType::String),
        VariableValue::Json(_) => TypeLookup::Known(ValueType::Json),
        VariableValue::Headers(_) => TypeLookup::Known(ValueType::Headers),
        VariableValue::SystemFunction {
            name: full_name, ..
        } => match function_return_type(full_name) {
            Some(value_type) => TypeLookup::Known(value_type),
            None => TypeLookup::Undeclared,
        },
        VariableValue::Reference(target) => type_of_variable(target, variables, depth + 1),
    }
}

pub fn slot_type_error(
    binding: &SlotBinding,
    variables: &HashMap<&str, &VariableValue>,
) -> Option<String> {
    match type_of_slot(&binding.value, variables) {
        TypeLookup::Undeclared => None,
        TypeLookup::Invalid(message) => Some(message),
        TypeLookup::Known(actual) => {
            if binding.slot.accepts().contains(&actual) {
                return None;
            }
            Some(describe_mismatch(binding, actual))
        }
    }
}

fn describe_mismatch(binding: &SlotBinding, actual: ValueType) -> String {
    let slot = binding.slot.label();
    let expected = binding.slot.expectation();
    match &binding.value {
        SlotValue::Variable(name) => {
            format!(
                "Variable '{name}' is {}; parameter '{slot}' expects {expected}",
                actual.label()
            )
        }
        SlotValue::Function(full_name) => {
            format!(
                "{full_name}() returns {}; parameter '{slot}' expects {expected}",
                actual.label()
            )
        }
        SlotValue::Literal(_) => {
            format!(
                "Parameter '{slot}' expects {expected}, but got {}",
                actual.label()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::variable_context::VariableValue;

    fn binding(slot: ParameterSlot, value: SlotValue) -> SlotBinding {
        SlotBinding {
            slot,
            value,
            line: 0,
            character: 0,
        }
    }

    #[test]
    fn headers_variable_is_rejected_by_the_url_slot() {
        let headers = VariableValue::Headers(vec![("A".into(), "b".into())]);
        let variables = HashMap::from([("h", &headers)]);
        let target = slot_type_error(
            &binding(ParameterSlot::Url, SlotValue::Variable("h".into())),
            &variables,
        );
        assert_eq!(
            target,
            Some("Variable 'h' is a headers map; parameter 'url' expects a string".to_string())
        );
    }

    #[test]
    fn json_variable_is_accepted_by_the_body_slot() {
        let json = VariableValue::Json("{\"a\":1}".into());
        let variables = HashMap::from([("p", &json)]);
        let target = slot_type_error(
            &binding(ParameterSlot::Body, SlotValue::Variable("p".into())),
            &variables,
        );
        assert_eq!(target, None);
    }

    #[test]
    fn json_variable_is_rejected_by_the_url_slot() {
        let json = VariableValue::Json("{\"a\":1}".into());
        let variables = HashMap::from([("p", &json)]);
        let target = slot_type_error(
            &binding(ParameterSlot::Url, SlotValue::Variable("p".into())),
            &variables,
        );
        assert_eq!(
            target,
            Some("Variable 'p' is a JSON value; parameter 'url' expects a string".to_string())
        );
    }

    #[test]
    fn string_variable_is_rejected_by_the_headers_slot() {
        let text = VariableValue::String("not-headers".into());
        let variables = HashMap::from([("h", &text)]);
        let target = slot_type_error(
            &binding(ParameterSlot::Headers, SlotValue::Variable("h".into())),
            &variables,
        );
        assert_eq!(
            target,
            Some("Variable 'h' is a string; parameter 'headers' expects a headers map".to_string())
        );
    }

    #[test]
    fn a_reference_chain_resolves_to_the_target_type() {
        let json = VariableValue::Json("{}".into());
        let reference = VariableValue::Reference("a".into());
        let variables = HashMap::from([("a", &json), ("b", &reference)]);
        let target = type_of_variable("b", &variables, 0);
        assert_eq!(target, TypeLookup::Known(ValueType::Json));
    }

    #[test]
    fn a_circular_reference_chain_is_reported_as_invalid() {
        let to_b = VariableValue::Reference("b".into());
        let to_a = VariableValue::Reference("a".into());
        let variables = HashMap::from([("a", &to_b), ("b", &to_a)]);
        let target = type_of_variable("a", &variables, 0);
        assert!(matches!(target, TypeLookup::Invalid(_)));
    }

    #[test]
    fn an_undeclared_variable_yields_no_type_error() {
        let variables = HashMap::new();
        let target = slot_type_error(
            &binding(ParameterSlot::Url, SlotValue::Variable("missing".into())),
            &variables,
        );
        assert_eq!(target, None);
    }

    #[test]
    fn a_json_returning_function_is_rejected_by_the_url_slot() {
        let variables = HashMap::new();
        let target = slot_type_error(
            &binding(
                ParameterSlot::Url,
                SlotValue::Function("io.read_json".into()),
            ),
            &variables,
        );
        assert_eq!(
            target,
            Some(
                "io.read_json() returns a JSON value; parameter 'url' expects a string".to_string()
            )
        );
    }

    #[test]
    fn a_string_returning_function_is_accepted_by_the_url_slot() {
        let variables = HashMap::new();
        let target = slot_type_error(
            &binding(
                ParameterSlot::Url,
                SlotValue::Function("io.read_file".into()),
            ),
            &variables,
        );
        assert_eq!(target, None);
    }

    #[test]
    fn the_body_slot_expectation_names_both_accepted_types() {
        assert_eq!(
            ParameterSlot::Body.expectation(),
            "a string or a JSON value"
        );
    }
}
