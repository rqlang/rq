use crate::error::RqError;
use crate::syntax::Request;
use std::future::Future;
use std::pin::Pin;

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
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RqError>> + Send + 'a>>;
}
