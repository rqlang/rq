pub use rq_lib::logger::Logger;

pub fn init_logging(debug: bool) {
    Logger::init(debug);
    if !Logger::is_debug_enabled() {
        return;
    }
    let args: Vec<String> = std::env::args().collect();
    Logger::debug(&format!(
        "* rq {} ({} {})",
        crate::core::version::app_version(),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    Logger::debug(&format!("* Command: {}", masked_command_line(&args)));
    if let Ok(dir) = std::env::current_dir() {
        Logger::debug(&format!("* Working directory: {}", dir.display()));
    }
}

pub fn log_finished(exit_code: i32, error: Option<&str>) {
    let detail = error.map(|e| format!(": {e}")).unwrap_or_default();
    Logger::debug(&format!("* Finished with exit code {exit_code}{detail}"));
}

fn masked_command_line(args: &[String]) -> String {
    let mut masked = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for arg in args {
        let shown = if mask_next {
            mask_variable(arg)
        } else if let Some(variable) = arg.strip_prefix("--variable=") {
            format!("--variable={}", mask_variable(variable))
        } else if let Some(variable) = short_variable_value(arg).filter(|v| !v.is_empty()) {
            let flags = arg.strip_suffix(variable).unwrap_or(arg);
            format!("{flags}{}", mask_variable(variable))
        } else {
            arg.clone()
        };
        mask_next = arg == "--variable" || short_variable_value(arg).is_some_and(str::is_empty);
        masked.push(quote_if_needed(shown));
    }
    masked.join(" ")
}

fn short_variable_value(arg: &str) -> Option<&str> {
    let cluster = arg.strip_prefix('-').filter(|c| !c.starts_with('-'))?;
    for (index, flag) in cluster.char_indices() {
        match flag {
            'v' => return cluster.get(index + 1..),
            'd' | 'h' | 'V' => continue,
            _ => return None,
        }
    }
    None
}

fn mask_variable(variable: &str) -> String {
    match variable.split_once('=') {
        Some((name, _)) => format!("{name}=***"),
        None => variable.to_string(),
    }
}

fn quote_if_needed(arg: String) -> String {
    if arg.contains(char::is_whitespace) {
        format!("\"{arg}\"")
    } else {
        arg
    }
}

#[cfg(test)]
mod tests {
    use super::masked_command_line;

    fn line(args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        masked_command_line(&args)
    }

    #[test]
    fn separate_variable_value_is_masked() {
        let target = line(&["rq", "-v", "token=abc", "-s", "api.rq"]);
        assert_eq!(target, "rq -v token=*** -s api.rq");
    }

    #[test]
    fn long_variable_value_is_masked() {
        let target = line(&["rq", "--variable", "token=abc", "--variable=user=ana"]);
        assert_eq!(target, "rq --variable token=*** --variable=user=***");
    }

    #[test]
    fn attached_short_variable_value_is_masked() {
        let target = line(&["rq", "-vtoken=abc"]);
        assert_eq!(target, "rq -vtoken=***");
    }

    #[test]
    fn variable_after_bundled_debug_flag_is_masked() {
        let target = line(&["rq", "-dv", "pat=abc"]);
        assert_eq!(target, "rq -dv pat=***");
    }

    #[test]
    fn variable_attached_to_bundled_flags_is_masked() {
        let target = line(&["rq", "-dvpat=abc"]);
        assert_eq!(target, "rq -dvpat=***");
    }

    #[test]
    fn value_of_another_short_flag_is_not_treated_as_a_variable() {
        let target = line(&["rq", "-sv", "-e", "local"]);
        assert_eq!(target, "rq -sv -e local");
    }

    #[test]
    fn argument_with_spaces_is_quoted() {
        let target = line(&["rq", "-s", "my api.rq"]);
        assert_eq!(target, "rq -s \"my api.rq\"");
    }
}
