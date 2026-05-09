use axum::body::{Body, Bytes};
use axum::extract::ws::{
    CloseFrame as AxumCloseFrame, Message as AxumWsMessage, WebSocket, WebSocketUpgrade,
};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{self, HeaderMap, HeaderName, HeaderValue, Response, Uri};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    protocol::{
        frame::{coding::CloseCode as TungsteniteCloseCode, CloseFrame as TungsteniteCloseFrame},
        Message as TungsteniteMessage,
    },
};

use crate::control::GatewayPublicRequestContext;
use crate::handlers::proxy::proxy_request;
use crate::handlers::shared::{module_available_from_env, query_param_value, system_config_bool};
use crate::headers::header_value_str;
use crate::system_features::{AMP_PROXY_CONFIG_KEY, AMP_PROXY_ENABLED_CONFIG_KEY};
use crate::{AppState, GatewayError, LocalExecutionRuntimeMissDiagnostic};

#[derive(Debug, Clone, Default, Deserialize)]
struct AmpProxyConfig {
    #[serde(default)]
    upstream_url: String,
    #[serde(default)]
    upstream_api_key: String,
    #[serde(default)]
    upstream_api_keys: Vec<AmpProxyUpstreamApiKeyRoute>,
    #[serde(default)]
    fallback_to_upstream_on_model_miss: bool,
    #[serde(default)]
    force_legacy_worker_runtime: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AmpProxyUpstreamApiKeyRoute {
    #[serde(default)]
    api_keys: Vec<String>,
    #[serde(default)]
    upstream_api_key: String,
}

#[derive(Debug, Clone)]
struct AmpWebsocketProxyContext {
    request_path_and_query: String,
    upstream_url: String,
    upstream_key: Option<String>,
    headers: HeaderMap,
}

#[derive(Debug, Clone)]
struct AmpProviderProxyOriginalPathAndQuery(String);

pub(crate) async fn amp_provider_proxy_request(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<std::net::SocketAddr>,
    mut request: Request,
) -> Result<Response<Body>, GatewayError> {
    if !amp_proxy_module_enabled(&state).await? {
        return Ok(amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_proxy_disabled",
            "AMP 代理模块未启用",
        ));
    }

    let Some(normalized_path) = normalize_amp_provider_alias_path(request.uri().path()) else {
        return Ok(amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_provider_route_not_supported",
            "AMP Provider 路径暂不支持",
        ));
    };
    let original_path_and_query = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_string();
    request
        .extensions_mut()
        .insert(AmpProviderProxyOriginalPathAndQuery(
            original_path_and_query,
        ));
    let Some(request) = rewrite_request_path(request, &normalized_path) else {
        return Ok(amp_proxy_json_error(
            http::StatusCode::BAD_REQUEST,
            "invalid_amp_provider_route",
            "AMP Provider 路径无效",
        ));
    };

    proxy_request(State(state), ConnectInfo(remote_addr), request).await
}

pub(crate) async fn maybe_build_local_amp_management_proxy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &HeaderMap,
    request_body: Option<&Bytes>,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("amp_proxy") {
        return None;
    }
    if decision.local_auth_rejection.is_some() || decision.auth_context.is_none() {
        return Some(amp_proxy_json_error(
            http::StatusCode::UNAUTHORIZED,
            "amp_proxy_auth_required",
            "AMP 代理请求需要有效的 Aether API Key",
        ));
    }

    let enabled = amp_proxy_module_enabled(state).await.ok()?;
    if !enabled {
        return Some(amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_proxy_disabled",
            "AMP 代理模块未启用",
        ));
    }

    let config = load_amp_proxy_config(state).await.ok()?;
    let upstream_url = config.upstream_url.trim();
    if upstream_url.is_empty() {
        return Some(amp_proxy_json_error(
            http::StatusCode::SERVICE_UNAVAILABLE,
            "amp_proxy_upstream_not_configured",
            "AMP 上游地址未配置",
        ));
    }
    if config.force_legacy_worker_runtime
        && is_amp_thread_actor_request_path(&request_context.request_path)
    {
        return Some(amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_thread_actors_disabled",
            "AMP thread-actors 已被代理配置禁用",
        ));
    }

    Some(
        forward_amp_management_request(state, request_context, headers, request_body, &config)
            .await
            .unwrap_or_else(|err| {
                tracing::warn!(
                    error = ?err,
                    path = %request_context.request_path_and_query(),
                    "amp proxy upstream request failed"
                );
                amp_proxy_json_error(
                    http::StatusCode::BAD_GATEWAY,
                    "amp_proxy_upstream_error",
                    "AMP 上游请求失败",
                )
            }),
    )
}

pub(crate) fn is_amp_management_websocket_upgrade(
    request_context: &GatewayPublicRequestContext,
    headers: &HeaderMap,
) -> bool {
    let Some(decision) = request_context.control_decision.as_ref() else {
        return false;
    };
    if decision.route_class.as_deref() != Some("public_support")
        || decision.route_family.as_deref() != Some("amp_proxy")
        || decision.route_kind.as_deref() != Some("management")
    {
        return false;
    }

    header_contains_token(headers, http::header::CONNECTION, "upgrade")
        && header_equals(headers, http::header::UPGRADE, "websocket")
}

pub(crate) async fn build_local_amp_management_websocket_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &HeaderMap,
    ws: WebSocketUpgrade,
) -> Response<Body> {
    let Some(decision) = request_context.control_decision.as_ref() else {
        return amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_proxy_route_not_found",
            "AMP 代理路径未匹配",
        );
    };
    if decision.route_family.as_deref() != Some("amp_proxy") {
        return amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_proxy_route_not_found",
            "AMP 代理路径未匹配",
        );
    }
    if decision.local_auth_rejection.is_some() || decision.auth_context.is_none() {
        return amp_proxy_json_error(
            http::StatusCode::UNAUTHORIZED,
            "amp_proxy_auth_required",
            "AMP 代理请求需要有效的 Aether API Key",
        );
    }

    let enabled = match amp_proxy_module_enabled(state).await {
        Ok(enabled) => enabled,
        Err(err) => {
            tracing::warn!(error = ?err, "failed to read amp proxy module flag");
            return amp_proxy_json_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "amp_proxy_config_error",
                "AMP 代理配置读取失败",
            );
        }
    };
    if !enabled {
        return amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_proxy_disabled",
            "AMP 代理模块未启用",
        );
    }

    let config = match load_amp_proxy_config(state).await {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(error = ?err, "failed to read amp proxy config");
            return amp_proxy_json_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "amp_proxy_config_error",
                "AMP 代理配置读取失败",
            );
        }
    };
    let upstream_base_url = config.upstream_url.trim();
    if upstream_base_url.is_empty() {
        return amp_proxy_json_error(
            http::StatusCode::SERVICE_UNAVAILABLE,
            "amp_proxy_upstream_not_configured",
            "AMP 上游地址未配置",
        );
    }
    if config.force_legacy_worker_runtime
        && is_amp_thread_actor_request_path(&request_context.request_path)
    {
        return amp_proxy_json_error(
            http::StatusCode::NOT_FOUND,
            "amp_thread_actors_disabled",
            "AMP thread-actors 已被代理配置禁用",
        );
    }

    let client_key =
        extract_amp_client_api_key(headers, request_context.request_query_string.as_deref());
    let upstream_key = select_amp_upstream_api_key(&config, client_key.as_deref());
    let upstream_http_url = match build_amp_upstream_url(
        upstream_base_url,
        &request_context.request_path_and_query(),
        client_key.as_deref(),
    ) {
        Ok(url) => url,
        Err(detail) => {
            tracing::warn!(
                detail,
                path = %request_context.request_path_and_query(),
                "failed to build amp websocket upstream URL"
            );
            return amp_proxy_json_error(
                http::StatusCode::BAD_GATEWAY,
                "amp_proxy_upstream_url_invalid",
                "AMP 上游地址无效",
            );
        }
    };
    let upstream_url = match build_amp_websocket_upstream_url(&upstream_http_url) {
        Ok(url) => url,
        Err(detail) => {
            tracing::warn!(
                detail = detail.as_str(),
                path = %request_context.request_path_and_query(),
                "failed to build amp websocket upstream URL"
            );
            return amp_proxy_json_error(
                http::StatusCode::BAD_GATEWAY,
                "amp_proxy_upstream_url_invalid",
                "AMP 上游地址无效",
            );
        }
    };

    let protocols = websocket_protocols(headers);
    let ws = ws.max_frame_size(64 * 1024 * 1024);
    let ws = if protocols.is_empty() {
        ws
    } else {
        ws.protocols(protocols)
    };
    let context = AmpWebsocketProxyContext {
        request_path_and_query: request_context.request_path_and_query(),
        upstream_url,
        upstream_key,
        headers: headers.clone(),
    };

    ws.on_failed_upgrade(|err| {
        tracing::warn!(error = %err, "amp websocket downstream upgrade failed");
    })
    .on_upgrade(move |socket| relay_amp_websocket(socket, context))
}

pub(crate) async fn maybe_build_amp_provider_model_fallback_response(
    state: &AppState,
    parts: &http::request::Parts,
    diagnostic: Option<&LocalExecutionRuntimeMissDiagnostic>,
    request_body: &Bytes,
) -> Option<Response<Body>> {
    let config = load_amp_proxy_config(state).await.ok()?;
    let path_and_query = amp_provider_model_miss_fallback_path(&config, parts, diagnostic)?;
    let upstream_url = config.upstream_url.trim();
    if upstream_url.is_empty() {
        return Some(amp_proxy_json_error(
            http::StatusCode::SERVICE_UNAVAILABLE,
            "amp_proxy_upstream_not_configured",
            "AMP 上游地址未配置",
        ));
    }

    tracing::info!(
        path = %path_and_query,
        requested_model = diagnostic
            .and_then(|value| value.requested_model.as_deref())
            .unwrap_or_default(),
        "falling back amp provider request to upstream after local model candidate miss"
    );

    Some(
        forward_amp_upstream_request(
            state,
            &parts.method,
            path_and_query,
            &parts.headers,
            Some(request_body),
            &config,
        )
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(
                error = ?err,
                path = %path_and_query,
                "amp provider model fallback upstream request failed"
            );
            amp_proxy_json_error(
                http::StatusCode::BAD_GATEWAY,
                "amp_proxy_upstream_error",
                "AMP 上游请求失败",
            )
        }),
    )
}

fn amp_provider_model_miss_fallback_path<'a>(
    config: &AmpProxyConfig,
    parts: &'a http::request::Parts,
    diagnostic: Option<&LocalExecutionRuntimeMissDiagnostic>,
) -> Option<&'a str> {
    if !config.fallback_to_upstream_on_model_miss {
        return None;
    }

    let diagnostic = diagnostic?;
    if diagnostic.reason != "candidate_list_empty" {
        return None;
    }
    if diagnostic
        .requested_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return None;
    }

    parts
        .extensions
        .get::<AmpProviderProxyOriginalPathAndQuery>()
        .map(|value| value.0.as_str())
        .filter(|value| !value.trim().is_empty())
}

async fn amp_proxy_module_enabled(state: &AppState) -> Result<bool, GatewayError> {
    if !module_available_from_env("AMP_PROXY_AVAILABLE", true) {
        return Ok(false);
    }
    let value = state
        .read_system_config_json_value(AMP_PROXY_ENABLED_CONFIG_KEY)
        .await?;
    Ok(system_config_bool(value.as_ref(), false))
}

async fn load_amp_proxy_config(state: &AppState) -> Result<AmpProxyConfig, GatewayError> {
    let value = state
        .read_system_config_json_value(AMP_PROXY_CONFIG_KEY)
        .await?;
    Ok(value
        .or_else(|| aether_admin::system::admin_system_config_default_value(AMP_PROXY_CONFIG_KEY))
        .and_then(|value| serde_json::from_value::<AmpProxyConfig>(value).ok())
        .unwrap_or_default())
}

async fn forward_amp_management_request(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &HeaderMap,
    request_body: Option<&Bytes>,
    config: &AmpProxyConfig,
) -> Result<Response<Body>, GatewayError> {
    let path_and_query = request_context.request_path_and_query();
    forward_amp_upstream_request(
        state,
        &request_context.request_method,
        &path_and_query,
        headers,
        request_body,
        config,
    )
    .await
}

async fn forward_amp_upstream_request(
    state: &AppState,
    method: &http::Method,
    path_and_query: &str,
    headers: &HeaderMap,
    request_body: Option<&Bytes>,
    config: &AmpProxyConfig,
) -> Result<Response<Body>, GatewayError> {
    let query = path_and_query.split_once('?').map(|(_, query)| query);
    let client_key = extract_amp_client_api_key(headers, query);
    let upstream_key = select_amp_upstream_api_key(config, client_key.as_deref());
    let upstream_url = build_amp_upstream_url(
        config.upstream_url.trim(),
        path_and_query,
        client_key.as_deref(),
    )
    .map_err(|detail| GatewayError::Internal(detail.to_string()))?;
    let mut request_builder = state.client.request(method.clone(), upstream_url);

    for (name, value) in headers {
        if should_forward_amp_header(name) {
            request_builder = request_builder.header(name, value);
        }
    }
    if let Some(upstream_key) = upstream_key.as_deref().filter(|value| !value.is_empty()) {
        request_builder = request_builder
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {upstream_key}"),
            )
            .header("x-api-key", upstream_key);
    }
    if let Some(body) = request_body {
        request_builder = request_builder.body(body.clone());
    }

    let upstream_response = request_builder
        .send()
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    if config.force_legacy_worker_runtime && is_amp_get_user_info_request(path_and_query) {
        return build_amp_get_user_info_legacy_response(upstream_response).await;
    }
    Ok(build_amp_upstream_response(upstream_response))
}

fn build_amp_upstream_response(upstream_response: reqwest::Response) -> Response<Body> {
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if should_forward_amp_response_header(name) {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from_stream(upstream_response.bytes_stream()))
        .unwrap_or_else(|_| {
            amp_proxy_json_error(
                http::StatusCode::BAD_GATEWAY,
                "amp_proxy_response_build_failed",
                "AMP 上游响应构建失败",
            )
        })
}

async fn build_amp_get_user_info_legacy_response(
    upstream_response: reqwest::Response,
) -> Result<Response<Body>, GatewayError> {
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let body = upstream_response
        .bytes()
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    if !status.is_success() {
        return Ok(build_amp_upstream_bytes_response(status, &headers, body));
    }

    let Ok(mut payload) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return Ok(build_amp_upstream_bytes_response(status, &headers, body));
    };
    disable_amp_thread_actor_features(&mut payload);
    let body =
        serde_json::to_vec(&payload).map_err(|err| GatewayError::Internal(err.to_string()))?;
    Ok(build_amp_upstream_bytes_response(
        status,
        &headers,
        Bytes::from(body),
    ))
}

fn build_amp_upstream_bytes_response(
    status: http::StatusCode,
    headers: &HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if should_forward_amp_rewritten_response_header(name) {
            builder = builder.header(name, value);
        }
    }
    builder.body(Body::from(body)).unwrap_or_else(|_| {
        amp_proxy_json_error(
            http::StatusCode::BAD_GATEWAY,
            "amp_proxy_response_build_failed",
            "AMP 上游响应构建失败",
        )
    })
}

fn disable_amp_thread_actor_features(payload: &mut serde_json::Value) {
    let Some(features) = payload
        .pointer_mut("/result/features")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };

    let mut has_thread_actors_tui = false;
    let mut has_thread_actors_traces = false;
    for feature in features.iter_mut() {
        let Some(name) = feature
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if name == "thread-actors-tui" || name == "thread-actors-traces" {
            if let Some(object) = feature.as_object_mut() {
                object.insert("enabled".to_string(), serde_json::Value::Bool(false));
            }
        }
        has_thread_actors_tui |= name == "thread-actors-tui";
        has_thread_actors_traces |= name == "thread-actors-traces";
    }
    if !has_thread_actors_tui {
        features.push(json!({ "name": "thread-actors-tui", "enabled": false }));
    }
    if !has_thread_actors_traces {
        features.push(json!({ "name": "thread-actors-traces", "enabled": false }));
    }
}

fn is_amp_get_user_info_request(path_and_query: &str) -> bool {
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, query))
        .unwrap_or((path_and_query, ""));
    path.trim_end_matches('/') == "/api/internal"
        && query
            .split('&')
            .any(|part| part.split_once('=').map(|(key, _)| key).unwrap_or(part) == "getUserInfo")
}

fn is_amp_thread_actor_request_path(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    path == "/api/thread-actors" || path.starts_with("/api/thread-actors/")
}

fn should_forward_amp_header(name: &HeaderName) -> bool {
    let normalized = name.as_str().to_ascii_lowercase();
    !matches!(
        normalized.as_str(),
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "authorization"
            | "x-api-key"
            | "x-goog-api-key"
            | "api-key"
    )
}

fn should_forward_amp_response_header(name: &HeaderName) -> bool {
    let normalized = name.as_str().to_ascii_lowercase();
    !matches!(
        normalized.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn should_forward_amp_rewritten_response_header(name: &HeaderName) -> bool {
    should_forward_amp_response_header(name)
        && !matches!(
            name.as_str().to_ascii_lowercase().as_str(),
            "content-encoding" | "content-length"
        )
}

fn should_forward_amp_websocket_header(name: &HeaderName) -> bool {
    let normalized = name.as_str().to_ascii_lowercase();
    should_forward_amp_header(name)
        && !matches!(
            normalized.as_str(),
            "sec-websocket-accept"
                | "sec-websocket-extensions"
                | "sec-websocket-key"
                | "sec-websocket-version"
        )
}

fn extract_amp_client_api_key(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(value) = header_value_str(headers, http::header::AUTHORIZATION.as_str()) {
        if let Some(token) = value.trim().strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    for header in ["x-api-key", "x-goog-api-key", "api-key"] {
        if let Some(value) = header_value_str(headers, header) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    query_param_value(query, "key").or_else(|| query_param_value(query, "auth_token"))
}

fn select_amp_upstream_api_key(
    config: &AmpProxyConfig,
    client_key: Option<&str>,
) -> Option<String> {
    let client_key = client_key.map(str::trim).filter(|value| !value.is_empty());
    if let Some(client_key) = client_key {
        for route in &config.upstream_api_keys {
            let upstream_key = route.upstream_api_key.trim();
            if upstream_key.is_empty() {
                continue;
            }
            if route
                .api_keys
                .iter()
                .any(|candidate| candidate.trim() == client_key)
            {
                return Some(upstream_key.to_string());
            }
        }
    }
    let fallback = config.upstream_api_key.trim();
    (!fallback.is_empty()).then(|| fallback.to_string())
}

fn build_amp_upstream_url(
    upstream_url: &str,
    path_and_query: &str,
    client_key: Option<&str>,
) -> Result<String, &'static str> {
    let mut url = reqwest::Url::parse(upstream_url).map_err(|_| "invalid upstream URL")?;
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let base_path = url.path().trim_end_matches('/');
    let target_path = if base_path.is_empty() || base_path == "/" {
        normalized_path
    } else {
        format!("{base_path}{normalized_path}")
    };
    url.set_path(&target_path);
    url.set_query(query);
    strip_matching_query_credentials(&mut url, client_key);
    Ok(url.to_string())
}

fn strip_matching_query_credentials(url: &mut reqwest::Url, client_key: Option<&str>) {
    let Some(client_key) = client_key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let pairs = url
        .query_pairs()
        .filter(|(key, value)| {
            !matches!(key.as_ref(), "key" | "auth_token") || value.as_ref() != client_key
        })
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    url.set_query(None);
    if pairs.is_empty() {
        return;
    }
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
    }
}

fn build_amp_websocket_upstream_url(upstream_url: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(upstream_url).map_err(|err| format!("invalid URL: {err}"))?;
    let target_scheme = match url.scheme() {
        "http" => Some("ws"),
        "https" => Some("wss"),
        "ws" | "wss" => None,
        _ => return Err("unsupported websocket upstream URL scheme".to_string()),
    };
    if let Some(target_scheme) = target_scheme {
        url.set_scheme(target_scheme)
            .map_err(|_| "unsupported websocket upstream URL scheme".to_string())?;
    }
    Ok(url.to_string())
}

fn header_contains_token(headers: &HeaderMap, name: HeaderName, token: &str) -> bool {
    headers.get_all(name).iter().any(|value| {
        value.to_str().ok().is_some_and(|raw| {
            raw.split(',')
                .any(|candidate| candidate.trim().eq_ignore_ascii_case(token))
        })
    })
}

fn header_equals(headers: &HeaderMap, name: HeaderName, expected: &str) -> bool {
    headers.get_all(name).iter().any(|value| {
        value
            .to_str()
            .ok()
            .is_some_and(|raw| raw.trim().eq_ignore_ascii_case(expected))
    })
}

fn websocket_protocols(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(http::header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn build_amp_websocket_upstream_request(
    context: &AmpWebsocketProxyContext,
) -> Result<tokio_tungstenite::tungstenite::handshake::client::Request, String> {
    let mut request = context
        .upstream_url
        .as_str()
        .into_client_request()
        .map_err(|err| err.to_string())?;
    {
        let headers = request.headers_mut();
        for (name, value) in context.headers.iter() {
            if should_forward_amp_websocket_header(name) {
                headers.append(name.clone(), value.clone());
            }
        }
        if let Some(upstream_key) = context
            .upstream_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let bearer = format!("Bearer {upstream_key}");
            headers.insert(
                http::header::AUTHORIZATION,
                HeaderValue::from_str(&bearer).map_err(|err| err.to_string())?,
            );
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(upstream_key).map_err(|err| err.to_string())?,
            );
        }
    }
    Ok(request)
}

async fn relay_amp_websocket(mut downstream: WebSocket, context: AmpWebsocketProxyContext) {
    let upstream_request = match build_amp_websocket_upstream_request(&context) {
        Ok(request) => request,
        Err(err) => {
            tracing::warn!(
                error = err.as_str(),
                path = %context.request_path_and_query,
                "failed to build amp websocket upstream request"
            );
            close_downstream_with_upstream_error(&mut downstream).await;
            return;
        }
    };

    tracing::debug!(
        path = %context.request_path_and_query,
        "connecting amp websocket upstream"
    );
    let (upstream, _) = match tokio_tungstenite::connect_async(upstream_request).await {
        Ok(upstream) => upstream,
        Err(err) => {
            tracing::warn!(
                error = %err,
                path = %context.request_path_and_query,
                "amp websocket upstream connection failed"
            );
            close_downstream_with_upstream_error(&mut downstream).await;
            return;
        }
    };

    let (mut downstream_tx, mut downstream_rx) = downstream.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let downstream_to_upstream = async {
        while let Some(message) = downstream_rx.next().await {
            let message = message.map_err(|err| err.to_string())?;
            let is_close = matches!(message, AxumWsMessage::Close(_));
            if let Some(message) = axum_to_tungstenite_message(message) {
                upstream_tx
                    .send(message)
                    .await
                    .map_err(|err| err.to_string())?;
            }
            if is_close {
                break;
            }
        }
        Ok::<(), String>(())
    };

    let upstream_to_downstream = async {
        while let Some(message) = upstream_rx.next().await {
            let message = message.map_err(|err| err.to_string())?;
            let is_close = matches!(message, TungsteniteMessage::Close(_));
            if let Some(message) = tungstenite_to_axum_message(message) {
                downstream_tx
                    .send(message)
                    .await
                    .map_err(|err| err.to_string())?;
            }
            if is_close {
                break;
            }
        }
        Ok::<(), String>(())
    };

    tokio::select! {
        result = downstream_to_upstream => {
            if let Err(err) = result {
                tracing::debug!(
                    error = err.as_str(),
                    path = %context.request_path_and_query,
                    "amp websocket downstream-to-upstream relay ended"
                );
            }
        }
        result = upstream_to_downstream => {
            if let Err(err) = result {
                tracing::debug!(
                    error = err.as_str(),
                    path = %context.request_path_and_query,
                    "amp websocket upstream-to-downstream relay ended"
                );
            }
        }
    }
}

async fn close_downstream_with_upstream_error(downstream: &mut WebSocket) {
    let _ = downstream
        .send(AxumWsMessage::Close(Some(AxumCloseFrame {
            code: 1011,
            reason: "amp upstream websocket unavailable".into(),
        })))
        .await;
}

fn axum_to_tungstenite_message(message: AxumWsMessage) -> Option<TungsteniteMessage> {
    match message {
        AxumWsMessage::Text(text) => Some(TungsteniteMessage::Text(text.to_string().into())),
        AxumWsMessage::Binary(bytes) => Some(TungsteniteMessage::Binary(bytes)),
        AxumWsMessage::Ping(bytes) => Some(TungsteniteMessage::Ping(bytes)),
        AxumWsMessage::Pong(bytes) => Some(TungsteniteMessage::Pong(bytes)),
        AxumWsMessage::Close(frame) => Some(TungsteniteMessage::Close(frame.map(|frame| {
            TungsteniteCloseFrame {
                code: TungsteniteCloseCode::from(frame.code),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn tungstenite_to_axum_message(message: TungsteniteMessage) -> Option<AxumWsMessage> {
    match message {
        TungsteniteMessage::Text(text) => Some(AxumWsMessage::Text(text.to_string().into())),
        TungsteniteMessage::Binary(bytes) => Some(AxumWsMessage::Binary(bytes)),
        TungsteniteMessage::Ping(bytes) => Some(AxumWsMessage::Ping(bytes)),
        TungsteniteMessage::Pong(bytes) => Some(AxumWsMessage::Pong(bytes)),
        TungsteniteMessage::Close(frame) => {
            Some(AxumWsMessage::Close(frame.map(|frame| AxumCloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            })))
        }
        TungsteniteMessage::Frame(_) => None,
    }
}

fn rewrite_request_path(request: Request, normalized_path: &str) -> Option<Request> {
    let path_and_query =
        if let Some(query) = request.uri().query().filter(|value| !value.is_empty()) {
            format!("{normalized_path}?{query}")
        } else {
            normalized_path.to_string()
        };
    let uri = Uri::builder().path_and_query(path_and_query).build().ok()?;
    let (mut parts, body) = request.into_parts();
    parts.uri = uri;
    Some(Request::from_parts(parts, body))
}

pub(crate) fn normalize_amp_provider_alias_path(path: &str) -> Option<String> {
    crate::control::canonical_amp_provider_alias_path(path)
}

fn amp_proxy_json_error(status: http::StatusCode, code: &str, detail: &str) -> Response<Body> {
    (status, Json(json!({ "error": code, "detail": detail }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        amp_provider_model_miss_fallback_path, build_amp_upstream_url,
        build_amp_websocket_upstream_url, disable_amp_thread_actor_features,
        extract_amp_client_api_key, is_amp_get_user_info_request, is_amp_thread_actor_request_path,
        normalize_amp_provider_alias_path, select_amp_upstream_api_key, should_forward_amp_header,
        should_forward_amp_rewritten_response_header, should_forward_amp_websocket_header,
        websocket_protocols, AmpProviderProxyOriginalPathAndQuery, AmpProxyConfig,
        AmpProxyUpstreamApiKeyRoute,
    };
    use crate::LocalExecutionRuntimeMissDiagnostic;
    use axum::http::{HeaderMap, HeaderName, HeaderValue, Request};
    use serde_json::json;

    #[test]
    fn amp_provider_alias_paths_map_to_existing_aether_public_paths() {
        assert_eq!(
            normalize_amp_provider_alias_path("/api/provider/openai/v1/chat/completions"),
            Some("/v1/chat/completions".to_string())
        );
        assert_eq!(
            normalize_amp_provider_alias_path("/api/provider/anthropic/v1/messages/count_tokens"),
            Some("/v1/messages/count_tokens".to_string())
        );
        assert_eq!(
            normalize_amp_provider_alias_path(
                "/api/provider/google/v1beta1/publishers/google/models/gemini-2.5-pro:generateContent"
            ),
            Some("/v1beta/models/gemini-2.5-pro:generateContent".to_string())
        );
        assert_eq!(
            normalize_amp_provider_alias_path(
                "/api/provider/google/v1beta1/models/gemini-2.5-pro:streamGenerateContent"
            ),
            Some("/v1beta/models/gemini-2.5-pro:streamGenerateContent".to_string())
        );
    }

    #[test]
    fn amp_upstream_key_routes_prefer_matching_client_key() {
        let config = AmpProxyConfig {
            upstream_url: "https://ampcode.com".to_string(),
            upstream_api_key: "default-upstream".to_string(),
            upstream_api_keys: vec![AmpProxyUpstreamApiKeyRoute {
                api_keys: vec!["client-a".to_string()],
                upstream_api_key: "tenant-a-upstream".to_string(),
            }],
            fallback_to_upstream_on_model_miss: false,
            force_legacy_worker_runtime: false,
        };

        assert_eq!(
            select_amp_upstream_api_key(&config, Some("client-a")),
            Some("tenant-a-upstream".to_string())
        );
        assert_eq!(
            select_amp_upstream_api_key(&config, Some("client-b")),
            Some("default-upstream".to_string())
        );
    }

    #[test]
    fn amp_upstream_url_preserves_path_and_strips_matching_client_credentials() {
        let url = build_amp_upstream_url(
            "https://ampcode.com/base",
            "/api/user?key=client-a&keep=1&auth_token=other",
            Some("client-a"),
        )
        .expect("url should build");

        assert_eq!(
            url,
            "https://ampcode.com/base/api/user?keep=1&auth_token=other"
        );
    }

    #[test]
    fn amp_client_api_key_can_be_read_from_headers_or_query() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer client-bearer"),
        );

        assert_eq!(
            extract_amp_client_api_key(&headers, None).as_deref(),
            Some("client-bearer")
        );
        assert_eq!(
            extract_amp_client_api_key(&HeaderMap::new(), Some("key=client-query")).as_deref(),
            Some("client-query")
        );
    }

    #[test]
    fn amp_management_proxy_does_not_forward_client_auth_headers() {
        for name in ["authorization", "x-api-key", "x-goog-api-key", "api-key"] {
            let name = HeaderName::from_bytes(name.as_bytes()).expect("header should parse");
            assert!(!should_forward_amp_header(&name), "{name}");
        }

        assert!(should_forward_amp_header(&HeaderName::from_static(
            "anthropic-version"
        )));
    }

    #[test]
    fn amp_websocket_upstream_url_uses_websocket_schemes() {
        assert_eq!(
            build_amp_websocket_upstream_url("https://ampcode.com/api/thread-actors/abc")
                .expect("url should build"),
            "wss://ampcode.com/api/thread-actors/abc"
        );
        assert_eq!(
            build_amp_websocket_upstream_url("http://localhost:3000/api/thread-actors/abc")
                .expect("url should build"),
            "ws://localhost:3000/api/thread-actors/abc"
        );
    }

    #[test]
    fn amp_websocket_proxy_does_not_forward_handshake_or_auth_headers() {
        for name in [
            "authorization",
            "x-api-key",
            "connection",
            "upgrade",
            "sec-websocket-key",
            "sec-websocket-version",
            "sec-websocket-extensions",
        ] {
            let name = HeaderName::from_bytes(name.as_bytes()).expect("header should parse");
            assert!(!should_forward_amp_websocket_header(&name), "{name}");
        }

        assert!(should_forward_amp_websocket_header(
            &HeaderName::from_static("sec-websocket-protocol")
        ));
    }

    #[test]
    fn amp_websocket_protocols_parse_comma_separated_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("alpha, beta"),
        );

        assert_eq!(websocket_protocols(&headers), vec!["alpha", "beta"]);
    }

    #[test]
    fn amp_model_miss_fallback_requires_enabled_config_and_provider_alias() {
        let enabled_config = AmpProxyConfig {
            upstream_url: "https://ampcode.com".to_string(),
            upstream_api_key: String::new(),
            upstream_api_keys: Vec::new(),
            fallback_to_upstream_on_model_miss: true,
            force_legacy_worker_runtime: false,
        };
        let disabled_config = AmpProxyConfig {
            fallback_to_upstream_on_model_miss: false,
            ..enabled_config.clone()
        };
        let diagnostic = LocalExecutionRuntimeMissDiagnostic {
            reason: "candidate_list_empty".to_string(),
            requested_model: Some("claude-haiku-4-5-20251001".to_string()),
            ..LocalExecutionRuntimeMissDiagnostic::default()
        };

        let mut request = Request::builder()
            .uri("/v1/messages")
            .body(())
            .expect("request should build");
        request
            .extensions_mut()
            .insert(AmpProviderProxyOriginalPathAndQuery(
                "/api/provider/anthropic/v1/messages".to_string(),
            ));
        let (parts, _) = request.into_parts();

        assert_eq!(
            amp_provider_model_miss_fallback_path(&enabled_config, &parts, Some(&diagnostic)),
            Some("/api/provider/anthropic/v1/messages")
        );
        assert_eq!(
            amp_provider_model_miss_fallback_path(&disabled_config, &parts, Some(&diagnostic)),
            None
        );

        let non_model_miss = LocalExecutionRuntimeMissDiagnostic {
            reason: "all_candidates_skipped".to_string(),
            ..diagnostic
        };
        assert_eq!(
            amp_provider_model_miss_fallback_path(&enabled_config, &parts, Some(&non_model_miss)),
            None
        );
    }

    #[test]
    fn amp_get_user_info_detection_requires_internal_get_user_info_query() {
        assert!(is_amp_get_user_info_request("/api/internal?getUserInfo"));
        assert!(is_amp_get_user_info_request(
            "/api/internal/?foo=1&getUserInfo=true"
        ));
        assert!(!is_amp_get_user_info_request(
            "/api/internal?method=getUserInfo"
        ));
        assert!(!is_amp_get_user_info_request("/api/thread-actors"));
    }

    #[test]
    fn amp_thread_actor_path_detection_covers_subpaths() {
        assert!(is_amp_thread_actor_request_path("/api/thread-actors"));
        assert!(is_amp_thread_actor_request_path("/api/thread-actors/"));
        assert!(is_amp_thread_actor_request_path(
            "/api/thread-actors/session-1"
        ));
        assert!(!is_amp_thread_actor_request_path(
            "/api/thread-actors-extra"
        ));
        assert!(!is_amp_thread_actor_request_path("/api/internal"));
    }

    #[test]
    fn amp_get_user_info_rewrite_disables_thread_actor_features() {
        let mut payload = json!({
            "result": {
                "features": [
                    { "name": "thread-actors-tui", "enabled": true },
                    { "name": "accept-abuse-data-retention", "enabled": true }
                ]
            }
        });

        disable_amp_thread_actor_features(&mut payload);
        let features = payload
            .pointer("/result/features")
            .and_then(serde_json::Value::as_array)
            .expect("features should remain an array");
        assert!(features.iter().any(|feature| {
            feature.get("name").and_then(serde_json::Value::as_str) == Some("thread-actors-tui")
                && feature.get("enabled").and_then(serde_json::Value::as_bool) == Some(false)
        }));
        assert!(features.iter().any(|feature| {
            feature.get("name").and_then(serde_json::Value::as_str) == Some("thread-actors-traces")
                && feature.get("enabled").and_then(serde_json::Value::as_bool) == Some(false)
        }));
        assert!(features.iter().any(|feature| {
            feature.get("name").and_then(serde_json::Value::as_str)
                == Some("accept-abuse-data-retention")
                && feature.get("enabled").and_then(serde_json::Value::as_bool) == Some(true)
        }));
    }

    #[test]
    fn amp_rewritten_response_headers_drop_body_specific_headers() {
        assert!(!should_forward_amp_rewritten_response_header(
            &HeaderName::from_static("content-length")
        ));
        assert!(!should_forward_amp_rewritten_response_header(
            &HeaderName::from_static("content-encoding")
        ));
        assert!(should_forward_amp_rewritten_response_header(
            &HeaderName::from_static("content-type")
        ));
    }
}
