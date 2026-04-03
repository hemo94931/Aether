//! Standard contract planning surface.
//!
//! This groups the public standard matrix in one place:
//! request-side conversion, matrix registry, and local standard execution entrypoints.

use axum::body::Body;
use axum::http::Response;

use crate::gateway::{
    AppState, GatewayControlDecision, GatewayControlSyncDecisionResponse, GatewayError,
};

pub(crate) mod claude;
mod family;
pub(crate) mod gemini;
mod matrix;
pub(crate) mod openai;

pub(crate) use crate::gateway::ai_pipeline::conversion::{
    build_core_error_body_for_client_format, request_conversion_kind,
    request_conversion_transport_supported, sync_chat_response_conversion_kind,
    sync_cli_response_conversion_kind, RequestConversionKind, SyncChatResponseConversionKind,
    SyncCliResponseConversionKind,
};
pub(crate) use self::matrix::{
    build_standard_request_body, build_standard_upstream_url,
    normalize_standard_request_to_openai_chat_request,
};
pub(crate) use self::openai::{
    copy_request_number_field, copy_request_number_field_as,
    map_openai_reasoning_effort_to_claude_output, map_openai_reasoning_effort_to_gemini_budget,
    maybe_build_stream_local_decision_payload, maybe_build_sync_local_decision_payload,
    maybe_execute_stream_via_local_decision, maybe_execute_sync_via_local_decision,
    parse_openai_stop_sequences, resolve_openai_chat_max_tokens, value_as_u64,
};
pub(crate) use self::openai::{
    maybe_build_stream_local_openai_cli_decision_payload,
    maybe_build_sync_local_openai_cli_decision_payload,
    maybe_execute_stream_via_local_openai_cli_decision,
    maybe_execute_sync_via_local_openai_cli_decision,
};

pub(crate) async fn maybe_execute_sync_via_local_standard_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    if let Some(response) = self::claude::maybe_execute_sync_via_local_claude_decision(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await?
    {
        return Ok(Some(response));
    }

    self::gemini::maybe_execute_sync_via_local_gemini_decision(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await
}

pub(crate) async fn maybe_execute_stream_via_local_standard_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    if let Some(response) = self::claude::maybe_execute_stream_via_local_claude_decision(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await?
    {
        return Ok(Some(response));
    }

    self::gemini::maybe_execute_stream_via_local_gemini_decision(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await
}

pub(crate) async fn maybe_build_sync_local_standard_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    if let Some(payload) = self::claude::maybe_build_sync_local_claude_decision_payload(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await?
    {
        return Ok(Some(payload));
    }

    self::gemini::maybe_build_sync_local_gemini_decision_payload(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await
}

pub(crate) async fn maybe_build_stream_local_standard_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    if let Some(payload) = self::claude::maybe_build_stream_local_claude_decision_payload(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await?
    {
        return Ok(Some(payload));
    }

    self::gemini::maybe_build_stream_local_gemini_decision_payload(
        state, parts, trace_id, decision, body_json, plan_kind,
    )
    .await
}
