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

pub fn pretty_body(body: &str) -> String {
    if serde_json::from_str::<serde::de::IgnoredAny>(body).is_ok() {
        reindent_json(body.trim())
    } else {
        body.to_string()
    }
}

fn reindent_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len() * 2);
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = json.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '{' | '[' => {
                out.push(ch);
                skip_whitespace(&mut chars);
                if matches!(chars.peek(), Some('}') | Some(']')) {
                    continue;
                }
                depth += 1;
                push_newline(&mut out, depth);
            }
            '}' | ']' => {
                if !out.ends_with('{') && !out.ends_with('[') {
                    depth = depth.saturating_sub(1);
                    push_newline(&mut out, depth);
                }
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                push_newline(&mut out, depth);
            }
            ':' => out.push_str(": "),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    out
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        chars.next();
    }
}

fn push_newline(out: &mut String, depth: usize) {
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
}

#[cfg(test)]
mod tests {
    use super::pretty_body;

    #[test]
    fn json_body_keeps_key_order() {
        let target = pretty_body(r#"{"b":1,"a":{"c":[1,2]}}"#);
        assert_eq!(
            target,
            "{\n  \"b\": 1,\n  \"a\": {\n    \"c\": [\n      1,\n      2\n    ]\n  }\n}"
        );
    }

    #[test]
    fn json_body_keeps_empty_containers_inline() {
        let target = pretty_body(r#"{"a":{},"b":[ ]}"#);
        assert_eq!(target, "{\n  \"a\": {},\n  \"b\": []\n}");
    }

    #[test]
    fn json_body_leaves_strings_untouched() {
        let target = pretty_body(r#"{"a":"x, {y}: \"z\""}"#);
        assert_eq!(target, "{\n  \"a\": \"x, {y}: \\\"z\\\"\"\n}");
    }

    #[test]
    fn non_json_body_is_returned_as_is() {
        let target = pretty_body("plain text, {not json");
        assert_eq!(target, "plain text, {not json");
    }
}
