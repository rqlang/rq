mod auth_provider;
mod auth_provider_bearer;
mod auth_provider_oauth2_authorization_code;
mod auth_provider_oauth2_client_credentials;
mod auth_provider_oauth2_implicit;

pub use auth_provider::AuthFuture;
pub use auth_provider::AuthProvider;
pub use auth_provider_bearer::BearerAuthProvider;
pub use auth_provider_oauth2_authorization_code::OAuth2AuthorizationCodeProvider;
pub use auth_provider_oauth2_client_credentials::OAuth2ClientCredentialsProvider;
pub use auth_provider_oauth2_implicit::OAuth2ImplicitProvider;

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

    pub fn get_config(&self) -> Box<dyn AuthProvider> {
        match self {
            AuthType::Bearer => Box::new(BearerAuthProvider::new()),
            AuthType::OAuth2AuthorizationCode => Box::new(OAuth2AuthorizationCodeProvider::new()),
            AuthType::OAuth2ClientCredentials => Box::new(OAuth2ClientCredentialsProvider::new()),
            AuthType::OAuth2Implicit => Box::new(OAuth2ImplicitProvider::new()),
        }
    }

    pub fn required_fields(&self) -> Vec<&'static str> {
        match self {
            AuthType::Bearer => vec!["token"],
            AuthType::OAuth2AuthorizationCode => {
                vec!["client_id", "authorization_url", "token_url"]
            }
            AuthType::OAuth2ClientCredentials => {
                vec!["client_id", "token_url"]
            }
            AuthType::OAuth2Implicit => vec!["client_id", "authorization_url"],
        }
    }

    pub fn optional_fields(&self) -> Vec<&'static str> {
        match self {
            AuthType::Bearer => vec![],
            AuthType::OAuth2AuthorizationCode => vec![
                "client_secret",
                "redirect_uri",
                "scope",
                "code_challenge_method",
                "use_state",
            ],
            AuthType::OAuth2ClientCredentials => {
                vec!["client_secret", "scope", "cert_file", "cert_password"]
            }
            AuthType::OAuth2Implicit => vec!["redirect_uri", "scope"],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub name: String,
    pub auth_type: AuthType,
    pub fields: HashMap<String, Token>,
    pub file_path: PathBuf,
}

impl Config {
    pub fn validate(&self) -> Result<(), SyntaxError> {
        let auth_provider = self.auth_type.get_config();
        auth_provider.validate(&self.name, &self.fields)
    }

    /// - redirect_uri: "vscode://rq.rq-language/oauth-callback" (if not present)
    /// - code_challenge_method: "S256" (if not present)
    ///   fields.insert("client_id".to_string(), "my-client".to_string());
    ///   name: "oauth".to_string(),
    ///   assert_eq!(config.fields.get("redirect_uri").unwrap(), "vscode://rq.rq-language/auth-callback");
    ///   assert_eq!(config.fields.get("code_challenge_method").unwrap(), "S256");
    pub fn apply_defaults(&mut self) {
        match self.auth_type {
            AuthType::OAuth2AuthorizationCode => {
                if !self.fields.contains_key("redirect_uri") {
                    self.fields.insert(
                        "redirect_uri".to_string(),
                        Token {
                            token_type: crate::syntax::token::TokenType::String,
                            value: "vscode://rq.rq-language/oauth-callback".to_string(),
                            span: 0..0,
                        },
                    );
                }

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
            AuthType::OAuth2Implicit => {
                if !self.fields.contains_key("redirect_uri") {
                    self.fields.insert(
                        "redirect_uri".to_string(),
                        Token {
                            token_type: crate::syntax::token::TokenType::String,
                            value: "vscode://rq.rq-language/oauth-callback".to_string(),
                            span: 0..0,
                        },
                    );
                }
            }
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
    fn test_config_validate_success() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("my-token"));

        let config = Config {
            name: "test_auth".to_string(),
            auth_type: AuthType::Bearer,
            fields,
            file_path: PathBuf::new(),
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
        };

        config.apply_defaults();

        assert_eq!(
            config.fields.get("redirect_uri").unwrap().value,
            "vscode://rq.rq-language/oauth-callback"
        );
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
    fn test_config_apply_defaults_bearer_no_changes() {
        let mut fields = HashMap::new();
        fields.insert("token".to_string(), t("my-token"));

        let mut config = Config {
            name: "test_bearer".to_string(),
            auth_type: AuthType::Bearer,
            fields: fields.clone(),
            file_path: PathBuf::new(),
        };

        config.apply_defaults();

        assert_eq!(config.fields, fields);
    }
}
