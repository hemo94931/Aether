use crate::gateway::{
    execute_execution_runtime_stream, execute_execution_runtime_sync, AppState,
    GatewayControlDecision, GatewayControlSyncDecisionResponse, GatewayError,
};
use axum::body::Body;
use axum::http::Response;

mod decision;
mod plans;

use self::decision::{
    mark_unused_local_openai_cli_candidates, materialize_local_openai_cli_candidate_attempts,
    maybe_build_local_openai_cli_decision_payload_for_candidate,
    resolve_local_openai_cli_decision_input,
};
use self::plans::{
    build_local_stream_plan_and_reports, build_local_sync_plan_and_reports, resolve_stream_spec,
    resolve_sync_spec,
};

pub(crate) async fn maybe_execute_sync_via_local_openai_cli_decision(
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

    let mut remaining = plan_and_reports.into_iter();
    while let Some(plan_and_report) = remaining.next() {
        if let Some(response) = execute_execution_runtime_sync(
            state,
            parts.uri.path(),
            plan_and_report.plan,
            trace_id,
            decision,
            plan_kind,
            plan_and_report.report_kind,
            plan_and_report.report_context,
        )
        .await?
        {
            mark_unused_local_openai_cli_candidates(state, remaining.collect()).await;
            return Ok(Some(response));
        }
    }

    Ok(None)
}

pub(crate) async fn maybe_execute_stream_via_local_openai_cli_decision(
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

    let mut remaining = plan_and_reports.into_iter();
    while let Some(plan_and_report) = remaining.next() {
        if let Some(response) = execute_execution_runtime_stream(
            state,
            plan_and_report.plan,
            trace_id,
            decision,
            plan_kind,
            plan_and_report.report_kind,
            plan_and_report.report_context,
        )
        .await?
        {
            mark_unused_local_openai_cli_candidates(state, remaining.collect()).await;
            return Ok(Some(response));
        }
    }

    Ok(None)
}

pub(crate) async fn maybe_build_sync_local_openai_cli_decision_payload(
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

    let Some(input) =
        resolve_local_openai_cli_decision_input(state, trace_id, decision, body_json).await
    else {
        return Ok(None);
    };

    let attempts =
        materialize_local_openai_cli_candidate_attempts(state, trace_id, &input, spec).await?;

    for attempt in attempts {
        if let Some(payload) = maybe_build_local_openai_cli_decision_payload_for_candidate(
            state, parts, trace_id, body_json, &input, attempt, spec,
        )
        .await
        {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}

pub(crate) async fn maybe_build_stream_local_openai_cli_decision_payload(
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

    let Some(input) =
        resolve_local_openai_cli_decision_input(state, trace_id, decision, body_json).await
    else {
        return Ok(None);
    };

    let attempts =
        materialize_local_openai_cli_candidate_attempts(state, trace_id, &input, spec).await?;

    for attempt in attempts {
        if let Some(payload) = maybe_build_local_openai_cli_decision_payload_for_candidate(
            state, parts, trace_id, body_json, &input, attempt, spec,
        )
        .await
        {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}
