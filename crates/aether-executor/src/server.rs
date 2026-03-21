use std::convert::Infallible;
use std::path::Path;

use aether_contracts::{
    ExecutionError, ExecutionErrorKind, ExecutionPhase, ExecutionPlan, ExecutionResult,
    ExecutionTelemetry, StreamFrame, StreamFramePayload, StreamFrameType,
};
use async_stream::stream;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::json;

use crate::{encode_frame, ExecutorServiceError, SyncExecutor};

#[derive(Debug, Clone, Default)]
pub struct AppState {
    executor: SyncExecutor,
}

pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/execute/sync", post(execute_sync))
        .route("/v1/execute/stream", post(execute_stream))
        .with_state(AppState {
            executor: SyncExecutor::new(),
        })
}

pub async fn serve_tcp(bind: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, build_router()).await?;
    Ok(())
}

pub async fn serve_unix(socket_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = tokio::net::UnixListener::bind(socket_path)?;
    axum::serve(listener, build_router()).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn execute_sync(
    State(state): State<AppState>,
    Json(plan): Json<ExecutionPlan>,
) -> Result<Json<ExecutionResult>, AppError> {
    state
        .executor
        .execute_sync(plan)
        .await
        .map(Json)
        .map_err(AppError)
}

async fn execute_stream(
    State(state): State<AppState>,
    Json(plan): Json<ExecutionPlan>,
) -> Result<Response, AppError> {
    let execution = state
        .executor
        .execute_stream(plan)
        .await
        .map_err(AppError)?;

    let status_code = execution.status_code;
    let response_headers = execution.headers.clone();
    let started_at = execution.started_at;
    let upstream_response = execution.response;

    let body_stream = stream! {
        let headers_frame = StreamFrame {
            frame_type: StreamFrameType::Headers,
            payload: StreamFramePayload::Headers {
                status_code,
                headers: response_headers,
            },
        };
        yield Ok::<Bytes, Infallible>(encode_frame(&headers_frame).expect("headers frame should encode"));

        let mut upstream_bytes = 0u64;
        let mut bytes_stream = upstream_response.bytes_stream();
        while let Some(item) = bytes_stream.next().await {
            match item {
                Ok(chunk) => {
                    upstream_bytes += chunk.len() as u64;
                    let frame = StreamFrame {
                        frame_type: StreamFrameType::Data,
                        payload: StreamFramePayload::Data {
                            chunk_b64: Some(base64::engine::general_purpose::STANDARD.encode(&chunk)),
                            text: None,
                        },
                    };
                    yield Ok::<Bytes, Infallible>(encode_frame(&frame).expect("data frame should encode"));
                }
                Err(err) => {
                    let frame = StreamFrame {
                        frame_type: StreamFrameType::Error,
                        payload: StreamFramePayload::Error {
                            error: ExecutionError {
                                kind: ExecutionErrorKind::Internal,
                                phase: ExecutionPhase::StreamRead,
                                message: err.to_string(),
                                upstream_status: Some(status_code),
                                retryable: false,
                                failover_recommended: false,
                            },
                        },
                    };
                    yield Ok::<Bytes, Infallible>(encode_frame(&frame).expect("error frame should encode"));
                    break;
                }
            }
        }

        let telemetry_frame = StreamFrame {
            frame_type: StreamFrameType::Telemetry,
            payload: StreamFramePayload::Telemetry {
                telemetry: ExecutionTelemetry {
                    ttfb_ms: None,
                    elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
                    upstream_bytes: Some(upstream_bytes),
                },
            },
        };
        yield Ok::<Bytes, Infallible>(encode_frame(&telemetry_frame).expect("telemetry frame should encode"));
        yield Ok::<Bytes, Infallible>(encode_frame(&StreamFrame::eof()).expect("eof frame should encode"));
    };

    let mut response = Response::new(Body::from_stream(body_stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-ndjson"),
    );
    Ok(response)
}

#[derive(Debug)]
struct AppError(ExecutorServiceError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = match self.0 {
            ExecutorServiceError::StreamUnsupported
            | ExecutorServiceError::RequestBodyRequired
            | ExecutorServiceError::BodyDecode(_)
            | ExecutorServiceError::UnsupportedContentEncoding(_)
            | ExecutorServiceError::ProxyUnsupported
            | ExecutorServiceError::TlsProfileUnsupported
            | ExecutorServiceError::DelegateUnsupported
            | ExecutorServiceError::InvalidMethod(_)
            | ExecutorServiceError::InvalidHeaderName(_)
            | ExecutorServiceError::InvalidHeaderValue(_)
            | ExecutorServiceError::InvalidProxy(_)
            | ExecutorServiceError::BodyEncode(_) => StatusCode::BAD_REQUEST,
            ExecutorServiceError::ClientBuild(_)
            | ExecutorServiceError::UpstreamRequest(_)
            | ExecutorServiceError::RelayError(_)
            | ExecutorServiceError::InvalidJson(_) => StatusCode::BAD_GATEWAY,
        };

        (
            status_code,
            Json(json!({
                "error": self.0.to_string(),
            })),
        )
            .into_response()
    }
}
