pub(crate) mod auth;
mod auth_config;
mod network;
pub(crate) mod oauth_refresh;
pub(crate) mod policy;
pub(crate) mod rules;
pub(crate) mod snapshot;
pub(crate) mod url;

#[cfg(test)]
pub(crate) use crate::gateway::ai_pipeline::runtime::GenericOAuthRefreshAdapter;
pub(crate) use crate::gateway::ai_pipeline::runtime::{
    adapters, build_antigravity_safe_v1internal_request, build_antigravity_static_identity_headers,
    build_antigravity_v1internal_url, build_claude_code_messages_url,
    build_claude_code_passthrough_headers, build_claude_messages_url, build_gemini_content_url,
    build_gemini_files_passthrough_url, build_gemini_video_predict_long_running_url,
    build_kiro_generate_assistant_response_url, build_kiro_provider_headers,
    build_kiro_provider_request_body, build_openai_chat_url, build_openai_cli_url,
    build_openai_passthrough_headers, build_passthrough_headers_with_auth,
    build_passthrough_path_url, build_vertex_api_key_gemini_content_url,
    classify_local_antigravity_request_support, fixed_provider_template, is_codex_cli_backend_url,
    provider_type_admin_oauth_template, provider_type_enables_format_conversion_by_default,
    provider_type_is_fixed, provider_type_is_fixed_for_admin_oauth,
    provider_type_supports_local_openai_chat_transport,
    provider_type_supports_local_same_format_transport, provider_type_supports_model_fetch,
    provider_types, resolve_local_antigravity_request_auth, resolve_local_gemini_auth,
    resolve_local_openai_chat_auth, resolve_local_standard_auth,
    resolve_local_vertex_api_key_query_auth, sanitize_claude_code_request_body,
    supports_local_claude_code_transport_with_network,
    supports_local_kiro_request_transport_with_network, supports_local_openai_chat_transport,
    supports_local_standard_transport_with_network,
    supports_local_vertex_api_key_gemini_transport_with_network, AntigravityEnvelopeRequestType,
    AntigravityRequestAuthSupport, AntigravityRequestEnvelopeSupport,
    AntigravityRequestSideSupport, AntigravityRequestUrlAction, KiroAuthConfig,
    KiroOAuthRefreshAdapter, KiroRequestAuth, ProviderOAuthTemplate,
    ADMIN_PROVIDER_OAUTH_TEMPLATE_TYPES, KIRO_ENVELOPE_NAME,
};
pub(crate) use auth::{build_passthrough_headers, ensure_upstream_auth_header};
pub(crate) use network::{
    resolve_transport_execution_timeouts, resolve_transport_proxy_snapshot,
    resolve_transport_proxy_snapshot_with_tunnel_affinity, resolve_transport_tls_profile,
    transport_proxy_is_locally_supported,
};
#[cfg(test)]
pub(crate) use oauth_refresh::LocalOAuthRefreshAdapter;
pub(crate) use oauth_refresh::{
    supports_local_oauth_request_auth_resolution, CachedOAuthEntry, LocalOAuthRefreshCoordinator,
    LocalOAuthRefreshError, LocalResolvedOAuthRequestAuth,
};
pub(crate) use policy::{
    supports_local_gemini_transport, supports_local_gemini_transport_with_network,
    supports_local_standard_transport,
};
pub(crate) use rules::{
    apply_local_body_rules, apply_local_header_rules, body_rules_are_locally_supported,
    header_rules_are_locally_supported,
};
pub(crate) use snapshot::{read_provider_transport_snapshot, GatewayProviderTransportSnapshot};
