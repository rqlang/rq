use crate::error::RqError;
use crate::http::{HttpClient, HttpResponse};
use crate::syntax::Request;
use std::pin::Pin;

pub struct ReqwestHttpClient;

impl HttpClient for ReqwestHttpClient {
    fn execute<'a>(
        &'a self,
        request: &'a Request,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<HttpResponse, RqError>> + Send + 'a>> {
        Box::pin(async move { execute_with_reqwest(request).await })
    }
}

async fn execute_with_reqwest(request: &Request) -> Result<HttpResponse, RqError> {
    let client = reqwest::Client::new();
    let method = to_reqwest_method(&request.method);
    let mut req_builder = client.request(method, &request.url);

    req_builder = req_builder.header(
        reqwest::header::USER_AGENT,
        &format!("rq/{}", crate::version::app_version()),
    );

    for (key, value) in &request.headers {
        req_builder = req_builder.header(key, value);
    }

    if let Some(body) = &request.body {
        if is_json_content(body) {
            req_builder = req_builder.header(reqwest::header::CONTENT_TYPE, "application/json");
        }
        req_builder = req_builder.body(body.clone());
    }

    if let Some(timeout_str) = &request.timeout {
        if let Ok(secs) = timeout_str.parse::<f64>() {
            req_builder = req_builder.timeout(std::time::Duration::from_secs_f64(secs));
        } else {
            return Err(RqError::Generic(format!(
                "HTTP Error: Timeout value '{timeout_str}' must be a number"
            )));
        }
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| RqError::Network(error_chain(&e)))?;
    let status = response.status().as_u16();

    let mut headers = std::collections::HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            headers.insert(key.to_string(), value_str.to_string());
        }
    }

    let body = response
        .text()
        .await
        .map_err(|e| RqError::Network(error_chain(&e)))?;

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn to_reqwest_method(method: &crate::syntax::http_method::HttpMethod) -> reqwest::Method {
    use crate::syntax::http_method::HttpMethod;
    match method {
        HttpMethod::GET => reqwest::Method::GET,
        HttpMethod::POST => reqwest::Method::POST,
        HttpMethod::PUT => reqwest::Method::PUT,
        HttpMethod::DELETE => reqwest::Method::DELETE,
        HttpMethod::PATCH => reqwest::Method::PATCH,
        HttpMethod::HEAD => reqwest::Method::HEAD,
        HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
    }
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    msg
}

fn is_json_content(body: &str) -> bool {
    let trimmed = body.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}
