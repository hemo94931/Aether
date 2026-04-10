use axum::http::HeaderMap;
use url::Url;

use crate::constants::{
    EXECUTION_RUNTIME_LOOP_GUARD_HEADER, EXECUTION_RUNTIME_LOOP_GUARD_VALUE,
    EXECUTION_RUNTIME_LOOP_GUARD_VIA_TOKEN,
};
use crate::headers::header_value_str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayBindHostKind {
    AnyLocal,
    Loopback,
    Exact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayBindTarget {
    host_kind: GatewayBindHostKind,
    host: String,
    port: u16,
}

pub(crate) fn request_has_execution_runtime_loop_guard(headers: &HeaderMap) -> bool {
    header_value_str(headers, EXECUTION_RUNTIME_LOOP_GUARD_HEADER)
        .is_some_and(|value| value.eq_ignore_ascii_case(EXECUTION_RUNTIME_LOOP_GUARD_VALUE))
        || request_has_execution_runtime_via_guard(headers)
}

fn request_has_execution_runtime_via_guard(headers: &HeaderMap) -> bool {
    headers
        .get_all("via")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| {
            value
                .to_ascii_lowercase()
                .contains(EXECUTION_RUNTIME_LOOP_GUARD_VIA_TOKEN)
        })
}

pub(crate) fn frontdoor_self_loop_public_ai_path(path: &str) -> bool {
    matches!(
        path,
        "/v1/messages"
            | "/v1/messages/count_tokens"
            | "/v1/chat/completions"
            | "/v1/responses"
            | "/v1/responses/compact"
            | "/v1beta/files"
            | "/upload/v1beta/files"
            | "/v1beta/operations"
            | "/v1/videos"
    ) || path.starts_with("/v1/videos/")
        || path.starts_with("/v1beta/files/")
        || path.starts_with("/v1beta/operations/")
        || is_gemini_generation_path(path)
}

pub(crate) fn gateway_frontdoor_self_loop_guard_error(url: &str) -> Option<String> {
    let Some(bind) = std::env::var("AETHER_GATEWAY_BIND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return None;
    };
    gateway_frontdoor_self_loop_guard_error_with_bind(bind.as_str(), url)
}

pub(crate) fn gateway_frontdoor_self_loop_guard_error_with_bind(
    bind: &str,
    url: &str,
) -> Option<String> {
    gateway_frontdoor_self_loop_guard_matches_with_bind(bind, url).then(|| {
        format!(
            "upstream execution target resolves back to the local aether-gateway frontdoor: {url}"
        )
    })
}

pub(crate) fn gateway_frontdoor_self_loop_guard_matches_with_bind(bind: &str, url: &str) -> bool {
    let Some(bind_target) = parse_gateway_bind_target(bind) else {
        return false;
    };
    let Some(target_url) = Url::parse(url).ok() else {
        return false;
    };
    if !frontdoor_self_loop_public_ai_path(target_url.path()) {
        return false;
    }

    let Some(target_host) = target_url.host_str() else {
        return false;
    };
    let Some(target_port) = target_url.port_or_known_default() else {
        return false;
    };
    if target_port != bind_target.port {
        return false;
    }

    let target_host = normalize_host_for_frontdoor_loop_guard(target_host);
    match bind_target.host_kind {
        GatewayBindHostKind::AnyLocal | GatewayBindHostKind::Loopback => {
            is_loopbackish_host(target_host.as_str())
        }
        GatewayBindHostKind::Exact => target_host == bind_target.host,
    }
}

fn is_gemini_generation_path(path: &str) -> bool {
    path.strip_prefix("/v1/models/")
        .or_else(|| path.strip_prefix("/v1beta/models/"))
        .is_some_and(|suffix| {
            suffix.contains(":generateContent")
                || suffix.contains(":streamGenerateContent")
                || suffix.contains(":predictLongRunning")
        })
}

fn parse_gateway_bind_target(bind: &str) -> Option<GatewayBindTarget> {
    let trimmed = bind.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(socket_addr) = trimmed.parse::<std::net::SocketAddr>() {
        let (host_kind, host) = match socket_addr.ip() {
            std::net::IpAddr::V4(ip) if ip.is_unspecified() => {
                (GatewayBindHostKind::AnyLocal, "0.0.0.0".to_string())
            }
            std::net::IpAddr::V4(ip) if ip.is_loopback() => {
                (GatewayBindHostKind::Loopback, ip.to_string())
            }
            std::net::IpAddr::V4(ip) => (GatewayBindHostKind::Exact, ip.to_string()),
            std::net::IpAddr::V6(ip) if ip.is_unspecified() => {
                (GatewayBindHostKind::AnyLocal, "::".to_string())
            }
            std::net::IpAddr::V6(ip) if ip.is_loopback() => {
                (GatewayBindHostKind::Loopback, ip.to_string())
            }
            std::net::IpAddr::V6(ip) => (GatewayBindHostKind::Exact, ip.to_string()),
        };
        return Some(GatewayBindTarget {
            host_kind,
            host,
            port: socket_addr.port(),
        });
    }

    let (host, port) = trimmed.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return None;
    }

    let normalized_host = normalize_host_for_frontdoor_loop_guard(host);
    let host_kind = if matches!(normalized_host.as_str(), "0.0.0.0" | "::") {
        GatewayBindHostKind::AnyLocal
    } else if is_loopbackish_host(normalized_host.as_str()) {
        GatewayBindHostKind::Loopback
    } else {
        GatewayBindHostKind::Exact
    };

    Some(GatewayBindTarget {
        host_kind,
        host: normalized_host,
        port,
    })
}

fn normalize_host_for_frontdoor_loop_guard(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

fn is_loopbackish_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0" | "::")
}
