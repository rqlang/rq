use crate::syntax::auth::{AuthFuture, AuthProvider};
use crate::syntax::error::{AuthError, SyntaxError};
use crate::syntax::token::Token;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use openssl::pkcs12::Pkcs12;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use uuid::Uuid;

const CLIENT_ID_FIELD: &str = "client_id";
const CLIENT_SECRET_FIELD: &str = "client_secret";
const TOKEN_URL_FIELD: &str = "token_url";
const SCOPE_FIELD: &str = "scope";
const CERT_FILE_FIELD: &str = "cert_file";
const CERT_PASSWORD_FIELD: &str = "cert_password";

pub struct OAuth2ClientCredentialsProvider;

impl OAuth2ClientCredentialsProvider {
    pub fn new() -> Self {
        OAuth2ClientCredentialsProvider
    }
}

impl AuthProvider for OAuth2ClientCredentialsProvider {
    fn auth_type(&self) -> &str {
        "oauth2_client_credentials"
    }

    fn validate(&self, name: &str, fields: &HashMap<String, Token>) -> Result<(), SyntaxError> {
        const REQUIRED_FIELDS: &[&str] = &[CLIENT_ID_FIELD, TOKEN_URL_FIELD];
        const OPTIONAL_FIELDS: &[&str] = &[
            CLIENT_SECRET_FIELD,
            SCOPE_FIELD,
            CERT_FILE_FIELD,
            CERT_PASSWORD_FIELD,
        ];

        for field in REQUIRED_FIELDS {
            if !fields.contains_key(*field) {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Client Credentials auth '{name}' missing required field '{field}'"
                    ),
                    0,
                    0,
                    0..0,
                ));
            }
        }

        if !fields.contains_key(CLIENT_SECRET_FIELD) && !fields.contains_key(CERT_FILE_FIELD) {
            return Err(SyntaxError::new(
                format!(
                    "OAuth2 Client Credentials auth '{name}' must have either '{CLIENT_SECRET_FIELD}' or '{CERT_FILE_FIELD}'"
                ),
                0,
                0,
                0..0,
            ));
        }

        for field in REQUIRED_FIELDS {
            if let Some(token) = fields.get(*field) {
                if token.value.trim().is_empty() {
                    return Err(SyntaxError::new(
                        format!(
                            "OAuth2 Client Credentials auth '{name}' has empty '{field}' field"
                        ),
                        0,
                        0,
                        token.span.clone(),
                    ));
                }
            }
        }

        for (field_name, token) in fields {
            if !REQUIRED_FIELDS.contains(&field_name.as_str())
                && !OPTIONAL_FIELDS.contains(&field_name.as_str())
            {
                return Err(SyntaxError::new(
                    format!(
                        "OAuth2 Client Credentials auth '{name}' has unexpected field '{field_name}'. Expected fields: {}, {}",
                        REQUIRED_FIELDS.join(", "),
                        OPTIONAL_FIELDS.join(", ")
                    ),
                    0,
                    0,
                    token.span.clone(),
                ));
            }
        }

        Ok(())
    }

    fn configure<'a>(
        &'a self,
        auth_config: &'a crate::syntax::auth::Config,
        _context: &'a crate::syntax::variable_context::VariableContext,
        url: String,
        mut headers: Vec<(String, String)>,
    ) -> AuthFuture<'a> {
        Box::pin(async move {
            let client_id = &auth_config.fields[CLIENT_ID_FIELD].value;
            let token_url = &auth_config.fields[TOKEN_URL_FIELD].value;
            let scope = auth_config.fields.get(SCOPE_FIELD).map(|t| &t.value);

            let client_secret = auth_config
                .fields
                .get(CLIENT_SECRET_FIELD)
                .map(|t| &t.value);
            let cert_file = auth_config.fields.get(CERT_FILE_FIELD).map(|t| &t.value);
            let cert_password = auth_config
                .fields
                .get(CERT_PASSWORD_FIELD)
                .map(|t| &t.value);

            let client_builder = reqwest::Client::builder().user_agent("rq-cli");
            let mut jwt_assertion = None;
            let mut x5t_thumbprint = None;

            if let Some(cert_path) = cert_file {
                let path = std::path::Path::new(cert_path);
                let resolved_path = if path.is_absolute() {
                    path.to_path_buf()
                } else if let Some(parent) = auth_config.file_path.parent() {
                    parent.join(path)
                } else {
                    path.to_path_buf()
                };

                let cert_content = std::fs::read(&resolved_path).map_err(|e| {
                    AuthError::new(format!(
                        "Failed to read certificate file '{}': {e}",
                        resolved_path.display()
                    ))
                })?;

                // Note: We do NOT attach the identity to the client_builder (mTLS) when using private_key_jwt.
                // Azure and others expect the assertion, but attaching a client cert for mTLS might
                // cause handshake issues if the server isn't configured for it or the cert is self-signed/untrusted.
                // client_builder = client_builder.identity(identity);

                // Check if likely PEM (contains "-----BEGIN")
                let cert_str = String::from_utf8_lossy(&cert_content);
                let (cert_der, private_key_pem_bytes) = if cert_str.contains("-----BEGIN") {
                    let pems = pem::parse_many(&cert_content)
                        .map_err(|e| AuthError::new(format!("Failed to parse PEM file: {e}")))?;

                    let cert_pem =
                        pems.iter()
                            .find(|p| p.tag() == "CERTIFICATE")
                            .ok_or_else(|| {
                                AuthError::new("No CERTIFICATE found in PEM file".to_string())
                            })?;

                    let key_pem = pems
                        .iter()
                        .find(|p| p.tag().contains("PRIVATE KEY"))
                        .ok_or_else(|| {
                            AuthError::new(
                                "No PRIVATE KEY found in PEM file. Ensure it is in the file and unencrypted."
                                    .to_string(),
                            )
                        })?;
                    (
                        cert_pem.contents().to_vec(),
                        pem::encode(key_pem).into_bytes(),
                    )
                } else {
                    // Try parsing as P12/PFX using OpenSSL (robust for AES encrypted PFX)
                    let pass = cert_password.map(|s| s.as_str()).unwrap_or("");
                    let pkcs12 = Pkcs12::from_der(&cert_content)
                        .and_then(|p12| p12.parse2(pass))
                        .map_err(|e| AuthError::new(format!("OpenSSL failed to parse P12: {e}")))?;

                    let cert = pkcs12
                        .cert
                        .ok_or_else(|| AuthError::new("No certificate found in P12".to_string()))?;
                    let pkey = pkcs12
                        .pkey
                        .ok_or_else(|| AuthError::new("No private key found in P12".to_string()))?;

                    // Convert to DER to match the PEM path output
                    let cert_der = cert
                        .to_der()
                        .map_err(|e| AuthError::new(format!("Failed to export cert DER: {e}")))?;

                    let key_pem = pkey
                        .private_key_to_pem_pkcs8()
                        .map_err(|e| AuthError::new(format!("Failed to export key PEM: {e}")))?;

                    (cert_der, key_pem)
                };

                // Calculate x5t (SHA-1 thumbprint)
                let mut hasher = Sha1::new();
                hasher.update(&cert_der);
                let digest = hasher.finalize();
                let x5t = URL_SAFE_NO_PAD.encode(digest);

                x5t_thumbprint = Some(x5t.clone());

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as usize;

                let claims = serde_json::json!({
                    "iss": client_id,
                    "sub": client_id,
                    "aud": token_url,
                    "jti": Uuid::new_v4().to_string(),
                    "nbf": now - 60, // Allow for 1 minute clock skew
                    "exp": now + 300
                });

                let mut header = Header::new(Algorithm::RS256);
                header.x5t = Some(x5t);
                header.typ = Some("JWT".to_string());

                let token = encode(
                    &header,
                    &claims,
                    &EncodingKey::from_rsa_pem(&private_key_pem_bytes)
                        .map_err(|e| AuthError::new(format!("Failed to sign JWT: {e}")))?,
                )
                .map_err(|e| AuthError::new(format!("Failed to sign JWT: {e}")))?;

                jwt_assertion = Some(token);
            }

            let client = client_builder
                .build()
                .map_err(|e| AuthError::new(format!("Failed to build http client: {e}")))?;

            let mut params = HashMap::new();
            params.insert("grant_type", "client_credentials".to_string());
            params.insert("client_id", client_id.clone());

            if let Some(assertion) = jwt_assertion {
                params.insert(
                    "client_assertion_type",
                    "urn:ietf:params:oauth:client-assertion-type:jwt-bearer".to_string(),
                );
                params.insert("client_assertion", assertion);
            } else if let Some(secret) = client_secret {
                params.insert("client_secret", secret.clone());
            }

            if let Some(s) = scope {
                params.insert("scope", s.clone());
            }

            let response = client.post(token_url).form(&params).send().await?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                let x5t_info = x5t_thumbprint
                    .map(|t| format!(" (x5t used: {t})"))
                    .unwrap_or_default();

                return Err(AuthError::new(format!(
                    "OAuth2 Client Credentials failed.\n  URL: {token_url}\n  Client ID: {client_id}\n  Status: {status}\n  Response: {text}{x5t_info}"
                ))
                .into());
            }

            let token_response: serde_json::Value = response.json().await?;

            let access_token = token_response["access_token"]
                .as_str()
                .ok_or_else(|| AuthError::new("No access_token in response".to_string()))?;

            headers.push((
                reqwest::header::AUTHORIZATION.as_str().to_string(),
                format!("Bearer {access_token}"),
            ));

            Ok((url, headers))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::token::{Token, TokenType};

    fn t(s: &str) -> Token {
        Token {
            token_type: TokenType::String,
            value: s.to_string(),
            span: 0..0,
        }
    }

    #[test]
    fn test_client_credentials_type() {
        let provider = OAuth2ClientCredentialsProvider::new();
        assert_eq!(provider.auth_type(), "oauth2_client_credentials");
    }

    #[test]
    fn test_valid_client_credentials() {
        let provider = OAuth2ClientCredentialsProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(CLIENT_SECRET_FIELD.to_string(), t("my-secret"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_client_credentials_missing_field() {
        let provider = OAuth2ClientCredentialsProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        // Missing secret and token_url

        assert!(provider.validate("test_auth", &fields).is_err());
    }

    #[test]
    fn test_client_credentials_empty_field() {
        let provider = OAuth2ClientCredentialsProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t(""));
        fields.insert(CLIENT_SECRET_FIELD.to_string(), t("my-secret"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_err());
    }

    #[test]
    fn test_valid_client_credentials_with_cert() {
        let provider = OAuth2ClientCredentialsProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(CERT_FILE_FIELD.to_string(), t("/path/to/cert.p12"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        assert!(provider.validate("test_auth", &fields).is_ok());
    }

    #[test]
    fn test_client_credentials_missing_secret_and_cert() {
        let provider = OAuth2ClientCredentialsProvider::new();
        let mut fields = HashMap::new();
        fields.insert(CLIENT_ID_FIELD.to_string(), t("my-client"));
        fields.insert(TOKEN_URL_FIELD.to_string(), t("https://example.com/token"));

        let result = provider.validate("test_auth", &fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must have either"));
    }
}
