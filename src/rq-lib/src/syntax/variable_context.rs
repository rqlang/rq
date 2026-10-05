#[derive(Debug, Clone, PartialEq)]
pub enum VariableValue {
    String(String),
    Json(String),
    Reference(String),
    Headers(Vec<(String, String)>),
    SystemFunction { name: String, args: Vec<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub value: VariableValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableContext {
    pub file_variables: Vec<Variable>,
    pub environment_variables: Vec<Variable>,
    pub secret_variables: Vec<Variable>,
    pub endpoint_variables: Vec<Variable>,
    pub request_variables: Vec<Variable>,
    pub cli_variables: Vec<Variable>,
}

#[derive(Default)]
pub struct VariableContextBuilder {
    file_variables: Vec<Variable>,
    environment_variables: Vec<Variable>,
    secret_variables: Vec<Variable>,
    endpoint_variables: Vec<Variable>,
    request_variables: Vec<Variable>,
    cli_variables: Vec<Variable>,
}

impl VariableContextBuilder {
    pub fn file_variables(mut self, v: Vec<Variable>) -> Self {
        self.file_variables = v;
        self
    }

    pub fn environment_variables(mut self, v: Vec<Variable>) -> Self {
        self.environment_variables = v;
        self
    }

    pub fn secret_variables(mut self, v: Vec<Variable>) -> Self {
        self.secret_variables = v;
        self
    }

    pub fn endpoint_variables(mut self, v: Vec<Variable>) -> Self {
        self.endpoint_variables = v;
        self
    }

    pub fn request_variables(mut self, v: Vec<Variable>) -> Self {
        self.request_variables = v;
        self
    }

    pub fn cli_variables(mut self, v: Vec<Variable>) -> Self {
        self.cli_variables = v;
        self
    }

    pub fn build(self) -> VariableContext {
        VariableContext {
            file_variables: self.file_variables,
            environment_variables: self.environment_variables,
            secret_variables: self.secret_variables,
            endpoint_variables: self.endpoint_variables,
            request_variables: self.request_variables,
            cli_variables: self.cli_variables,
        }
    }
}

impl VariableValue {
    pub fn display(&self) -> String {
        match self {
            VariableValue::String(s) => format!("\"{s}\""),
            VariableValue::Reference(s) => s.clone(),
            VariableValue::Json(s) => format!("${{{s}}}"),
            VariableValue::Headers(pairs) => {
                let inner = pairs
                    .iter()
                    .map(|(k, v)| format!("\"{k}\": \"{v}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("$[{inner}]")
            }
            VariableValue::SystemFunction { name, args } => {
                if args.is_empty() {
                    format!("{name}()")
                } else {
                    let inner = args
                        .iter()
                        .map(|a| format!("\"{a}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{name}({inner})")
                }
            }
        }
    }
}

impl VariableContext {
    pub fn builder() -> VariableContextBuilder {
        VariableContextBuilder::default()
    }

    pub fn as_map(&self) -> std::collections::HashMap<&str, &VariableValue> {
        let mut map = std::collections::HashMap::new();
        for var in &self.file_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        for var in &self.environment_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        for var in &self.secret_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        for var in &self.endpoint_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        for var in &self.request_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        for var in &self.cli_variables {
            map.insert(var.name.as_str(), &var.value);
        }
        map
    }

    pub fn source_of(&self, name: &str) -> Option<&'static str> {
        let levels: [(&'static str, &Vec<Variable>); 6] = [
            ("cli", &self.cli_variables),
            ("request", &self.request_variables),
            ("endpoint", &self.endpoint_variables),
            ("secret", &self.secret_variables),
            ("env", &self.environment_variables),
            ("let", &self.file_variables),
        ];
        levels
            .into_iter()
            .find(|(_, variables)| variables.iter().any(|v| v.name == name))
            .map(|(level, _)| level)
    }

    pub fn all_variables(&self) -> Vec<Variable> {
        let mut all = Vec::new();
        all.extend(self.file_variables.clone());
        all.extend(self.environment_variables.clone());
        all.extend(self.secret_variables.clone());
        all.extend(self.endpoint_variables.clone());
        all.extend(self.request_variables.clone());
        all.extend(self.cli_variables.clone());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::{Variable, VariableContext, VariableValue};

    fn host(level: &str) -> Vec<Variable> {
        vec![Variable {
            name: "host".to_string(),
            value: VariableValue::String(level.to_string()),
        }]
    }

    fn context_from(lowest_level: usize) -> VariableContext {
        let defined = |level: usize, label: &str| {
            if level >= lowest_level {
                host(label)
            } else {
                Vec::new()
            }
        };
        VariableContext::builder()
            .file_variables(defined(5, "let"))
            .environment_variables(defined(4, "env"))
            .secret_variables(defined(3, "secret"))
            .endpoint_variables(defined(2, "endpoint"))
            .request_variables(defined(1, "request"))
            .cli_variables(defined(0, "cli"))
            .build()
    }

    #[test]
    fn source_of_names_the_level_whose_value_wins() {
        for (lowest_level, label) in ["cli", "request", "endpoint", "secret", "env", "let"]
            .into_iter()
            .enumerate()
        {
            let target = context_from(lowest_level);
            let winning = target.as_map().get("host").map(|value| (*value).clone());
            assert_eq!(target.source_of("host"), Some(label));
            assert_eq!(winning, Some(VariableValue::String(label.to_string())));
        }
    }

    #[test]
    fn source_of_an_undefined_variable_is_none() {
        let target = VariableContext::builder().build();
        assert_eq!(target.source_of("host"), None);
    }
}
