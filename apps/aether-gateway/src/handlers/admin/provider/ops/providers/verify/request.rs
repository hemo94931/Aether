use crate::handlers::admin::request::AdminAppState;
use crate::GatewayError;
use aether_contracts::{
    ExecutionPlan, ExecutionResult, ExecutionTimeouts, ProxySnapshot, RequestBody,
};
use aether_http::{apply_http_client_config, HttpClientConfig};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::read::{DeflateDecoder, GzDecoder};
use reqwest::redirect::Policy;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;

const ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS: u64 = 30_000;

pub(super) struct AdminProviderOpsTextResponse {
    pub(super) body: String,
}

pub(super) async fn admin_provider_ops_execute_get_json(
    state: &AdminAppState<'_>,
    request_id: &str,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    proxy_snapshot: Option<&ProxySnapshot>,
) -> Result<(http::StatusCode, Value), String> {
    if proxy_snapshot.is_none() {
        let response = match state
            .http_client()
            .get(url)
            .headers(headers.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) if err.is_timeout() => return Err("timeout".to_string()),
            Err(err) => return Err(err.to_string()),
        };
        let status = response.status();
        let content_encoding = response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes = response.bytes().await.map_err(|err| err.to_string())?;
        let decoded_bytes =
            admin_provider_ops_decode_response_bytes(bytes.as_ref(), content_encoding.as_deref())
                .unwrap_or_else(|| bytes.to_vec());
        let response_json = match serde_json::from_slice::<Value>(&decoded_bytes) {
            Ok(value) => value,
            Err(err) if status != http::StatusCode::OK => json!({}),
            Err(err) => return Err(format!("upstream response is not valid JSON: {err}")),
        };
        return Ok((status, response_json));
    }

    let result = admin_provider_ops_execute_request(
        state,
        request_id,
        reqwest::Method::GET,
        url,
        headers,
        None,
        proxy_snapshot,
    )
    .await?;
    Ok((
        admin_provider_ops_execution_status_code(&result),
        admin_provider_ops_execution_json_body(&result),
    ))
}

pub(in super::super) async fn admin_provider_ops_execute_proxy_json_request(
    state: &AdminAppState<'_>,
    request_id: &str,
    method: reqwest::Method,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    json_body: Option<Value>,
    proxy_snapshot: &ProxySnapshot,
) -> Result<(http::StatusCode, Value), String> {
    let result = admin_provider_ops_execute_request(
        state,
        request_id,
        method,
        url,
        headers,
        json_body,
        Some(proxy_snapshot),
    )
    .await?;
    Ok((
        admin_provider_ops_execution_status_code(&result),
        admin_provider_ops_execution_json_body(&result),
    ))
}

pub(super) async fn admin_provider_ops_execute_get_text(
    state: &AdminAppState<'_>,
    request_id: &str,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    proxy_snapshot: Option<&ProxySnapshot>,
) -> Result<AdminProviderOpsTextResponse, String> {
    let result = admin_provider_ops_execute_request(
        state,
        request_id,
        reqwest::Method::GET,
        url,
        headers,
        None,
        proxy_snapshot,
    )
    .await?;
    let body = result
        .body
        .as_ref()
        .and_then(|body| admin_provider_ops_execution_body_bytes(&result.headers, body))
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .or_else(|| {
            result
                .body
                .as_ref()
                .and_then(|body| body.json_body.as_ref())
                .and_then(|value| serde_json::to_string(value).ok())
        })
        .unwrap_or_default();
    Ok(AdminProviderOpsTextResponse { body })
}

pub(super) async fn admin_provider_ops_execute_get_text_no_redirect(
    url: &str,
    headers: &reqwest::header::HeaderMap,
    proxy_snapshot: Option<&ProxySnapshot>,
) -> Result<AdminProviderOpsTextResponse, String> {
    let mut builder = apply_http_client_config(
        reqwest::Client::builder().redirect(Policy::none()),
        &HttpClientConfig {
            connect_timeout_ms: Some(10_000),
            request_timeout_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            use_rustls_tls: true,
            http2_adaptive_window: true,
            ..HttpClientConfig::default()
        },
    );
    if let Some(proxy_url) = proxy_snapshot
        .and_then(|proxy| proxy.url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let proxy = reqwest::Proxy::all(proxy_url).map_err(|err| format!("连接失败: {err}"))?;
        builder = builder.proxy(proxy);
    }
    let client = builder.build().map_err(|err| format!("验证失败: {err}"))?;
    let response = match client.get(url).headers(headers.clone()).send().await {
        Ok(response) => response,
        Err(err) if err.is_timeout() => return Err("连接超时".to_string()),
        Err(err) if err.is_connect() => return Err(format!("连接失败: {err}")),
        Err(err) => return Err(format!("验证失败: {err}")),
    };
    let content_encoding = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .bytes()
        .await
        .map_err(|err| format!("验证失败: {err}"))
        .map(|bytes| {
            admin_provider_ops_decode_response_bytes(bytes.as_ref(), content_encoding.as_deref())
                .unwrap_or_else(|| bytes.to_vec())
        })
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())?;
    Ok(AdminProviderOpsTextResponse { body })
}

async fn admin_provider_ops_execute_request(
    state: &AdminAppState<'_>,
    request_id: &str,
    method: reqwest::Method,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    json_body: Option<Value>,
    proxy_snapshot: Option<&ProxySnapshot>,
) -> Result<ExecutionResult, String> {
    let has_json_body = json_body.is_some();
    let body = json_body
        .map(RequestBody::from_json)
        .unwrap_or(RequestBody {
            json_body: None,
            body_bytes_b64: None,
            body_ref: None,
        });
    let plan = ExecutionPlan {
        request_id: request_id.to_string(),
        candidate_id: None,
        provider_name: Some("provider_ops".to_string()),
        provider_id: String::new(),
        endpoint_id: String::new(),
        key_id: String::new(),
        method: method.as_str().to_string(),
        url: url.to_string(),
        headers: admin_provider_ops_execution_headers(headers),
        content_type: has_json_body.then(|| "application/json".to_string()),
        content_encoding: None,
        body,
        stream: false,
        client_api_format: "provider_ops:verify".to_string(),
        provider_api_format: "provider_ops:verify".to_string(),
        model_name: Some("verify-auth".to_string()),
        proxy: proxy_snapshot.cloned(),
        tls_profile: None,
        timeouts: Some(ExecutionTimeouts {
            connect_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            read_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            write_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            pool_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            total_ms: Some(ADMIN_PROVIDER_OPS_VERIFY_TIMEOUT_MS),
            ..ExecutionTimeouts::default()
        }),
    };
    state
        .execute_execution_runtime_sync_plan(None, &plan)
        .await
        .map_err(admin_provider_ops_gateway_error_message)
}

fn admin_provider_ops_execution_headers(
    headers: &reqwest::header::HeaderMap,
) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|text| (name.as_str().to_string(), text.to_string()))
        })
        .collect()
}

fn admin_provider_ops_execution_status_code(result: &ExecutionResult) -> http::StatusCode {
    http::StatusCode::from_u16(result.status_code).unwrap_or(http::StatusCode::BAD_GATEWAY)
}

fn admin_provider_ops_execution_json_body(result: &ExecutionResult) -> Value {
    result
        .body
        .as_ref()
        .and_then(|body| body.json_body.clone())
        .or_else(|| {
            result
                .body
                .as_ref()
                .and_then(|body| admin_provider_ops_execution_body_bytes(&result.headers, body))
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        })
        .unwrap_or_else(|| json!({}))
}

fn admin_provider_ops_execution_body_bytes(
    headers: &BTreeMap<String, String>,
    body: &aether_contracts::ResponseBody,
) -> Option<Vec<u8>> {
    let bytes = body
        .body_bytes_b64
        .as_deref()
        .and_then(|value| STANDARD.decode(value).ok())?;
    admin_provider_ops_decode_response_bytes(
        &bytes,
        headers.get("content-encoding").map(String::as_str),
    )
    .or(Some(bytes))
}

fn admin_provider_ops_decode_response_bytes(
    bytes: &[u8],
    content_encoding: Option<&str>,
) -> Option<Vec<u8>> {
    let encoding = content_encoding
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    match encoding.as_deref() {
        Some("gzip") => {
            let mut decoder = GzDecoder::new(bytes);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).ok()?;
            Some(out)
        }
        Some("deflate") => {
            let mut decoder = DeflateDecoder::new(bytes);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).ok()?;
            Some(out)
        }
        _ => None,
    }
}

fn admin_provider_ops_gateway_error_message(error: GatewayError) -> String {
    match error {
        GatewayError::UpstreamUnavailable { message, .. }
        | GatewayError::ControlUnavailable { message, .. }
        | GatewayError::Internal(message) => message,
    }
}

pub(super) fn admin_provider_ops_verify_execution_error_message(error: &str) -> String {
    let normalized = error.trim();
    let lower = normalized.to_ascii_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        return "连接超时".to_string();
    }
    if lower.contains("connect")
        || lower.contains("connection")
        || lower.contains("dns")
        || lower.contains("proxy")
        || lower.contains("relay")
    {
        return format!("连接失败: {normalized}");
    }
    format!("验证失败: {normalized}")
}
