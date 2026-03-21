use std::collections::BTreeMap;
use std::io::Error as IoError;

use aether_contracts::{ExecutionPlan, StreamFrame, StreamFramePayload};
use async_stream::stream;
use axum::body::{Body, Bytes};
use axum::http::Response;
use base64::Engine as _;
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;
use tracing::warn;

use crate::gateway::constants::*;
use crate::gateway::headers::{collect_control_headers, header_equals};
use crate::gateway::{
    build_client_response, build_client_response_from_parts, AppState, GatewayControlAuthContext,
    GatewayControlDecision, GatewayError,
};

const GEMINI_FILES_DOWNLOAD_PLAN_KIND: &str = "gemini_files_download";
const OPENAI_VIDEO_CONTENT_PLAN_KIND: &str = "openai_video_content";
const EXECUTOR_STREAM_ACTION: &str = "executor_stream";
const MAX_ERROR_BODY_BYTES: usize = 16_384;

#[derive(Debug, Serialize)]
struct GatewayControlPlanRequest {
    trace_id: String,
    method: String,
    path: String,
    query_string: Option<String>,
    headers: BTreeMap<String, String>,
    body_json: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_base64: Option<String>,
    auth_context: Option<GatewayControlAuthContext>,
}

#[derive(Debug, Deserialize)]
struct GatewayControlPlanResponse {
    action: String,
    #[serde(default)]
    plan_kind: Option<String>,
    #[serde(default)]
    plan: Option<ExecutionPlan>,
}

pub(crate) async fn maybe_execute_via_executor_stream(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: Option<&GatewayControlDecision>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(control_base_url) = state.control_base_url.as_deref() else {
        return Ok(None);
    };
    let Some(executor_base_url) = state.executor_base_url.as_deref() else {
        return Ok(None);
    };
    let Some(decision) = decision else {
        return Ok(None);
    };

    let Some(plan_kind) = resolve_direct_executor_stream_plan_kind(parts, decision) else {
        return Ok(None);
    };

    let request_payload = GatewayControlPlanRequest {
        trace_id: trace_id.to_string(),
        method: parts.method.to_string(),
        path: parts.uri.path().to_string(),
        query_string: parts.uri.query().map(ToOwned::to_owned),
        headers: collect_control_headers(&parts.headers),
        body_json: json!({}),
        body_base64: None,
        auth_context: decision.auth_context.clone(),
    };

    let response = state
        .client
        .post(format!(
            "{control_base_url}/api/internal/gateway/plan-stream"
        ))
        .header(TRACE_ID_HEADER, trace_id)
        .json(&request_payload)
        .send()
        .await
        .map_err(|err| GatewayError::ControlUnavailable {
            trace_id: trace_id.to_string(),
            message: err.to_string(),
        })?;

    if response.status() == http::StatusCode::CONFLICT
        && header_equals(
            response.headers(),
            CONTROL_ACTION_HEADER,
            CONTROL_ACTION_PROXY_PUBLIC,
        )
    {
        return Ok(None);
    }

    if header_equals(response.headers(), CONTROL_EXECUTED_HEADER, "true")
        && response.status() != http::StatusCode::OK
    {
        return Ok(Some(build_client_response(
            response,
            trace_id,
            Some(decision),
        )?));
    }

    let response = response
        .error_for_status()
        .map_err(|err| GatewayError::ControlUnavailable {
            trace_id: trace_id.to_string(),
            message: err.to_string(),
        })?;

    let payload: GatewayControlPlanResponse = response
        .json()
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;

    if payload.action != EXECUTOR_STREAM_ACTION {
        return Ok(None);
    }

    if payload.plan_kind.as_deref() != Some(plan_kind) {
        return Ok(None);
    }

    let Some(plan) = payload.plan else {
        return Err(GatewayError::Internal(
            "gateway plan response missing execution plan".to_string(),
        ));
    };

    execute_executor_stream(
        state,
        executor_base_url,
        plan,
        trace_id,
        decision,
        plan_kind,
    )
    .await
}

fn resolve_direct_executor_stream_plan_kind(
    parts: &http::request::Parts,
    decision: &GatewayControlDecision,
) -> Option<&'static str> {
    if parts.method != http::Method::GET || decision.route_class.as_deref() != Some("ai_public") {
        return None;
    }

    if decision.route_family.as_deref() == Some("gemini")
        && decision.route_kind.as_deref() == Some("files")
        && parts.uri.path().ends_with(":download")
    {
        return Some(GEMINI_FILES_DOWNLOAD_PLAN_KIND);
    }

    if decision.route_family.as_deref() == Some("openai")
        && decision.route_kind.as_deref() == Some("video")
        && parts.uri.path().ends_with("/content")
    {
        return Some(OPENAI_VIDEO_CONTENT_PLAN_KIND);
    }

    None
}

async fn execute_executor_stream(
    state: &AppState,
    executor_base_url: &str,
    plan: ExecutionPlan,
    trace_id: &str,
    decision: &GatewayControlDecision,
    plan_kind: &str,
) -> Result<Option<Response<Body>>, GatewayError> {
    let response = match state
        .client
        .post(format!("{executor_base_url}/v1/execute/stream"))
        .header(TRACE_ID_HEADER, trace_id)
        .json(&plan)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            warn!(trace_id = %trace_id, error = %err, "gateway direct executor stream unavailable");
            return Ok(None);
        }
    };

    if response.status() != http::StatusCode::OK {
        return Ok(Some(build_client_response(
            response,
            trace_id,
            Some(decision),
        )?));
    }

    let stream = response
        .bytes_stream()
        .map_err(|err| IoError::other(err.to_string()));
    let reader = StreamReader::new(stream);
    let mut lines = FramedRead::new(reader, LinesCodec::new());

    let first_frame = read_next_frame(&mut lines).await?.ok_or_else(|| {
        GatewayError::Internal("executor stream ended before headers frame".to_string())
    })?;
    let StreamFramePayload::Headers {
        status_code,
        headers,
    } = first_frame.payload
    else {
        return Err(GatewayError::Internal(
            "executor stream must start with headers frame".to_string(),
        ));
    };

    if status_code >= 400 {
        let error_body = collect_error_body(&mut lines).await?;
        return Ok(Some(build_executor_error_response(
            trace_id,
            decision,
            plan_kind,
            status_code,
            headers,
            error_body,
        )?));
    }

    let trace_id_owned = trace_id.to_string();
    let body_stream = stream! {
        loop {
            let next_frame = match read_next_frame(&mut lines).await {
                Ok(frame) => frame,
                Err(err) => {
                    warn!(trace_id = %trace_id_owned, error = %format!("{err:?}"), "gateway failed to decode executor stream frame");
                    break;
                }
            };
            let Some(frame) = next_frame else {
                break;
            };
            match frame.payload {
                StreamFramePayload::Data { chunk_b64, text } => {
                    if let Some(chunk_b64) = chunk_b64 {
                        match base64::engine::general_purpose::STANDARD.decode(chunk_b64) {
                            Ok(decoded) => yield Ok::<Bytes, IoError>(Bytes::from(decoded)),
                            Err(err) => {
                                warn!(trace_id = %trace_id_owned, error = %err, "gateway failed to decode executor chunk");
                                break;
                            }
                        }
                    } else if let Some(text) = text {
                        yield Ok::<Bytes, IoError>(Bytes::from(text.into_bytes()));
                    }
                }
                StreamFramePayload::Telemetry { .. } => {}
                StreamFramePayload::Eof { .. } => break,
                StreamFramePayload::Error { error } => {
                    warn!(trace_id = %trace_id_owned, error = %error.message, "executor stream emitted error frame");
                    break;
                }
                StreamFramePayload::Headers { .. } => {}
            }
        }
    };

    Ok(Some(build_client_response_from_parts(
        status_code,
        &headers,
        Body::from_stream(body_stream),
        trace_id,
        Some(decision),
    )?))
}

async fn collect_error_body<R>(
    lines: &mut FramedRead<R, LinesCodec>,
) -> Result<Vec<u8>, GatewayError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut body = Vec::new();
    while let Some(frame) = read_next_frame(lines).await? {
        match frame.payload {
            StreamFramePayload::Data { chunk_b64, text } => {
                let chunk = if let Some(chunk_b64) = chunk_b64 {
                    base64::engine::general_purpose::STANDARD
                        .decode(chunk_b64)
                        .map_err(|err| GatewayError::Internal(err.to_string()))?
                } else {
                    text.unwrap_or_default().into_bytes()
                };
                body.extend_from_slice(&chunk);
                if body.len() >= MAX_ERROR_BODY_BYTES {
                    body.truncate(MAX_ERROR_BODY_BYTES);
                    break;
                }
            }
            StreamFramePayload::Telemetry { .. } => {}
            StreamFramePayload::Eof { .. } => break,
            StreamFramePayload::Error { error } => {
                warn!(error = %error.message, "executor stream emitted error frame while collecting error body");
                break;
            }
            StreamFramePayload::Headers { .. } => {}
        }
    }
    Ok(body)
}

async fn read_next_frame<R>(
    lines: &mut FramedRead<R, LinesCodec>,
) -> Result<Option<StreamFrame>, GatewayError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    while let Some(line) = lines.next().await {
        let line = line.map_err(|err| GatewayError::Internal(err.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        let frame: StreamFrame =
            serde_json::from_str(&line).map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(frame));
    }
    Ok(None)
}

fn build_executor_error_response(
    trace_id: &str,
    decision: &GatewayControlDecision,
    plan_kind: &str,
    status_code: u16,
    headers: BTreeMap<String, String>,
    error_body: Vec<u8>,
) -> Result<Response<Body>, GatewayError> {
    let content_type = headers
        .get("content-type")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if plan_kind == GEMINI_FILES_DOWNLOAD_PLAN_KIND && !content_type.starts_with("application/json")
    {
        let wrapped = serde_json::to_vec(&json!({
            "error": String::from_utf8_lossy(&error_body).to_string(),
        }))
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let wrapped_headers =
            BTreeMap::from([("content-type".to_string(), "application/json".to_string())]);
        return build_client_response_from_parts(
            status_code,
            &wrapped_headers,
            Body::from(wrapped),
            trace_id,
            Some(decision),
        );
    }

    if plan_kind == OPENAI_VIDEO_CONTENT_PLAN_KIND && !content_type.starts_with("application/json")
    {
        let wrapped = serde_json::to_vec(&json!({
            "error": {
                "type": "upstream_error",
                "message": "Video not available",
            }
        }))
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let wrapped_headers =
            BTreeMap::from([("content-type".to_string(), "application/json".to_string())]);
        return build_client_response_from_parts(
            status_code,
            &wrapped_headers,
            Body::from(wrapped),
            trace_id,
            Some(decision),
        );
    }

    build_client_response_from_parts(
        status_code,
        &headers,
        Body::from(error_body),
        trace_id,
        Some(decision),
    )
}
