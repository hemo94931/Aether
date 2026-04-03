use std::collections::{BTreeMap, BTreeSet};

use aether_data::repository::candidates::StoredRequestCandidate;
use aether_data::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_data::DataLayerError;

use super::candidates::RequestCandidateFinalStatus;
use super::state::GatewayDataState;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct DecisionTraceCandidate {
    #[serde(flatten)]
    pub(crate) candidate: StoredRequestCandidate,
    pub(crate) provider_name: Option<String>,
    pub(crate) provider_website: Option<String>,
    pub(crate) provider_type: Option<String>,
    pub(crate) endpoint_api_format: Option<String>,
    pub(crate) endpoint_api_family: Option<String>,
    pub(crate) endpoint_kind: Option<String>,
    pub(crate) provider_key_name: Option<String>,
    pub(crate) provider_key_auth_type: Option<String>,
    pub(crate) provider_key_capabilities: Option<serde_json::Value>,
    pub(crate) provider_key_is_active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct DecisionTrace {
    pub(crate) request_id: String,
    pub(crate) total_candidates: usize,
    pub(crate) final_status: RequestCandidateFinalStatus,
    pub(crate) total_latency_ms: u64,
    pub(crate) candidates: Vec<DecisionTraceCandidate>,
}

pub(crate) async fn read_decision_trace(
    state: &GatewayDataState,
    request_id: &str,
    attempted_only: bool,
) -> Result<Option<DecisionTrace>, DataLayerError> {
    let Some(trace) = state
        .read_request_candidate_trace(request_id, attempted_only)
        .await?
    else {
        return Ok(None);
    };

    let provider_ids = unique_ids(
        trace
            .candidates
            .iter()
            .filter_map(|item| item.provider_id.as_ref()),
    );
    let endpoint_ids = unique_ids(
        trace
            .candidates
            .iter()
            .filter_map(|item| item.endpoint_id.as_ref()),
    );
    let key_ids = unique_ids(
        trace
            .candidates
            .iter()
            .filter_map(|item| item.key_id.as_ref()),
    );

    let provider_map = state
        .list_provider_catalog_providers_by_ids(&provider_ids)
        .await?
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let endpoint_map = state
        .list_provider_catalog_endpoints_by_ids(&endpoint_ids)
        .await?
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let key_map = state
        .list_provider_catalog_keys_by_ids(&key_ids)
        .await?
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    Ok(Some(DecisionTrace {
        request_id: trace.request_id,
        total_candidates: trace.total_candidates,
        final_status: trace.final_status,
        total_latency_ms: trace.total_latency_ms,
        candidates: trace
            .candidates
            .into_iter()
            .map(|candidate| enrich_candidate(candidate, &provider_map, &endpoint_map, &key_map))
            .collect(),
    }))
}

fn enrich_candidate(
    candidate: StoredRequestCandidate,
    provider_map: &BTreeMap<String, StoredProviderCatalogProvider>,
    endpoint_map: &BTreeMap<String, StoredProviderCatalogEndpoint>,
    key_map: &BTreeMap<String, StoredProviderCatalogKey>,
) -> DecisionTraceCandidate {
    let provider = candidate
        .provider_id
        .as_ref()
        .and_then(|provider_id| provider_map.get(provider_id));
    let endpoint = candidate
        .endpoint_id
        .as_ref()
        .and_then(|endpoint_id| endpoint_map.get(endpoint_id));
    let provider_key = candidate
        .key_id
        .as_ref()
        .and_then(|key_id| key_map.get(key_id));

    DecisionTraceCandidate {
        provider_name: provider.map(|item| item.name.clone()),
        provider_website: provider.and_then(|item| item.website.clone()),
        provider_type: provider.map(|item| item.provider_type.clone()),
        endpoint_api_format: endpoint.map(|item| item.api_format.clone()),
        endpoint_api_family: endpoint.and_then(|item| item.api_family.clone()),
        endpoint_kind: endpoint.and_then(|item| item.endpoint_kind.clone()),
        provider_key_name: provider_key
            .map(|item| item.name.clone())
            .or_else(|| candidate.api_key_name.clone()),
        provider_key_auth_type: provider_key.map(|item| item.auth_type.clone()),
        provider_key_capabilities: provider_key.and_then(|item| item.capabilities.clone()),
        provider_key_is_active: provider_key.map(|item| item.is_active),
        candidate,
    }
}

fn unique_ids<'a>(items: impl Iterator<Item = &'a String>) -> Vec<String> {
    items
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_data::repository::candidates::{
        InMemoryRequestCandidateRepository, RequestCandidateStatus, StoredRequestCandidate,
    };
    use aether_data::repository::provider_catalog::{
        InMemoryProviderCatalogReadRepository, StoredProviderCatalogEndpoint,
        StoredProviderCatalogKey, StoredProviderCatalogProvider,
    };

    use super::{read_decision_trace, DecisionTrace, DecisionTraceCandidate};
    use crate::gateway::gateway_data::candidates::RequestCandidateFinalStatus;
    use crate::gateway::gateway_data::GatewayDataState;

    fn sample_candidate(request_id: &str) -> StoredRequestCandidate {
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            request_id.to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            Some("alice".to_string()),
            Some("default".to_string()),
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("provider-key-1".to_string()),
            RequestCandidateStatus::Failed,
            None,
            false,
            Some(502),
            Some("bad_gateway".to_string()),
            Some("upstream failed".to_string()),
            Some(37),
            Some(1),
            None,
            Some(serde_json::json!({"cache_1h": true})),
            100,
            Some(101),
            Some(102),
        )
        .expect("candidate should build")
    }

    fn sample_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-1".to_string(),
            "OpenAI".to_string(),
            Some("https://openai.com".to_string()),
            "custom".to_string(),
        )
        .expect("provider should build")
    }

    fn sample_endpoint() -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-1".to_string(),
            "provider-1".to_string(),
            "openai:chat".to_string(),
            Some("openai".to_string()),
            Some("chat".to_string()),
            true,
        )
        .expect("endpoint should build")
    }

    fn sample_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "provider-key-1".to_string(),
            "provider-1".to_string(),
            "prod-key".to_string(),
            "api_key".to_string(),
            Some(serde_json::json!({"cache_1h": true})),
            true,
        )
        .expect("key should build")
    }

    #[tokio::test]
    async fn enriches_request_candidate_trace_with_provider_catalog_metadata() {
        let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
            sample_candidate("req-1"),
        ]));
        let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![sample_provider()],
            vec![sample_endpoint()],
            vec![sample_key()],
        ));
        let state = GatewayDataState::with_decision_trace_readers_for_tests(
            request_candidates,
            provider_catalog,
        );

        let trace = read_decision_trace(&state, "req-1", true)
            .await
            .expect("trace should read")
            .expect("trace should exist");

        assert_eq!(
            trace,
            DecisionTrace {
                request_id: "req-1".to_string(),
                total_candidates: 1,
                final_status: RequestCandidateFinalStatus::Failed,
                total_latency_ms: 37,
                candidates: vec![DecisionTraceCandidate {
                    candidate: sample_candidate("req-1"),
                    provider_name: Some("OpenAI".to_string()),
                    provider_website: Some("https://openai.com".to_string()),
                    provider_type: Some("custom".to_string()),
                    endpoint_api_format: Some("openai:chat".to_string()),
                    endpoint_api_family: Some("openai".to_string()),
                    endpoint_kind: Some("chat".to_string()),
                    provider_key_name: Some("prod-key".to_string()),
                    provider_key_auth_type: Some("api_key".to_string()),
                    provider_key_capabilities: Some(serde_json::json!({"cache_1h": true})),
                    provider_key_is_active: Some(true),
                }],
            }
        );
    }
}
