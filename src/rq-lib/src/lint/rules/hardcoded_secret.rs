use crate::lint::{line_col, significant_tokens, LintContext, LintDiagnostic, LintRule};
use crate::syntax::token::TokenType;

pub struct Rule;

const AUTH_HEADER: &str = "authorization";
const AUTH_HEADER_VARIABLE: &str = "api_token";

const SECRET_NAMES: &[&str] = &[
    "token",
    "secret",
    "password",
    "passwd",
    "credential",
    "credentials",
    "authorization",
    "apikey",
    "api_key",
    "access_token",
    "refresh_token",
    "client_secret",
    "private_key",
    "secret_key",
];

const SECRET_SUFFIXES: &[&str] = &[
    "_token",
    "_secret",
    "_password",
    "_passwd",
    "_api_key",
    "_apikey",
    "_secret_key",
    "_private_key",
    "_access_key",
    "_signing_key",
    "_credential",
    "_credentials",
];

const NON_SECRET_SUFFIXES: &[&str] = &[
    "_url",
    "_uri",
    "_endpoint",
    "_id",
    "_file",
    "_path",
    "_method",
    "_name",
    "_type",
];

struct HardcodedSecret {
    field: String,
    variable: String,
    offset: usize,
}

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "hardcoded_secret"
    }

    fn description(&self) -> &'static str {
        "A credential written as a literal in a `.rq` file is committed to version control. \
         Move it to a `.env` file next to the source and reference it as `{{name}}`."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for secret in hardcoded_secrets(ctx.source) {
            let (line, column) = line_col(ctx.source, secret.offset);
            let field = &secret.field;
            let variable = &secret.variable;
            let key = variable.to_ascii_uppercase();
            out.push(LintDiagnostic {
                severity: "error",
                rule: "hardcoded_secret",
                message: format!(
                    "`{field}` is set to a literal credential. `.rq` files are committed to \
                     version control, so the secret leaks with the repository. Put the value in a \
                     `.env` file next to this one as `{key}=<value>` — or \
                     `ENV__<ENVIRONMENT>__{key}=<value>` to scope it to one environment — and \
                     reference it here as `\"{{{{{variable}}}}}\"`. Secrets outrank environments \
                     and file-level `let` bindings in rq's precedence chain, so the reference \
                     resolves at run time with no other change."
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Add `{key}=<value>` to `.env` (git-ignored) and replace the literal with \
                     `\"{{{{{variable}}}}}\"`."
                )),
            });
        }
    }
}

fn hardcoded_secrets(source: &str) -> Vec<HardcodedSecret> {
    let significant = significant_tokens(source);
    let mut found = Vec::new();
    for window in significant.windows(3) {
        let [field, separator, value] = window else {
            continue;
        };
        if !is_binding_separator(separator) {
            continue;
        }
        if value.token_type != TokenType::String {
            continue;
        }
        let Some(name) = binding_name(field) else {
            continue;
        };
        if !is_secret_name(&name) {
            continue;
        }
        let Some(literal) = string_content(&value.value) else {
            continue;
        };
        if literal.contains("{{") || literal.trim().is_empty() {
            continue;
        }
        found.push(HardcodedSecret {
            variable: secret_variable(&name),
            field: name,
            offset: value.span.start,
        });
    }
    found
}

fn is_binding_separator(token: &crate::syntax::token::Token) -> bool {
    match token.token_type {
        TokenType::Punctuation => token.value == ":",
        TokenType::Operator => token.value == "=",
        _ => false,
    }
}

fn binding_name(token: &crate::syntax::token::Token) -> Option<String> {
    let raw = match token.token_type {
        TokenType::Identifier => token.value.as_str(),
        TokenType::String => string_content(&token.value)?,
        _ => return None,
    };
    Some(raw.trim().to_ascii_lowercase().replace('-', "_"))
}

fn is_secret_name(name: &str) -> bool {
    if name.contains("public") {
        return false;
    }
    if NON_SECRET_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return false;
    }
    SECRET_NAMES.contains(&name) || SECRET_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn secret_variable(name: &str) -> String {
    if name == AUTH_HEADER {
        return AUTH_HEADER_VARIABLE.to_string();
    }
    name.to_string()
}

fn string_content(raw: &str) -> Option<&str> {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return None;
    }
    Some(&raw[1..raw.len() - 1])
}

#[cfg(test)]
mod tests {
    use crate::lint::lint;

    fn diagnostics_for(src: &str) -> Vec<crate::lint::LintDiagnostic> {
        lint(src, Some("draft.rq"), None)
            .diagnostics
            .into_iter()
            .filter(|d| d.rule == "hardcoded_secret")
            .collect()
    }

    #[test]
    fn flags_a_literal_token_in_an_auth_provider() {
        let src = "auth api_auth(auth_type.bearer) {\n    token: \"ghp_abc123\",\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert_eq!(target[0].line, 2);
        assert!(target[0].message.contains("TOKEN=<value>"));
        assert!(target[0].message.contains("{{token}}"));
    }

    #[test]
    fn does_not_echo_the_secret_value() {
        let src = "auth api_auth(auth_type.bearer) {\n    token: \"ghp_abc123\",\n}\n";
        let target = diagnostics_for(src);
        assert!(
            !target[0].message.contains("ghp_abc123"),
            "the diagnostic must not repeat the credential: {}",
            target[0].message
        );
    }

    #[test]
    fn flags_a_literal_secret_in_a_let_binding() {
        let src = "let api_token = \"abc123\";\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("API_TOKEN=<value>"));
    }

    #[test]
    fn flags_a_literal_secret_in_an_environment_block() {
        let src = "env local {\n    base_url: \"http://localhost:8080\",\n    client_secret: \"shhh\",\n}\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0]
            .message
            .contains("ENV__<ENVIRONMENT>__CLIENT_SECRET"));
    }

    #[test]
    fn flags_a_literal_token_in_an_api_key_header() {
        let src = "rq list(\"http://x\", headers: $[\"X-Api-Key\": \"abc123\"]);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("X_API_KEY=<value>"));
    }

    #[test]
    fn names_the_authorization_header_secret_after_the_token_it_carries() {
        let src = "rq list(\"http://x\", headers: $[\"Authorization\": \"Bearer abc123\"]);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("API_TOKEN=<value>"));
    }

    #[test]
    fn does_not_flag_an_interpolated_value() {
        let src = "auth api_auth(auth_type.bearer) {\n    token: \"{{api_token}}\",\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_an_identifier_value() {
        let src = "auth api_auth(auth_type.bearer) {\n    token: api_token,\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_url_field_whose_name_ends_in_a_secret_word() {
        let src = "auth api_auth(auth_type.oauth2_client_credentials) {\n    client_id: \"my-client\",\n    token_url: \"https://auth.example.com/token\",\n}\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "`token_url` is an endpoint, not a credential"
        );
    }

    #[test]
    fn does_not_flag_a_certificate_path() {
        let src = "auth api_auth(auth_type.oauth2_client_credentials) {\n    client_id: \"my-client\",\n    token_url: \"https://auth.example.com/token\",\n    cert_file: \"certs/client.p12\",\n}\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_an_ordinary_variable() {
        let src = "let base_url = \"http://localhost:8080\";\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_an_empty_literal() {
        let src = "let auth_provider = \"\";\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
