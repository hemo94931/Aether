use axum::body::Body;
use axum::http::Response;

use crate::gateway::ai_pipeline::planner::family_core::{
    execute_stream_plan_and_reports, execute_sync_plan_and_reports,
};
use crate::gateway::ai_pipeline::planner::plan_builders::{
    LocalStreamPlanAndReport, LocalSyncPlanAndReport,
};
use crate::gateway::{AppState, GatewayControlDecision, GatewayControlSyncDecisionResponse, GatewayError};

use super::super::plans::{
    build_local_stream_plan_and_reports, build_local_sync_plan_and_reports, resolve_stream_spec,
    resolve_sync_spec,
};
use super::candidates::{
    materialize_local_same_format_provider_candidate_attempts,
    resolve_local_same_format_provider_decision_input,
};
use super::payload::maybe_build_local_same_format_provider_decision_payload_for_candidate;

pub(crate) async fn maybe_execute_sync_via_local_same_format_provider_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(spec) = resolve_sync_spec(plan_kind) else {
        return Ok(None);
    };

    let plan_and_reports =
        build_local_sync_plan_and_reports(state, parts, trace_id, decision, body_json, spec)
            .await?;
    if plan_and_reports.is_empty() {
        return Ok(None);
    }

    execute_sync_plan_and_reports(state, parts, trace_id, decision, plan_kind, plan_and_reports)
        .await
}

pub(crate) async fn maybe_execute_stream_via_local_same_format_provider_decision(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(spec) = resolve_stream_spec(plan_kind) else {
        return Ok(None);
    };

    let plan_and_reports =
        build_local_stream_plan_and_reports(state, parts, trace_id, decision, body_json, spec)
            .await?;
    if plan_and_reports.is_empty() {
        return Ok(None);
    }

    execute_stream_plan_and_reports(state, trace_id, decision, plan_kind, plan_and_reports).await
}

pub(crate) async fn maybe_build_sync_local_same_format_provider_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    let Some(spec) = resolve_sync_spec(plan_kind) else {
        return Ok(None);
    };

    let Some(input) = resolve_local_same_format_provider_decision_input(
        state, parts, trace_id, decision, body_json, spec,
    )
    .await
    else {
        return Ok(None);
    };

    let attempts =
        materialize_local_same_format_provider_candidate_attempts(state, trace_id, &input, spec)
            .await?;

    for attempt in attempts {
        if let Some(payload) =
            maybe_build_local_same_format_provider_decision_payload_for_candidate(
                state, parts, trace_id, body_json, &input, attempt, spec,
            )
            .await
        {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}

pub(crate) async fn maybe_build_stream_local_same_format_provider_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    let Some(spec) = resolve_stream_spec(plan_kind) else {
        return Ok(None);
    };

    let Some(input) = resolve_local_same_format_provider_decision_input(
        state, parts, trace_id, decision, body_json, spec,
    )
    .await
    else {
        return Ok(None);
    };

    let attempts =
        materialize_local_same_format_provider_candidate_attempts(state, trace_id, &input, spec)
            .await?;

    for attempt in attempts {
        if let Some(payload) =
            maybe_build_local_same_format_provider_decision_payload_for_candidate(
                state, parts, trace_id, body_json, &input, attempt, spec,
            )
            .await
        {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}
