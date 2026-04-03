use axum::body::Body;
use axum::http::Response;

use crate::gateway::{
    AppState, GatewayControlDecision, GatewayControlSyncDecisionResponse, GatewayError,
};

use super::family::{
    maybe_build_stream_via_standard_family_payload, maybe_build_sync_via_standard_family_payload,
    maybe_execute_stream_via_standard_family_decision,
    maybe_execute_sync_via_standard_family_decision,
};
pub(crate) use crate::gateway::ai_pipeline::conversion::request::normalize_gemini_request_to_openai_chat_request;

mod chat;
mod cli;

pub(crate) async fn maybe_execute_sync_via_local_gemini_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    maybe_execute_sync_via_standard_family_decision(
        state,
        parts,
        trace_id,
        decision,
        body_json,
        plan_kind,
        |plan_kind| {
            chat::resolve_sync_spec(plan_kind).or_else(|| cli::resolve_sync_spec(plan_kind))
        },
    )
    .await
}

pub(crate) async fn maybe_execute_stream_via_local_gemini_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    maybe_execute_stream_via_standard_family_decision(
        state,
        parts,
        trace_id,
        decision,
        body_json,
        plan_kind,
        |plan_kind| {
            chat::resolve_stream_spec(plan_kind).or_else(|| cli::resolve_stream_spec(plan_kind))
        },
    )
    .await
}

pub(crate) async fn maybe_build_sync_local_gemini_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    maybe_build_sync_via_standard_family_payload(
        state,
        parts,
        trace_id,
        decision,
        body_json,
        plan_kind,
        |plan_kind| {
            chat::resolve_sync_spec(plan_kind).or_else(|| cli::resolve_sync_spec(plan_kind))
        },
    )
    .await
}

pub(crate) async fn maybe_build_stream_local_gemini_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    maybe_build_stream_via_standard_family_payload(
        state,
        parts,
        trace_id,
        decision,
        body_json,
        plan_kind,
        |plan_kind| {
            chat::resolve_stream_spec(plan_kind).or_else(|| cli::resolve_stream_spec(plan_kind))
        },
    )
    .await
}
