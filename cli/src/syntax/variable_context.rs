#[derive(Debug, Clone, PartialEq)]
pub enum VariableValue {
    String(String),
    Array(Vec<String>),
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
            VariableValue::Array(items) => {
                let inner = items
                    .iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{inner}]")
            }
            VariableValue::Headers(pairs) => {
                let inner = pairs
                    .iter()
                    .map(|(k, v)| format!("\"{k}\": \"{v}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{inner}]")
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
