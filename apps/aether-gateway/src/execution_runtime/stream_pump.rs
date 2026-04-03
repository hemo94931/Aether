use std::io::Error as IoError;

use aether_contracts::{
    ExecutionError, ExecutionErrorKind, ExecutionPhase, ExecutionTelemetry, StreamFrame,
    StreamFramePayload, StreamFrameType,
};
use async_stream::stream;
use axum::body::Bytes;
use base64::Engine as _;
use futures_util::{Stream, StreamExt};

use crate::gateway::execution_runtime::ndjson::encode_stream_frame_ndjson;
use crate::gateway::execution_runtime::DirectUpstreamStreamExecution;

pub(crate) fn build_direct_execution_frame_stream(
    execution: DirectUpstreamStreamExecution,
) -> impl Stream<Item = Result<Bytes, IoError>> + Send + 'static {
    stream! {
        let DirectUpstreamStreamExecution {
            request_id: _,
            candidate_id: _,
            status_code,
            headers,
            response,
            started_at,
        } = execution;

        let headers_frame = StreamFrame {
            frame_type: StreamFrameType::Headers,
            payload: StreamFramePayload::Headers {
                status_code,
                headers,
            },
        };
        match encode_stream_frame_ndjson(&headers_frame) {
            Ok(frame) => yield Ok(frame),
            Err(err) => {
                yield Err(err);
                return;
            }
        }

        let mut upstream_bytes = 0u64;
        let mut bytes_stream = response.bytes_stream();
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
                    match encode_stream_frame_ndjson(&frame) {
                        Ok(frame) => yield Ok(frame),
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    }
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
                    match encode_stream_frame_ndjson(&frame) {
                        Ok(frame) => yield Ok(frame),
                        Err(encode_err) => {
                            yield Err(encode_err);
                            return;
                        }
                    }
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
        match encode_stream_frame_ndjson(&telemetry_frame) {
            Ok(frame) => yield Ok(frame),
            Err(err) => {
                yield Err(err);
                return;
            }
        }
        match encode_stream_frame_ndjson(&StreamFrame::eof()) {
            Ok(frame) => yield Ok(frame),
            Err(err) => yield Err(err),
        }
    }
}
