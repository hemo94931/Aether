use axum::body::{Body, Bytes};
use axum::http::Response;

use crate::gateway::intent;
use crate::gateway::{AppState, GatewayControlDecision, GatewayError};

pub(crate) async fn maybe_execute_sync_local_path(
    state: &AppState,
    parts: &http::request::Parts,
    body_bytes: &Bytes,
    trace_id: &str,
    decision: &GatewayControlDecision,
) -> Result<Option<Response<Body>>, GatewayError> {
    intent::maybe_execute_via_sync_intent_path(state, parts, body_bytes, trace_id, decision).await
}

pub(crate) async fn maybe_execute_stream_local_path(
    state: &AppState,
    parts: &http::request::Parts,
    body_bytes: &Bytes,
    trace_id: &str,
    decision: &GatewayControlDecision,
) -> Result<Option<Response<Body>>, GatewayError> {
    intent::maybe_execute_via_stream_intent_path(state, parts, body_bytes, trace_id, decision).await
}
