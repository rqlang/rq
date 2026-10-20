use crate::error::RqError;
use crate::syntax::Request;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

pub trait HttpClient: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: &'a Request,
        timeout: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RqError>> + Send + 'a>>;
}

pub(crate) fn request_timeout(request: &Request) -> Result<Option<Duration>, RqError> {
    let Some(timeout) = &request.timeout else {
        return Ok(None);
    };
    let secs = timeout.parse::<f64>().map_err(|_| {
        RqError::Generic(format!(
            "HTTP Error: Timeout value '{timeout}' must be a number"
        ))
    })?;
    if !secs.is_finite() || secs < 0.0 {
        return Err(RqError::Generic(format!(
            "HTTP Error: Timeout value '{timeout}' must be a non-negative finite number"
        )));
    }
    Duration::try_from_secs_f64(secs).map(Some).map_err(|_| {
        RqError::Generic(format!(
            "HTTP Error: Timeout value '{timeout}' is too large"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::request_timeout;
    use crate::syntax::http_method::HttpMethod;
    use crate::syntax::Request;
    use std::time::Duration;

    fn request(timeout: Option<&str>) -> Request {
        Request {
            name: "get".to_string(),
            url: "http://localhost:8080".to_string(),
            raw_url: "http://localhost:8080".to_string(),
            method: HttpMethod::GET,
            headers: Vec::new(),
            body: None,
            body_type: None,
            headers_var: None,
            slot_bindings: Vec::new(),
            endpoint: None,
            auth: None,
            auth_location: None,
            timeout: timeout.map(str::to_string),
            required_variables: Vec::new(),
            source_path: None,
            related_files: Vec::new(),
            line: 0,
            character: 0,
        }
    }

    fn with_timeout(timeout: &str) -> Request {
        request(Some(timeout))
    }

    #[test]
    fn a_numeric_timeout_becomes_a_duration_in_seconds() {
        let target = request_timeout(&with_timeout("1.5"));
        assert_eq!(target.ok(), Some(Some(Duration::from_millis(1500))));
    }

    #[test]
    fn a_request_without_timeout_has_none() {
        let target = request_timeout(&request(None));
        assert_eq!(target.ok(), Some(None));
    }

    #[test]
    fn a_non_numeric_timeout_is_rejected() {
        let target = request_timeout(&with_timeout("soon"));
        assert!(target.is_err_and(|e| e.to_string().contains("must be a number")));
    }

    #[test]
    fn a_timeout_too_large_for_a_duration_is_rejected() {
        let target = request_timeout(&with_timeout("1e30"));
        assert!(target.is_err_and(|e| e.to_string().contains("is too large")));
    }

    #[test]
    fn a_negative_timeout_is_rejected() {
        let target = request_timeout(&with_timeout("-1"));
        assert!(target.is_err_and(|e| e.to_string().contains("non-negative finite number")));
    }
}
