use http::method::InvalidMethod;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorClientError {
    #[error("executor endpoint is not configured")]
    MissingEndpoint,
    #[error("executor request is not implemented yet")]
    Unimplemented,
    #[error("failed to encode NDJSON frame: {0}")]
    Encode(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ExecutorServiceError {
    #[error("stream execution is not implemented yet")]
    StreamUnsupported,
    #[error("request body must contain json_body or body_bytes_b64")]
    RequestBodyRequired,
    #[error("request body base64 is invalid: {0}")]
    BodyDecode(base64::DecodeError),
    #[error("request content-encoding is not supported: {0}")]
    UnsupportedContentEncoding(String),
    #[error("proxy execution is not implemented yet")]
    ProxyUnsupported,
    #[error("tls profile overrides are not implemented yet")]
    TlsProfileUnsupported,
    #[error("tunnel delegate execution is not implemented yet")]
    DelegateUnsupported,
    #[error("invalid method: {0}")]
    InvalidMethod(#[from] InvalidMethod),
    #[error("invalid upstream header name: {0}")]
    InvalidHeaderName(String),
    #[error("invalid upstream header value for {0}")]
    InvalidHeaderValue(String),
    #[error("invalid proxy configuration: {0}")]
    InvalidProxy(reqwest::Error),
    #[error("failed to encode request body: {0}")]
    BodyEncode(serde_json::Error),
    #[error("failed to build HTTP client: {0}")]
    ClientBuild(reqwest::Error),
    #[error("failed to execute upstream request: {0}")]
    UpstreamRequest(reqwest::Error),
    #[error("hub relay request failed: {0}")]
    RelayError(String),
    #[error("upstream response is not valid JSON: {0}")]
    InvalidJson(serde_json::Error),
}
