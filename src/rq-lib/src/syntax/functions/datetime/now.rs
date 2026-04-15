use super::super::traits::{FunctionContext, RqFunction};
use chrono::Local;

pub struct DateTimeNow;

impl RqFunction for DateTimeNow {
    fn namespace(&self) -> &str {
        "datetime"
    }

    fn name(&self) -> &str {
        "now"
    }

    fn validate_args(&self, args: &[String]) -> Result<(), String> {
        if args.len() > 1 {
            return Err("datetime.now takes at most one argument (format)".to_string());
        }
        Ok(())
    }

    fn execute(&self, args: &[String], _ctx: &FunctionContext) -> Result<String, String> {
        let now = Local::now();
        if args.is_empty() {
            Ok(now.format("%Y-%m-%dT%H:%M:%S.%3f%z").to_string())
        } else {
            // https://docs.rs/chrono/latest/chrono/format/strftime/index.html
            // Supports yyyy by replacing it with %Y to make it easier for users
            let format = args[0]
                .replace("yyyy", "%Y")
                .replace("MM", "%m")
                .replace("dd", "%d")
                .replace("HH", "%H")
                .replace("mm", "%M")
                .replace("ss", "%S");
            Ok(now.format(&format).to_string())
        }
    }
}
