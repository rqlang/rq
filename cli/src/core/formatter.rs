use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;

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

fn render_text_from_model<T: Serialize + ?Sized>(model: &T) -> String {
    let value = serde_json::to_value(model).unwrap_or(Value::Null);
    render_value(&value, 0)
}

fn render_value(value: &Value, indent: usize) -> String {
    let pad = " ".repeat(indent);
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => format!("{pad}{b}\n"),
        Value::Number(n) => format!("{pad}{n}\n"),
        Value::String(s) => {
            // If string is multiline, indent subsequent lines
            let lines: Vec<&str> = s.lines().collect();
            if lines.len() > 1 {
                let mut out = String::new();
                for line in lines {
                    // Indent all lines
                    out.push_str(&format!("{pad}{line}\n"));
                }
                out
            } else {
                format!("{pad}{s}\n")
            }
        }
        Value::Array(arr) => {
            let mut s = String::new();
            for item in arr {
                match item {
                    Value::Array(_) | Value::Object(_) => {
                        s.push_str(&format!("{pad}-\n"));
                        s.push_str(&render_value(item, indent + 2));
                    }
                    _ => {
                        // Primitives: render on same line
                        let rendered = render_value(item, 0);
                        // trim output to handle the newline added by render_value
                        s.push_str(&format!("{}- {}\n", pad, rendered.trim_end()));
                    }
                }
            }
            s
        }
        Value::Object(map) => {
            let mut s = String::new();
            // Using logic to detect complex vs simple values for inline printing
            for (k, v) in map {
                match v {
                    Value::Array(_) | Value::Object(_) => {
                        s.push_str(&format!("{pad}{k}:\n"));
                        s.push_str(&render_value(v, indent + 2));
                    }
                    _ => {
                        // Primitives on same line
                        let v_str = match v {
                            Value::String(str_val) => str_val.clone(),
                            value => value.to_string(),
                        };
                        s.push_str(&format!("{pad}{k}: {v_str}\n"));
                    }
                }
            }
            s
        }
    }
}

pub struct Formatter {
    engine: OutputFormat,
}

impl Formatter {
    pub fn new(engine: OutputFormat) -> Self {
        Self { engine }
    }

    pub fn format<T: Serialize>(&self, model: &T) -> String {
        match self.engine {
            OutputFormat::Text => render_text_from_model(model),
            OutputFormat::Json => serde_json::to_string_pretty(model).unwrap_or_default(),
        }
    }

    pub fn format_list<T: Serialize>(&self, list: &[T], title: &str, empty_msg: &str) -> String {
        match self.engine {
            OutputFormat::Text => {
                if list.is_empty() {
                    empty_msg.to_string()
                } else {
                    let content = render_text_from_model(list);
                    if title.is_empty() {
                        content
                    } else {
                        format!("{title}\n{content}")
                    }
                }
            }
            OutputFormat::Json => {
                serde_json::to_string_pretty(list).unwrap_or_else(|_| "[]".to_string())
            }
        }
    }
}

pub fn get_formatter(output_format: &OutputFormat) -> Formatter {
    Formatter::new(*output_format)
}
