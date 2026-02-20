use crate::core::logger::Logger;
use crate::syntax::Request;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct HttpError {
    pub message: String,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP Error: {}", self.message)
    }
}

impl Error for HttpError {}

impl From<reqwest::Error> for HttpError {
    fn from(error: reqwest::Error) -> Self {
        HttpError {
            message: error.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

pub async fn execute_request(request: &Request) -> Result<HttpResponse, HttpError> {
    Logger::debug(&format!(
        "Executing {} request '{}' to URL: {}",
        request.method.as_str(),
        request.name,
        request.url
    ));

    let client = reqwest::Client::new();
    let mut req_builder = client.request(request.method.to_reqwest_method(), &request.url);

    req_builder = req_builder.header(
        reqwest::header::USER_AGENT,
        &format!("rq/{}", crate::core::version::app_version()),
    );

    for (key, value) in &request.headers {
        Logger::debug(&format!("Adding header: {key}: {value}"));
        req_builder = req_builder.header(key, value);
    }

    if let Some(body) = &request.body {
        if crate::core::json::is_json_content(body) {
            req_builder = req_builder.header(
                reqwest::header::CONTENT_TYPE,
                crate::core::json::CONTENT_TYPE,
            );
        }
        req_builder = req_builder.body(body.clone());
    }

    if let Some(timeout_str) = &request.timeout {
        if let Ok(secs) = timeout_str.parse::<f64>() {
            req_builder = req_builder.timeout(std::time::Duration::from_secs_f64(secs));
        } else {
            return Err(HttpError {
                message: format!("Timeout value '{timeout_str}' must be a number"),
            });
        }
    }

    let response = req_builder.send().await?;
    let status = response.status().as_u16();

    let mut headers = std::collections::HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            headers.insert(key.to_string(), value_str.to_string());
        }
    }

    let body = response.text().await?;

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}
