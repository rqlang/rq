mod auth_config;
mod auth_config_bearer;
mod auth_config_oauth2_authorization_code;
mod auth_config_oauth2_client_credentials;
mod auth_config_oauth2_implicit;

pub use auth_config::AuthConfig;
pub use auth_config_bearer::BearerAuthConfig;
pub use auth_config_oauth2_authorization_code::OAuth2AuthorizationCodeConfig;
pub use auth_config_oauth2_client_credentials::OAuth2ClientCredentialsConfig;
pub use auth_config_oauth2_implicit::OAuth2ImplicitConfig;

use crate::syntax::error::SyntaxError;
use crate::syntax::token::Token;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthType {
    Bearer,
    OAuth2AuthorizationCode,
    OAuth2ClientCredentials,
    OAuth2Implicit,
}

impl AuthType {
    pub fn all() -> [AuthType; 4] {
        [
            AuthType::Bearer,
            AuthType::OAuth2AuthorizationCode,
            AuthType::OAuth2ClientCredentials,
            AuthType::OAuth2Implicit,
        ]
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<AuthType, SyntaxError> {
        match s {
            "bearer" => Ok(AuthType::Bearer),
            "oauth2_authorization_code" => Ok(AuthType::OAuth2AuthorizationCode),
            "oauth2_client_credentials" => Ok(AuthType::OAuth2ClientCredentials),
            "oauth2_implicit" => Ok(AuthType::OAuth2Implicit),
            _ => Err(SyntaxError {
                message: format!("Unknown auth type: {s}"),
                line: 0,
                column: 0,
                span: 0..0,
                file_path: None,
            }),
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            AuthType::Bearer => "bearer",
            AuthType::OAuth2AuthorizationCode => "oauth2_authorization_code",
            AuthType::OAuth2ClientCredentials => "oauth2_client_credentials",
            AuthType::OAuth2Implicit => "oauth2_implicit",
        }
    }

    pub fn get_config(&self) -> &'static dyn AuthConfig {
        match self {
            AuthType::Bearer => &BearerAuthConfig,
            AuthType::OAuth2AuthorizationCode => &OAuth2AuthorizationCodeConfig,
            AuthType::OAuth2ClientCredentials => &OAuth2ClientCredentialsConfig,
            AuthType::OAuth2Implicit => &OAuth2ImplicitConfig,
        }
    }

    pub fn required_fields(&self) -> &'static [&'static str] {
        self.get_config().required_fields()
    }

    pub fn optional_fields(&self) -> &'static [&'static str] {
        self.get_config().optional_fields()
    }

    pub fn one_of_fields(&self) -> &'static [&'static [&'static str]] {
        self.get_config().one_of_fields()
    }

    pub fn fields(&self) -> Vec<(&'static str, bool)> {
        self.required_fields()
            .iter()
            .map(|field| (*field, true))
            .chain(self.optional_fields().iter().map(|field| (*field, false)))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub name: String,
    pub auth_type: AuthType,
    pub fields: HashMap<String, Token>,
    pub file_path: PathBuf,
    pub line: usize,
    pub character: usize,
}

impl Config {
    pub fn validate(&self) -> Result<(), SyntaxError> {
        let auth_config = self.auth_type.get_config();
        auth_config.validate(&self.name, &self.fields)
    }

    pub fn apply_defaults(&mut self) {
        match self.auth_type {
            AuthType::OAuth2AuthorizationCode => {
                if !self.fields.contains_key("code_challenge_method") {
                    self.fields.insert(
                        "code_challenge_method".to_string(),
                        Token {
                            token_type: crate::syntax::token::TokenType::String,
                            value: "S256".to_string(),
                            span: 0..0,
                        },
                    );
                }
            }
            AuthType::OAuth2Implicit => {}
            AuthType::Bearer => {}
            AuthType::OAuth2ClientCredentials => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::token::TokenType;

    fn t(s: &str) -> Token {
        Token {
            token_type: TokenType::String,
            value: s.to_string(),
            span: 0..0,
        }
    }

    #[test]
    fn test_auth_type_from_str() {
        assert_eq!(AuthType::from_str("bearer").unwrap(), AuthType::Bearer);
        assert_eq!(
            AuthType::from_str("oauth2_authorization_code").unwrap(),
            AuthType::OAuth2AuthorizationCode
        );
    }

    #[test]
    fn test_auth_type_from_str_unknown() {
        assert!(AuthType::from_str("unknown").is_err());
    }

    #[test]
    fn test_auth_type_as_str() {
        assert_eq!(AuthType::Bearer.as_str(), "bearer");
        assert_eq!(
            AuthType::OAuth2AuthorizationCode.as_str(),
            "oauth2_authorization_code"
        );
    }

    #[test]
    fn test_get_config_bearer() {
        let auth_type = AuthType::Bearer;
        let config = auth_type.get_config();
        assert_eq!(config.auth_type(), "bearer");
    }

    #[test]
    fn test_get_config_oauth2_authorization_code() {
        let auth_type = AuthType::OAuth2AuthorizationCode;
        let config = auth_type.get_config();
        assert_eq!(config.auth_type(), "oauth2_authorization_code");
    }

    #[test]
    fn test_fields_list_required_before_optional() {
        assert_eq!(
            AuthType::OAuth2Implicit.fields(),
            vec![
                ("client_id", true),
                ("authorization_url", true),
                ("redirect_uri", true),
                ("scope", false)
            ]
        );
    }

    #[test]
    fn test_interactive_auth_types_require_redirect_uri() {
        assert!(AuthType::OAuth2AuthorizationCode
            .required_fields()
            .contains(&"redirect_uri"));
        assert!(AuthType::OAuth2Implicit
            .required_fields()
            .contains(&"redirect_uri"));
    }

    #[test]
    fn test_config_validate_success() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("my-token"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_failure() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("   ")); // Empty token

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_apply_defaults_oauth2() {
        let mut fields = HashMap::new();
        fields.insert("client_id".to_string(), t("my-client"));
        fields.insert(
            "authorization_url".to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert("token_url".to_string(), t("https://auth.example.com/token"));

        let mut config = Config {
            name: "test_oauth".to_string(),
            auth_type: AuthType::OAuth2AuthorizationCode,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        config.apply_defaults();

        assert_eq!(
            config.fields.get("code_challenge_method").unwrap().value,
            "S256"
        );
    }

    #[test]
    fn test_config_apply_defaults_oauth2_preserves_existing() {
        let mut fields = HashMap::new();
        fields.insert("client_id".to_string(), t("my-client"));
        fields.insert(
            "authorization_url".to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert("token_url".to_string(), t("https://auth.example.com/token"));
        fields.insert(
            "redirect_uri".to_string(),
            t("http://localhost:3000/callback"),
        );
        fields.insert("code_challenge_method".to_string(), t("plain"));

        let mut config = Config {
            name: "test_oauth".to_string(),
            auth_type: AuthType::OAuth2AuthorizationCode,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        config.apply_defaults();

        assert_eq!(
            config.fields.get("redirect_uri").unwrap().value,
            "http://localhost:3000/callback"
        );
        assert_eq!(
            config.fields.get("code_challenge_method").unwrap().value,
            "plain"
        );
    }

    #[test]
    fn test_config_apply_defaults_oauth2_leaves_redirect_uri_unset() {
        let mut fields = HashMap::new();
        fields.insert("client_id".to_string(), t("my-client"));
        fields.insert(
            "authorization_url".to_string(),
            t("https://auth.example.com/authorize"),
        );
        fields.insert("token_url".to_string(), t("https://auth.example.com/token"));

        let mut config = Config {
            name: "test_oauth".to_string(),
            auth_type: AuthType::OAuth2AuthorizationCode,
            fields,
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        config.apply_defaults();

        assert!(!config.fields.contains_key("redirect_uri"));
    }

    #[test]
    fn test_config_apply_defaults_bearer_no_changes() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("my-token"));

        let mut config = Config {
            name: "test_bearer".to_string(),
            auth_type: AuthType::Bearer,
            fields: fields.clone(),
            file_path: PathBuf::new(),
            line: 0,
            character: 0,
        };

        config.apply_defaults();

        assert_eq!(config.fields, fields);
    }
}
