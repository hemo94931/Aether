use axum::body::Body;
use axum::http::Response;
use std::collections::BTreeMap;
use url::form_urlencoded;

use aether_data::repository::candidates::{RequestCandidateStatus, UpsertRequestCandidateRecord};
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::gateway::headers::collect_control_headers;
use crate::gateway::provider_transport::{
    apply_local_body_rules, apply_local_header_rules, build_antigravity_safe_v1internal_request,
    build_antigravity_static_identity_headers, build_antigravity_v1internal_url,
    build_claude_code_messages_url, build_claude_code_passthrough_headers,
    build_claude_messages_url, build_gemini_content_url,
    build_kiro_generate_assistant_response_url, build_kiro_provider_headers,
    build_kiro_provider_request_body, build_openai_passthrough_headers, build_passthrough_headers,
    build_passthrough_path_url, build_vertex_api_key_gemini_content_url,
    classify_local_antigravity_request_support, ensure_upstream_auth_header,
    resolve_local_gemini_auth, resolve_local_standard_auth,
    resolve_local_vertex_api_key_query_auth, resolve_transport_execution_timeouts,
    resolve_transport_proxy_snapshot_with_tunnel_affinity, resolve_transport_tls_profile,
    sanitize_claude_code_request_body, supports_local_claude_code_transport_with_network,
    supports_local_gemini_transport_with_network,
    supports_local_kiro_request_transport_with_network,
    supports_local_standard_transport_with_network,
    supports_local_vertex_api_key_gemini_transport_with_network, AntigravityEnvelopeRequestType,
    AntigravityRequestEnvelopeSupport, AntigravityRequestSideSupport, AntigravityRequestUrlAction,
    LocalResolvedOAuthRequestAuth, KIRO_ENVELOPE_NAME,
};
use crate::gateway::request_candidates::{
    current_unix_secs, record_local_request_candidate_status,
};
use crate::gateway::ai_pipeline::planner::plan_builders::{
    LocalStreamPlanAndReport, LocalSyncPlanAndReport,
};
use crate::gateway::ai_pipeline::planner::prefer_local_tunnel_owner_candidates;
use crate::gateway::scheduler::{
    list_selectable_candidates, GatewayMinimalCandidateSelectionCandidate,
};
use crate::gateway::{
    append_execution_contract_fields_to_value, execute_execution_runtime_stream,
    execute_execution_runtime_sync, AppState, ConversionMode, ExecutionStrategy,
    GatewayControlDecision, GatewayControlSyncDecisionResponse, GatewayError,
};
use crate::gateway::ai_pipeline::planner::{
    EXECUTION_RUNTIME_STREAM_DECISION_ACTION, EXECUTION_RUNTIME_SYNC_DECISION_ACTION,
};

mod family;
mod plans;
mod request;

pub(super) use self::family::{
    materialize_local_same_format_provider_candidate_attempts,
    maybe_build_local_same_format_provider_decision_payload_for_candidate,
    resolve_local_same_format_provider_decision_input, LocalSameFormatProviderFamily,
    LocalSameFormatProviderSpec,
};
pub(crate) use self::family::{
    maybe_build_stream_local_same_format_provider_decision_payload,
    maybe_build_sync_local_same_format_provider_decision_payload,
    maybe_execute_stream_via_local_same_format_provider_decision,
    maybe_execute_sync_via_local_same_format_provider_decision,
};
use self::plans::{
    build_local_stream_plan_and_reports, build_local_sync_plan_and_reports, resolve_stream_spec,
    resolve_sync_spec,
};
use self::request::{
    build_same_format_provider_request_body, build_same_format_upstream_url,
    extract_gemini_model_from_path,
};

const ANTIGRAVITY_ENVELOPE_NAME: &str = "antigravity:v1internal";
