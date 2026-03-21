use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::Response;

use crate::gateway::constants::*;
use crate::gateway::headers::should_skip_response_header;
use crate::gateway::{insert_header_if_missing, GatewayControlDecision, GatewayError};

pub(crate) fn build_client_response(
    upstream_response: reqwest::Response,
    trace_id: &str,
    control_decision: Option<&GatewayControlDecision>,
) -> Result<Response<Body>, GatewayError> {
    let status = upstream_response.status();
    let upstream_headers = upstream_response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let upstream_stream = upstream_response.bytes_stream();
    build_client_response_from_parts(
        status.as_u16(),
        &upstream_headers,
        Body::from_stream(upstream_stream),
        trace_id,
        control_decision,
    )
}

pub(crate) fn build_client_response_from_parts(
    status_code: u16,
    upstream_headers: &BTreeMap<String, String>,
    body: Body,
    trace_id: &str,
    control_decision: Option<&GatewayControlDecision>,
) -> Result<Response<Body>, GatewayError> {
    let mut response = Response::builder()
        .status(status_code)
        .body(body)
        .map_err(|err| GatewayError::Internal(err.to_string()))?;

    for (name, value) in upstream_headers {
        if should_skip_response_header(name.as_str()) {
            continue;
        }
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let header_value =
            HeaderValue::from_str(value).map_err(|err| GatewayError::Internal(err.to_string()))?;
        response.headers_mut().insert(header_name, header_value);
    }
    insert_header_if_missing(response.headers_mut(), TRACE_ID_HEADER, trace_id)?;
    insert_header_if_missing(response.headers_mut(), GATEWAY_HEADER, "rust-phase3b")?;
    if let Some(decision) = control_decision {
        insert_header_if_missing(
            response.headers_mut(),
            CONTROL_ROUTE_CLASS_HEADER,
            decision.route_class.as_deref().unwrap_or("passthrough"),
        )?;
        insert_header_if_missing(
            response.headers_mut(),
            CONTROL_EXECUTOR_HEADER,
            if decision.executor_candidate {
                "true"
            } else {
                "false"
            },
        )?;
        if let Some(route_family) = decision.route_family.as_deref() {
            insert_header_if_missing(
                response.headers_mut(),
                CONTROL_ROUTE_FAMILY_HEADER,
                route_family,
            )?;
        }
        if let Some(route_kind) = decision.route_kind.as_deref() {
            insert_header_if_missing(
                response.headers_mut(),
                CONTROL_ROUTE_KIND_HEADER,
                route_kind,
            )?;
        }
    }
    Ok(response)
}
