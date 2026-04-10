use super::request::{
    admin_provider_ops_execute_get_text, admin_provider_ops_execute_get_text_no_redirect,
};
use crate::handlers::admin::request::AdminAppState;
use aether_admin::provider::ops::admin_provider_ops_anyrouter_compute_acw_sc_v2;
use aether_contracts::ProxySnapshot;
use aether_data::repository::proxy_nodes::StoredProxyNode;
use aether_provider_transport::TransportTunnelAffinityLookup;
use regex::Regex;
use serde_json::{json, Map, Value};
use url::Url;

const TUNNEL_BASE_URL_EXTRA_KEY: &str = "tunnel_base_url";
const TUNNEL_OWNER_INSTANCE_ID_EXTRA_KEY: &str = "tunnel_owner_instance_id";
const TUNNEL_OWNER_OBSERVED_AT_EXTRA_KEY: &str = "tunnel_owner_observed_at_unix_secs";

pub(in super::super) struct AdminProviderOpsAnyrouterChallenge {
    pub(in super::super) acw_cookie: String,
}

pub(in super::super) async fn admin_provider_ops_anyrouter_acw_cookie(
    state: &AdminAppState<'_>,
    base_url: &str,
    connector_config: Option<&Map<String, Value>>,
) -> Option<AdminProviderOpsAnyrouterChallenge> {
    let proxy_snapshot = admin_provider_ops_resolve_proxy_snapshot(state, connector_config).await;
    let headers = reqwest::header::HeaderMap::from_iter([(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    )]);
    let response = if admin_provider_ops_proxy_uses_tunnel(proxy_snapshot.as_ref()) {
        admin_provider_ops_execute_get_text(
            state,
            "provider-ops-acw:anyrouter",
            base_url.trim_end_matches('/'),
            &headers,
            proxy_snapshot.as_ref(),
        )
        .await
        .ok()?
    } else {
        admin_provider_ops_execute_get_text_no_redirect(
            base_url.trim_end_matches('/'),
            &headers,
            proxy_snapshot.as_ref(),
        )
        .await
        .ok()?
    };
    let compiled = Regex::new(r"var\s+arg1\s*=\s*'([0-9a-fA-F]{40})'").ok()?;
    let captures = compiled.captures(&response.body)?;
    let arg1 = captures.get(1)?.as_str();
    admin_provider_ops_anyrouter_compute_acw_sc_v2(arg1).map(|value| {
        AdminProviderOpsAnyrouterChallenge {
            acw_cookie: format!("acw_sc__v2={value}"),
        }
    })
}

pub(in super::super) async fn admin_provider_ops_resolve_proxy_snapshot(
    state: &AdminAppState<'_>,
    connector_config: Option<&Map<String, Value>>,
) -> Option<ProxySnapshot> {
    let explicit_node_id = connector_config
        .and_then(|config| admin_provider_ops_string_field(config, "proxy_node_id"));
    if let Some(snapshot) =
        admin_provider_ops_resolve_proxy_node_snapshot(state, explicit_node_id.as_deref()).await
    {
        return Some(snapshot);
    }

    if explicit_node_id.is_none() {
        let system_node_id = state
            .read_system_config_json_value("system_proxy_node_id")
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::trim).map(ToOwned::to_owned))
            .filter(|value| !value.is_empty());
        if let Some(snapshot) =
            admin_provider_ops_resolve_proxy_node_snapshot(state, system_node_id.as_deref()).await
        {
            return Some(snapshot);
        }
    }

    connector_config
        .and_then(|config| config.get("proxy"))
        .and_then(admin_provider_ops_legacy_proxy_snapshot)
}

async fn admin_provider_ops_resolve_proxy_node_snapshot(
    state: &AdminAppState<'_>,
    node_id: Option<&str>,
) -> Option<ProxySnapshot> {
    let node_id = node_id.map(str::trim).filter(|value| !value.is_empty())?;
    let node = state.find_proxy_node(node_id).await.ok().flatten()?;
    if node.status.trim() != "online" {
        return None;
    }
    if node.tunnel_mode && node.tunnel_connected {
        let mut extra = Map::new();
        if let Ok(Some(owner)) = state.app().lookup_tunnel_attachment_owner(node_id).await {
            extra.insert(
                TUNNEL_BASE_URL_EXTRA_KEY.to_string(),
                Value::String(owner.relay_base_url),
            );
            extra.insert(
                TUNNEL_OWNER_INSTANCE_ID_EXTRA_KEY.to_string(),
                Value::String(owner.gateway_instance_id),
            );
            extra.insert(
                TUNNEL_OWNER_OBSERVED_AT_EXTRA_KEY.to_string(),
                json!(owner.observed_at_unix_secs),
            );
        }
        return Some(ProxySnapshot {
            enabled: Some(true),
            mode: Some("tunnel".to_string()),
            node_id: Some(node_id.to_string()),
            label: Some(node.name),
            url: None,
            extra: if extra.is_empty() {
                None
            } else {
                Some(Value::Object(extra))
            },
        });
    }
    if !node.is_manual {
        return None;
    }
    let proxy_url = node
        .proxy_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(ProxySnapshot {
        enabled: Some(true),
        mode: admin_provider_ops_proxy_mode(Some(proxy_url)),
        node_id: Some(node.id.clone()),
        label: Some(node.name.clone()),
        url: admin_provider_ops_proxy_url_with_node_auth(&node),
        extra: None,
    })
}

fn admin_provider_ops_legacy_proxy_snapshot(value: &Value) -> Option<ProxySnapshot> {
    match value {
        Value::String(proxy_url) => {
            let proxy_url = proxy_url.trim();
            if proxy_url.is_empty() {
                return None;
            }
            Some(ProxySnapshot {
                enabled: Some(true),
                mode: admin_provider_ops_proxy_mode(Some(proxy_url)),
                node_id: None,
                label: None,
                url: Some(proxy_url.to_string()),
                extra: None,
            })
        }
        Value::Object(object) => {
            if object.get("enabled").and_then(Value::as_bool) == Some(false) {
                return None;
            }
            let proxy_url = object
                .get("url")
                .or_else(|| object.get("proxy_url"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let username = object
                .get("username")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let password = object
                .get("password")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            Some(ProxySnapshot {
                enabled: Some(true),
                mode: object
                    .get("mode")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .or_else(|| admin_provider_ops_proxy_mode(Some(proxy_url))),
                node_id: None,
                label: object
                    .get("label")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                url: admin_provider_ops_inject_proxy_auth(proxy_url, username, password)
                    .or_else(|| Some(proxy_url.to_string())),
                extra: None,
            })
        }
        _ => None,
    }
}

fn admin_provider_ops_proxy_url_with_node_auth(node: &StoredProxyNode) -> Option<String> {
    let proxy_url = node
        .proxy_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let username = node
        .proxy_username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let password = node
        .proxy_password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    admin_provider_ops_inject_proxy_auth(proxy_url, username, password)
        .or_else(|| Some(proxy_url.to_string()))
}

fn admin_provider_ops_inject_proxy_auth(
    proxy_url: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Option<String> {
    let username = username.filter(|value| !value.is_empty())?;
    let mut parsed = Url::parse(proxy_url).ok()?;
    parsed.set_username(username).ok()?;
    parsed.set_password(password).ok()?;
    Some(parsed.to_string())
}

fn admin_provider_ops_proxy_mode(proxy_url: Option<&str>) -> Option<String> {
    proxy_url
        .and_then(|value| {
            Url::parse(value)
                .ok()
                .map(|parsed| parsed.scheme().to_string())
        })
        .or_else(|| {
            proxy_url.and_then(|value| {
                value
                    .split_once("://")
                    .map(|(scheme, _)| scheme.trim().to_ascii_lowercase())
                    .filter(|scheme| !scheme.is_empty())
            })
        })
}

fn admin_provider_ops_proxy_uses_tunnel(proxy_snapshot: Option<&ProxySnapshot>) -> bool {
    proxy_snapshot.is_some_and(|proxy| {
        proxy.mode.as_deref().map(str::trim) == Some("tunnel")
            || (proxy
                .url
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
                && proxy
                    .node_id
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()))
    })
}

fn admin_provider_ops_string_field(config: &Map<String, Value>, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
