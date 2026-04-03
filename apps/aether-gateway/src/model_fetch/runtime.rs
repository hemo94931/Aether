use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use aether_contracts::{ExecutionPlan, ExecutionResult, RequestBody};
use aether_data::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use http::HeaderMap;
use regex::Regex;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::gateway::provider_transport::{
    apply_local_header_rules, build_passthrough_path_url, ensure_upstream_auth_header,
    resolve_local_gemini_auth, resolve_local_openai_chat_auth, resolve_local_standard_auth,
    resolve_local_vertex_api_key_query_auth, resolve_transport_execution_timeouts,
    resolve_transport_proxy_snapshot_with_tunnel_affinity, resolve_transport_tls_profile,
    LocalResolvedOAuthRequestAuth,
};
use crate::gateway::{AppState, GatewayError};

mod association_sync;

use self::association_sync::sync_provider_model_whitelist_associations;

const MODEL_FETCH_INTERVAL_MINUTES_DEFAULT: u64 = 1440;
const MODEL_FETCH_INTERVAL_MINUTES_MIN: u64 = 60;
const MODEL_FETCH_INTERVAL_MINUTES_MAX: u64 = 10080;
const MODEL_FETCH_STARTUP_DELAY_SECONDS_DEFAULT: u64 = 10;
const MODEL_FETCH_CACHE_KEY_PREFIX: &str = "upstream_models";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModelFetchRunSummary {
    pub(crate) attempted: usize,
    pub(crate) succeeded: usize,
    pub(crate) failed: usize,
    pub(crate) skipped: usize,
}

#[derive(Debug, Clone)]
struct SelectedFetchTarget {
    provider: StoredProviderCatalogProvider,
    endpoint: StoredProviderCatalogEndpoint,
    key: StoredProviderCatalogKey,
}

#[derive(Debug, Clone)]
struct ModelsFetchSuccess {
    fetched_model_ids: Vec<String>,
    cached_models: Vec<Value>,
}

pub(crate) fn spawn_model_fetch_worker(state: AppState) -> Option<tokio::task::JoinHandle<()>> {
    if !state.has_provider_catalog_data_reader() || !state.has_provider_catalog_data_writer() {
        return None;
    }

    Some(tokio::spawn(async move {
        if model_fetch_startup_enabled() {
            let startup_delay = model_fetch_startup_delay_seconds();
            if startup_delay > 0 {
                tokio::time::sleep(Duration::from_secs(startup_delay)).await;
            }
            if let Err(err) = run_model_fetch_cycle(&state, "startup").await {
                warn!(error = ?err, "gateway model fetch startup failed");
            }
        } else {
            info!("gateway model fetch startup disabled");
        }

        let mut interval = tokio::time::interval(Duration::from_secs(
            model_fetch_interval_minutes().saturating_mul(60),
        ));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(err) = run_model_fetch_cycle(&state, "tick").await {
                warn!(error = ?err, "gateway model fetch tick failed");
            }
        }
    }))
}

pub(crate) async fn perform_model_fetch_once(
    state: &AppState,
) -> Result<ModelFetchRunSummary, GatewayError> {
    if !state.data.has_provider_catalog_reader() || !state.data.has_provider_catalog_writer() {
        return Ok(ModelFetchRunSummary {
            attempted: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
        });
    }

    let providers = state.list_provider_catalog_providers(true).await?;
    if providers.is_empty() {
        return Ok(ModelFetchRunSummary {
            attempted: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
        });
    }

    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let mut endpoints_by_provider = HashMap::<String, Vec<StoredProviderCatalogEndpoint>>::new();
    for endpoint in state
        .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
        .await?
    {
        endpoints_by_provider
            .entry(endpoint.provider_id.clone())
            .or_default()
            .push(endpoint);
    }
    let mut keys_by_provider = HashMap::<String, Vec<StoredProviderCatalogKey>>::new();
    for key in state
        .list_provider_catalog_keys_by_provider_ids(&provider_ids)
        .await?
    {
        keys_by_provider
            .entry(key.provider_id.clone())
            .or_default()
            .push(key);
    }

    let mut targets = Vec::new();
    for provider in providers {
        let endpoints = endpoints_by_provider
            .remove(&provider.id)
            .unwrap_or_default();
        let keys = keys_by_provider.remove(&provider.id).unwrap_or_default();
        for key in keys {
            if !key.is_active || !key.auto_fetch_models {
                continue;
            }
            if let Some(endpoint) = select_models_fetch_endpoint(&endpoints, &key) {
                targets.push(SelectedFetchTarget {
                    provider: provider.clone(),
                    endpoint,
                    key,
                });
            } else {
                targets.push(SelectedFetchTarget {
                    provider: provider.clone(),
                    endpoint: StoredProviderCatalogEndpoint::new(
                        "__unsupported__".to_string(),
                        provider.id.clone(),
                        "__unsupported__".to_string(),
                        None,
                        None,
                        false,
                    )
                    .expect("unsupported sentinel endpoint should build")
                    .with_transport_fields(
                        "https://unsupported.invalid".to_string(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .expect("unsupported sentinel endpoint transport should build"),
                    key,
                });
            }
        }
    }

    let mut summary = ModelFetchRunSummary {
        attempted: targets.len(),
        succeeded: 0,
        failed: 0,
        skipped: 0,
    };
    for target in targets {
        match fetch_and_persist_key_models(state, &target).await? {
            KeyFetchDisposition::Succeeded => summary.succeeded += 1,
            KeyFetchDisposition::Failed => summary.failed += 1,
            KeyFetchDisposition::Skipped => summary.skipped += 1,
        }
    }
    Ok(summary)
}

async fn run_model_fetch_cycle(state: &AppState, phase: &'static str) -> Result<(), GatewayError> {
    let summary = perform_model_fetch_once(state).await?;
    if summary.attempted == 0 {
        debug!(phase, "gateway model fetch found no eligible keys");
        return Ok(());
    }

    info!(
        phase,
        attempted = summary.attempted,
        succeeded = summary.succeeded,
        failed = summary.failed,
        skipped = summary.skipped,
        "gateway model fetch cycle completed"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyFetchDisposition {
    Succeeded,
    Failed,
    Skipped,
}

async fn fetch_and_persist_key_models(
    state: &AppState,
    target: &SelectedFetchTarget,
) -> Result<KeyFetchDisposition, GatewayError> {
    let now_unix_secs = now_unix_secs();
    if target.endpoint.api_format == "__unsupported__" {
        persist_key_fetch_failure(
            state,
            &target.key,
            now_unix_secs,
            "No supported endpoint for Rust models fetch".to_string(),
        )
        .await?;
        return Ok(KeyFetchDisposition::Skipped);
    }

    let Some(transport) = state
        .read_provider_transport_snapshot(&target.provider.id, &target.endpoint.id, &target.key.id)
        .await?
    else {
        persist_key_fetch_failure(
            state,
            &target.key,
            now_unix_secs,
            "Provider transport snapshot unavailable".to_string(),
        )
        .await?;
        return Ok(KeyFetchDisposition::Skipped);
    };

    let result = match execute_models_fetch_request(state, &transport).await {
        Ok(result) => result,
        Err(err) => {
            persist_key_fetch_failure(state, &target.key, now_unix_secs, err.clone()).await?;
            warn!(
                provider_id = %target.provider.id,
                key_id = %target.key.id,
                message = %err,
                "gateway model fetch failed"
            );
            return Ok(KeyFetchDisposition::Failed);
        }
    };

    let filtered_models = apply_model_filters(
        &result.fetched_model_ids,
        json_string_list(target.key.locked_models.as_ref()),
        json_string_list(target.key.model_include_patterns.as_ref()),
        json_string_list(target.key.model_exclude_patterns.as_ref()),
    );

    persist_key_fetch_success(state, &target.key, now_unix_secs, &filtered_models).await?;
    write_upstream_models_cache(
        state,
        &target.provider.id,
        &target.key.id,
        &result.cached_models,
    )
    .await;
    sync_provider_model_whitelist_associations(state, &target.provider.id, &filtered_models)
        .await?;
    Ok(KeyFetchDisposition::Succeeded)
}

async fn execute_models_fetch_request(
    state: &AppState,
    transport: &crate::gateway::provider_transport::GatewayProviderTransportSnapshot,
) -> Result<ModelsFetchSuccess, String> {
    let (upstream_url, provider_api_format) = build_models_fetch_url(transport)
        .ok_or_else(|| "Rust models fetch does not support this provider format yet".to_string())?;
    let (auth_header_name, auth_header_value) = resolve_models_fetch_auth(state, transport)
        .await?
        .ok_or_else(|| {
            "Rust models fetch auth resolution is not supported for this key".to_string()
        })?;

    let mut headers = BTreeMap::from([(auth_header_name.clone(), auth_header_value.clone())]);
    if !apply_local_header_rules(
        &mut headers,
        transport.endpoint.header_rules.as_ref(),
        &[auth_header_name.as_str()],
        &json!({}),
        None,
    ) {
        return Err("Endpoint header_rules application failed".to_string());
    }
    ensure_upstream_auth_header(&mut headers, &auth_header_name, &auth_header_value);

    let plan = ExecutionPlan {
        request_id: format!("req-model-fetch-{}", transport.key.id),
        candidate_id: None,
        provider_name: Some(transport.provider.name.clone()),
        provider_id: transport.provider.id.clone(),
        endpoint_id: transport.endpoint.id.clone(),
        key_id: transport.key.id.clone(),
        method: "GET".to_string(),
        url: upstream_url,
        headers,
        content_type: None,
        content_encoding: None,
        body: RequestBody {
            json_body: None,
            body_bytes_b64: None,
            body_ref: None,
        },
        stream: false,
        client_api_format: provider_api_format.clone(),
        provider_api_format,
        model_name: None,
        proxy: resolve_transport_proxy_snapshot_with_tunnel_affinity(state, transport).await,
        tls_profile: resolve_transport_tls_profile(transport),
        timeouts: resolve_transport_execution_timeouts(transport),
    };

    let result = crate::gateway::execute_execution_runtime_sync_plan(state, None, &plan)
        .await
        .map_err(|err| format!("{err:?}"))?;

    if result.status_code != 200 {
        let message = result
            .body
            .as_ref()
            .and_then(|body| body.json_body.as_ref())
            .and_then(extract_error_message)
            .or_else(|| {
                result.error.as_ref().and_then(|error| {
                    let message = error.message.trim();
                    (!message.is_empty()).then_some(message.to_string())
                })
            })
            .unwrap_or_else(|| format!("upstream returned status {}", result.status_code));
        return Err(message);
    }

    let body_json = result
        .body
        .as_ref()
        .and_then(|body| body.json_body.as_ref())
        .ok_or_else(|| "models fetch response body is missing JSON payload".to_string())?;
    parse_models_response(transport, body_json)
}

fn extract_error_message(value: &Value) -> Option<String> {
    value
        .get("error")
        .and_then(Value::as_object)
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn build_models_fetch_url(
    transport: &crate::gateway::provider_transport::GatewayProviderTransportSnapshot,
) -> Option<(String, String)> {
    let api_format = normalize_api_format(&transport.endpoint.api_format);
    if !crate::gateway::provider_transport::provider_type_supports_model_fetch(
        &transport.provider.provider_type,
    ) {
        return None;
    }

    let url = if api_format.starts_with("openai:") || api_format.starts_with("claude:") {
        build_v1_models_url(&transport.endpoint.base_url)
    } else if api_format.starts_with("gemini:") {
        build_gemini_models_url(&transport.endpoint.base_url)
    } else {
        return None;
    }?;
    Some((url, api_format))
}

fn build_v1_models_url(base_url: &str) -> Option<String> {
    let (trimmed_base_url, query) = split_url_query(base_url);
    let trimmed_base_url = trimmed_base_url.trim_end_matches('/');
    if trimmed_base_url.is_empty() {
        return None;
    }
    let mut url = if trimmed_base_url.ends_with("/v1") {
        format!("{trimmed_base_url}/models")
    } else {
        format!("{trimmed_base_url}/v1/models")
    };
    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}

fn build_gemini_models_url(base_url: &str) -> Option<String> {
    let (trimmed_base_url, base_query) = split_url_query(base_url);
    let trimmed_base_url = trimmed_base_url.trim_end_matches('/');
    if trimmed_base_url.is_empty() {
        return None;
    }

    let mut url = if trimmed_base_url.ends_with("/v1beta") {
        format!("{trimmed_base_url}/models")
    } else if trimmed_base_url.contains("/v1beta/models") {
        trimmed_base_url.to_string()
    } else {
        format!("{trimmed_base_url}/v1beta/models")
    };
    if let Some(query) = base_query.filter(|value| !value.trim().is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}

fn split_url_query(base_url: &str) -> (&str, Option<&str>) {
    let trimmed = base_url.trim();
    trimmed
        .split_once('?')
        .map(|(base, query)| (base, Some(query)))
        .unwrap_or((trimmed, None))
}

async fn resolve_models_fetch_auth(
    state: &AppState,
    transport: &crate::gateway::provider_transport::GatewayProviderTransportSnapshot,
) -> Result<Option<(String, String)>, String> {
    if transport.key.auth_type.trim().eq_ignore_ascii_case("oauth")
        || transport.key.auth_type.trim().eq_ignore_ascii_case("kiro")
    {
        return match state.resolve_local_oauth_request_auth(transport).await {
            Ok(Some(LocalResolvedOAuthRequestAuth::Header { name, value })) => {
                Ok(Some((name, value)))
            }
            Ok(Some(LocalResolvedOAuthRequestAuth::Kiro(_))) => Ok(None),
            Ok(None) => Ok(None),
            Err(err) => Err(format!("{err:?}")),
        };
    }

    if let Some(auth) = resolve_local_openai_chat_auth(transport) {
        return Ok(Some(auth));
    }
    if let Some(auth) = resolve_local_standard_auth(transport) {
        return Ok(Some(auth));
    }
    if let Some(auth) = resolve_local_gemini_auth(transport) {
        return Ok(Some(auth));
    }
    if let Some(query_auth) = resolve_local_vertex_api_key_query_auth(transport) {
        let url = build_passthrough_path_url(
            &transport.endpoint.base_url,
            "/v1/publishers/google/models",
            Some(&format!("{}={}", query_auth.name, query_auth.value)),
            &[],
        );
        if url.is_some() {
            return Ok(None);
        }
    }
    Ok(None)
}

fn parse_models_response(
    transport: &crate::gateway::provider_transport::GatewayProviderTransportSnapshot,
    body: &Value,
) -> Result<ModelsFetchSuccess, String> {
    let api_format = normalize_api_format(&transport.endpoint.api_format);
    let mut cached_models = Vec::new();
    let mut fetched_model_ids = Vec::new();
    let mut seen = BTreeSet::new();

    if api_format.starts_with("openai:") || api_format.starts_with("claude:") {
        let items = if let Some(items) = body.get("data").and_then(Value::as_array) {
            items
        } else if let Some(items) = body.as_array() {
            items
        } else {
            return Err("models response is missing data array".to_string());
        };
        for item in items {
            let Some(model_id) = item
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen.insert(model_id.to_string()) {
                continue;
            }
            fetched_model_ids.push(model_id.to_string());
            cached_models.push(normalize_cached_model(item, model_id, &api_format));
        }
    } else if api_format.starts_with("gemini:") {
        let items = body
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| "gemini models response is missing models array".to_string())?;
        for item in items {
            let Some(name) = item
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let model_id = name.strip_prefix("models/").unwrap_or(name).trim();
            if model_id.is_empty() || !seen.insert(model_id.to_string()) {
                continue;
            }
            fetched_model_ids.push(model_id.to_string());
            cached_models.push(normalize_cached_model(item, model_id, &api_format));
        }
    } else {
        return Err("models response parser does not support this provider format".to_string());
    }

    Ok(ModelsFetchSuccess {
        fetched_model_ids,
        cached_models,
    })
}

fn normalize_cached_model(item: &Value, model_id: &str, api_format: &str) -> Value {
    let mut object = item.as_object().cloned().unwrap_or_default();
    object.insert("id".to_string(), Value::String(model_id.to_string()));
    object.insert(
        "api_formats".to_string(),
        Value::Array(vec![Value::String(api_format.to_string())]),
    );
    object.remove("api_format");
    Value::Object(object)
}

async fn persist_key_fetch_failure(
    state: &AppState,
    key: &StoredProviderCatalogKey,
    now_unix_secs: u64,
    error: String,
) -> Result<(), GatewayError> {
    let mut updated = key.clone();
    updated.last_models_fetch_at_unix_secs = Some(now_unix_secs);
    updated.last_models_fetch_error = Some(error);
    updated.updated_at_unix_secs = Some(now_unix_secs);
    state.update_provider_catalog_key(&updated).await?;
    Ok(())
}

async fn persist_key_fetch_success(
    state: &AppState,
    key: &StoredProviderCatalogKey,
    now_unix_secs: u64,
    allowed_models: &[String],
) -> Result<(), GatewayError> {
    let mut updated = key.clone();
    updated.allowed_models = if allowed_models.is_empty() {
        None
    } else {
        Some(json!(allowed_models))
    };
    updated.last_models_fetch_at_unix_secs = Some(now_unix_secs);
    updated.last_models_fetch_error = None;
    updated.updated_at_unix_secs = Some(now_unix_secs);
    state.update_provider_catalog_key(&updated).await?;
    Ok(())
}

async fn write_upstream_models_cache(
    state: &AppState,
    provider_id: &str,
    key_id: &str,
    cached_models: &[Value],
) {
    let Some(runner) = state.redis_kv_runner() else {
        return;
    };
    let Ok(serialized) = serde_json::to_string(&aggregate_models_for_cache(cached_models)) else {
        return;
    };
    let cache_key = format!("{MODEL_FETCH_CACHE_KEY_PREFIX}:{provider_id}:{key_id}");
    if let Err(err) = runner
        .setex(
            &cache_key,
            &serialized,
            Some(model_fetch_interval_minutes().saturating_mul(60)),
        )
        .await
    {
        debug!(
            provider_id = %provider_id,
            key_id = %key_id,
            error = %err,
            "gateway model fetch cache write failed"
        );
    }
}

fn aggregate_models_for_cache(models: &[Value]) -> Vec<Value> {
    let mut aggregated = BTreeMap::<String, serde_json::Map<String, Value>>::new();
    let mut order = Vec::<String>::new();

    for model in models {
        let Some(object) = model.as_object() else {
            continue;
        };
        let Some(model_id) = object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };

        let entry = aggregated.entry(model_id.to_string()).or_insert_with(|| {
            order.push(model_id.to_string());
            let mut cloned = object.clone();
            cloned.remove("api_format");
            cloned
        });

        let api_formats = object
            .get("api_formats")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let existing_formats = entry
            .get("api_formats")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let merged_formats = existing_formats
            .union(&api_formats)
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>();
        entry.insert("api_formats".to_string(), Value::Array(merged_formats));

        for (key, value) in object {
            if key == "api_format" || entry.contains_key(key) {
                continue;
            }
            entry.insert(key.clone(), value.clone());
        }
    }

    order
        .into_iter()
        .filter_map(|model_id| aggregated.remove(&model_id))
        .map(Value::Object)
        .collect()
}

fn select_models_fetch_endpoint(
    endpoints: &[StoredProviderCatalogEndpoint],
    key: &StoredProviderCatalogKey,
) -> Option<StoredProviderCatalogEndpoint> {
    let key_formats = json_string_list(key.api_formats.as_ref())
        .into_iter()
        .map(|value| normalize_api_format(&value))
        .collect::<BTreeSet<_>>();
    endpoints
        .iter()
        .filter(|endpoint| endpoint.is_active)
        .find(|endpoint| {
            let api_format = normalize_api_format(&endpoint.api_format);
            (key_formats.is_empty() || key_formats.contains(&api_format))
                && endpoint_supports_rust_models_fetch(&endpoint.api_format)
        })
        .cloned()
}

fn endpoint_supports_rust_models_fetch(api_format: &str) -> bool {
    let api_format = normalize_api_format(api_format);
    matches!(
        api_format.as_str(),
        "openai:chat"
            | "openai:cli"
            | "openai:responses"
            | "openai:compact"
            | "claude:chat"
            | "claude:cli"
            | "gemini:chat"
            | "gemini:cli"
    )
}

fn apply_model_filters(
    fetched_model_ids: &[String],
    locked_models: Vec<String>,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
) -> Vec<String> {
    let mut filtered = BTreeSet::new();
    for model_id in fetched_model_ids {
        if model_id.trim().is_empty() {
            continue;
        }
        let included = if include_patterns.is_empty() {
            true
        } else {
            include_patterns
                .iter()
                .any(|pattern| wildcard_matches(pattern, model_id))
        };
        if !included {
            continue;
        }
        let excluded = exclude_patterns
            .iter()
            .any(|pattern| wildcard_matches(pattern, model_id));
        if !excluded {
            filtered.insert(model_id.trim().to_string());
        }
    }
    for model in locked_models {
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            filtered.insert(trimmed.to_string());
        }
    }
    filtered.into_iter().collect()
}

fn wildcard_matches(pattern: &str, model_id: &str) -> bool {
    let mut regex = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            other => regex.push_str(&regex::escape(&other.to_string())),
        }
    }
    regex.push('$');
    Regex::new(&regex)
        .ok()
        .is_some_and(|compiled| compiled.is_match(model_id))
}

fn json_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_api_format(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn model_fetch_interval_minutes() -> u64 {
    std::env::var("MODEL_FETCH_INTERVAL_MINUTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| {
            value.clamp(
                MODEL_FETCH_INTERVAL_MINUTES_MIN,
                MODEL_FETCH_INTERVAL_MINUTES_MAX,
            )
        })
        .unwrap_or(MODEL_FETCH_INTERVAL_MINUTES_DEFAULT)
}

fn model_fetch_startup_enabled() -> bool {
    std::env::var("MODEL_FETCH_STARTUP_ENABLED")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(true)
}

fn model_fetch_startup_delay_seconds() -> u64 {
    std::env::var("MODEL_FETCH_STARTUP_DELAY_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(MODEL_FETCH_STARTUP_DELAY_SECONDS_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::{aggregate_models_for_cache, apply_model_filters, build_gemini_models_url};
    use serde_json::json;

    #[test]
    fn apply_model_filters_respects_include_exclude_and_locked_models() {
        let filtered = apply_model_filters(
            &[
                "gpt-5".to_string(),
                "gpt-beta".to_string(),
                "claude-4".to_string(),
            ],
            vec!["locked-model".to_string()],
            vec!["gpt-*".to_string()],
            vec!["gpt-beta".to_string()],
        );
        assert_eq!(
            filtered,
            vec!["gpt-5".to_string(), "locked-model".to_string()]
        );
    }

    #[test]
    fn aggregate_models_for_cache_merges_api_formats_by_model_id() {
        let aggregated = aggregate_models_for_cache(&[
            json!({"id":"gpt-5","api_formats":["openai:chat"]}),
            json!({"id":"gpt-5","api_formats":["openai:cli"]}),
        ]);
        assert_eq!(aggregated.len(), 1);
        assert_eq!(
            aggregated[0]["api_formats"],
            json!(["openai:chat", "openai:cli"])
        );
    }

    #[test]
    fn build_gemini_models_url_preserves_base_query() {
        let url =
            build_gemini_models_url("https://generativelanguage.googleapis.com/v1beta?key=abc")
                .expect("gemini models url should build");
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models?key=abc"
        );
    }
}
