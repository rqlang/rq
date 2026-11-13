use clap::ValueEnum;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

#[derive(Default)]
pub struct TextBlock {
    out: String,
}

impl TextBlock {
    pub fn field(mut self, key: &str, value: impl Display) -> Self {
        let value = value.to_string();
        if value.contains('\n') {
            self.out.push_str(&format!("{key}:\n"));
            for line in value.lines() {
                self.out.push_str(&format!("  {line}\n"));
            }
        } else {
            self.out.push_str(&format!("{key}: {value}\n"));
        }
        self
    }

    pub fn optional(self, key: &str, value: Option<impl Display>) -> Self {
        match value {
            Some(value) => self.field(key, value),
            None => self,
        }
    }

    pub fn map(mut self, key: &str, entries: &BTreeMap<String, String>) -> Self {
        if entries.is_empty() {
            return self;
        }
        self.out.push_str(&format!("{key}:\n"));
        for (name, value) in entries {
            self.out.push_str(&format!("  {name}: {value}\n"));
        }
        self
    }

    pub fn build(self) -> String {
        self.out
    }
}

pub fn render<T: Serialize>(
    format: OutputFormat,
    model: &T,
    to_text: impl FnOnce(&T) -> String,
) -> String {
    match format {
        OutputFormat::Json => to_json(model),
        OutputFormat::Text => to_text(model),
    }
}

pub fn to_json<T: Serialize + ?Sized>(model: &T) -> String {
    let json = serde_json::to_string_pretty(model).unwrap_or_else(|_| "null".to_string());
    format!("{json}\n")
}

pub fn render_list(items: &[String], title: &str, empty_msg: &str) -> String {
    if items.is_empty() {
        return format!("{empty_msg}\n");
    }
    let lines: String = items.iter().map(|item| format!("- {item}\n")).collect();
    format!("{title}\n{lines}")
}
