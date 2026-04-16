use super::auth_provider::{AuthFuture, AuthProvider};
use crate::syntax::error::AuthError;
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
        Self
    }
}

impl Default for OAuth2ClientCredentialsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthProvider for OAuth2ClientCredentialsProvider {
    fn auth_type(&self) -> &str {
        "oauth2_client_credentials"
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
                    let pass = match cert_password {
                        Some(s) => s.as_str(),
                        None => "",
                    };
                    let pkcs12 = Pkcs12::from_der(&cert_content)
                        .and_then(|p12| p12.parse2(pass))
                        .map_err(|e| AuthError::new(format!("OpenSSL failed to parse P12: {e}")))?;

                    let cert = pkcs12
                        .cert
                        .ok_or_else(|| AuthError::new("No certificate found in P12".to_string()))?;
                    let pkey = pkcs12
                        .pkey
                        .ok_or_else(|| AuthError::new("No private key found in P12".to_string()))?;

                    let cert_der = cert
                        .to_der()
                        .map_err(|e| AuthError::new(format!("Failed to export cert DER: {e}")))?;

                    let key_pem = pkey
                        .private_key_to_pem_pkcs8()
                        .map_err(|e| AuthError::new(format!("Failed to export key PEM: {e}")))?;

                    (cert_der, key_pem)
                };

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
                    "iat": now,
                    "nbf": now - 60,
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

    #[test]
    fn test_client_credentials_type() {
        let executor = OAuth2ClientCredentialsProvider::new();
        assert_eq!(executor.auth_type(), "oauth2_client_credentials");
    }
}
