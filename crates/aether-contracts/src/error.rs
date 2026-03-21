use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionErrorKind {
    ConnectTimeout,
    FirstByteTimeout,
    ReadTimeout,
    Upstream4xx,
    Upstream5xx,
    TlsError,
    ProxyError,
    ProtocolError,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    Connect,
    Handshake,
    Write,
    FirstByte,
    StreamRead,
    Decode,
    Finalize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionError {
    pub kind: ExecutionErrorKind,
    pub phase: ExecutionPhase,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_status: Option<u16>,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub failover_recommended: bool,
}
