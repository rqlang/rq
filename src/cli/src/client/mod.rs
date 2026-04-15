mod native;

use native::{NativeFs, NativeSecretProvider, ReqwestHttpClient};
use std::sync::Arc;

pub use rq_lib::client::models::{RequestExecutionResult, RqConfig};
pub use rq_lib::client::RqClient;

pub fn make_client(config: RqConfig) -> RqClient {
    RqClient::new(
        config,
        Arc::new(NativeFs),
        Arc::new(NativeSecretProvider),
        Arc::new(ReqwestHttpClient),
    )
}

pub fn make_listing_client() -> RqClient {
    make_client(RqConfig {
        source_path: String::new(),
        request_name: None,
        environment: None,
        variables: vec![],
        output_format: "text".to_string(),
    })
}
