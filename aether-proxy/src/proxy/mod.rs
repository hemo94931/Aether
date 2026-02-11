pub mod connect;
pub mod delegate;
pub mod delegate_client;
pub mod server;
pub mod target_filter;
pub mod tls;

use http_body_util::BodyExt;

/// Boxed body type used across proxy handlers.
pub type BoxBody =
    http_body_util::combinators::BoxBody<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>;

/// Create an empty [`BoxBody`] (for error responses, 405, etc.).
pub fn empty_box_body() -> BoxBody {
    http_body_util::Full::new(bytes::Bytes::new())
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { match e {} })
        .boxed()
}
