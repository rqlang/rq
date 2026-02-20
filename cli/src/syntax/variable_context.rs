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

impl VariableContext {
    /// Returns all variables from all contexts as a single flat vector.
    /// Order: file -> environment -> secrets -> endpoint -> request -> CLI
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
