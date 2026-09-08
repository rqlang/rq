use crate::lint::{line_col, significant_tokens, LintContext, LintDiagnostic, LintRule};
use crate::syntax::token::TokenType;

pub struct Rule;

const AUTH_HEADER: &str = "authorization";
const BEARER_SCHEME: &str = "bearer ";
const PROVIDER_NAME: &str = "api_auth";

impl LintRule for Rule {
    fn id(&self) -> &'static str {
        "manual_auth_header"
    }

    fn description(&self) -> &'static str {
        "A hand-written `\"Authorization\": \"Bearer …\"` header re-implements the `auth` artifact. \
         Declare an `auth …(auth_type.bearer)` provider and attach it with `[auth(\"…\")]` instead."
    }

    fn check(&self, ctx: &LintContext, out: &mut Vec<LintDiagnostic>) {
        for (token, offset) in bearer_auth_headers(ctx.source) {
            let (line, column) = line_col(ctx.source, offset);
            out.push(LintDiagnostic {
                severity: "error",
                rule: "manual_auth_header",
                message: format!(
                    "The `Authorization` header is assembled by hand as `Bearer {token}`. rqlang \
                     has a dedicated artifact for this: declare \
                     `auth {PROVIDER_NAME}(auth_type.bearer) {{ token: \"{token}\", }}` once, \
                     attach it with `[auth(\"{PROVIDER_NAME}\")]` on the `ep` or `rq`, and drop \
                     the header. The provider sends `Authorization: Bearer <token>` for you, so \
                     the credential is declared in one place, `rq auth show` can inspect it, and \
                     swapping in an OAuth2 flow later changes only the provider."
                ),
                line,
                column,
                file: Some(ctx.display_path.to_string()),
                suggested_fix: Some(format!(
                    "Declare `auth {PROVIDER_NAME}(auth_type.bearer) {{ token: \"{token}\", }}`, \
                     put `[auth(\"{PROVIDER_NAME}\")]` above the `ep`/`rq`, and remove the \
                     `\"Authorization\"` entry from `headers`."
                )),
            });
        }
    }
}

fn bearer_auth_headers(source: &str) -> Vec<(String, usize)> {
    let significant = significant_tokens(source);
    let mut found = Vec::new();
    for window in significant.windows(3) {
        let [key, colon, value] = window else {
            continue;
        };
        if colon.token_type != TokenType::Punctuation || colon.value != ":" {
            continue;
        }
        if key.token_type != TokenType::String || value.token_type != TokenType::String {
            continue;
        }
        let (Some(name), Some(header_value)) =
            (string_content(&key.value), string_content(&value.value))
        else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case(AUTH_HEADER) {
            continue;
        }
        let Some(token) = strip_bearer_scheme(header_value.trim()) else {
            continue;
        };
        found.push((token, key.span.start));
    }
    found
}

fn strip_bearer_scheme(value: &str) -> Option<String> {
    let prefix = value.get(..BEARER_SCHEME.len())?;
    if !prefix.eq_ignore_ascii_case(BEARER_SCHEME) {
        return None;
    }
    Some(value[BEARER_SCHEME.len()..].trim().to_string())
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
            .filter(|d| d.rule == "manual_auth_header")
            .collect()
    }

    #[test]
    fn flags_a_bearer_header_on_a_template_endpoint() {
        let src = "ep base(url: \"{{base_url}}\", headers: $[\"Authorization\": \"Bearer {{token}}\"]);\n";
        let target = diagnostics_for(src);
        assert_eq!(target.len(), 1, "got: {target:?}");
        assert!(target[0].message.contains("auth_type.bearer"));
        assert!(target[0].message.contains("token: \"{{token}}\""));
        assert!(target[0]
            .suggested_fix
            .as_ref()
            .expect("fix")
            .contains("[auth(\"api_auth\")]"));
    }

    #[test]
    fn reports_the_position_of_the_header_key() {
        let src = "rq list(\"http://x\", headers: $[\n    \"Authorization\": \"Bearer {{token}}\",\n]);\n";
        let target = diagnostics_for(src);
        assert_eq!(target[0].line, 2);
        assert_eq!(target[0].column, 5);
    }

    #[test]
    fn flags_a_lowercase_header_name() {
        let src = "rq list(\"http://x\", headers: $[\"authorization\": \"bearer {{token}}\"]);\n";
        assert_eq!(diagnostics_for(src).len(), 1);
    }

    #[test]
    fn flags_a_header_declared_in_a_headers_variable() {
        let src = "let common = $[\"Authorization\": \"Bearer {{token}}\"];\nrq list(\"http://x\", headers: common);\n";
        assert_eq!(diagnostics_for(src).len(), 1);
    }

    #[test]
    fn does_not_flag_an_endpoint_using_the_auth_attribute() {
        let src = "auth api_auth(auth_type.bearer) {\n    token: \"{{api_token}}\",\n}\n\n[auth(\"api_auth\")]\nep base(url: \"{{base_url}}\");\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_an_unrelated_header() {
        let src = "rq list(\"http://x\", headers: $[\"X-Trace\": \"Bearer-ish\"]);\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_flag_a_scheme_rq_has_no_provider_for() {
        let src =
            "rq list(\"http://x\", headers: $[\"Authorization\": \"Basic {{credentials}}\"]);\n";
        assert!(
            diagnostics_for(src).is_empty(),
            "rq has no basic provider, so the manual header is the only option"
        );
    }
}
