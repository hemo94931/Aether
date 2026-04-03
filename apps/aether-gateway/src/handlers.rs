use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aether_contracts::{ExecutionPlan, ExecutionResult, ExecutionTimeouts, RequestBody};
#[cfg(test)]
use aether_crypto::DEVELOPMENT_ENCRYPTION_KEY;
use aether_crypto::{decrypt_python_fernet_ciphertext, encrypt_python_fernet_plaintext};
use aether_data::redis::{RedisKeyspace, RedisKvRunner};
use aether_data::repository::candidate_selection::StoredMinimalCandidateSelectionRow;
use aether_data::repository::candidates::{
    PublicHealthTimelineBucket, RequestCandidateStatus, StoredRequestCandidate,
};
use aether_data::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery, CreateAdminGlobalModelRecord,
    PublicGlobalModelQuery, StoredAdminGlobalModel, StoredAdminProviderModel,
    StoredPublicGlobalModel, UpdateAdminGlobalModelRecord, UpsertAdminProviderModelRecord,
};
use aether_data::repository::management_tokens::{
    CreateManagementTokenRecord, ManagementTokenListQuery, RegenerateManagementTokenSecret,
    StoredManagementToken, StoredManagementTokenUserSummary, UpdateManagementTokenRecord,
};
use aether_data::repository::oauth_providers::{
    EncryptedSecretUpdate, UpsertOAuthProviderConfigRecord,
};
use aether_data::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_data::repository::proxy_nodes::{
    ProxyNodeHeartbeatMutation, ProxyNodeTunnelStatusMutation, StoredProxyNode,
    StoredProxyNodeEvent,
};
use aether_runtime::{maybe_hold_axum_response_permit, AdmissionPermit};
use axum::body::{to_bytes, Body, Bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::Response;
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{Datelike, SecondsFormat, Utc};
use futures_util::TryStreamExt;
use regex::Regex;
pub(crate) use serde::Deserialize;
pub(crate) use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;
pub(crate) use tracing::warn;
use url::form_urlencoded;
use url::Url;
use uuid::Uuid;

use crate::gateway::api::ai::{
    admin_default_body_rules_for_signature, admin_endpoint_signature_parts,
    public_api_format_local_path,
};
use crate::gateway::constants::*;
use crate::gateway::headers::{
    extract_or_generate_trace_id, header_value_str, should_skip_request_header,
};
use crate::gateway::provider_transport::{
    fixed_provider_template, provider_type_enables_format_conversion_by_default,
    provider_type_is_fixed,
};
use crate::gateway::scheduler::{
    count_recent_rpm_requests_for_provider_key_since, is_provider_key_circuit_open,
    provider_key_health_score,
};
use crate::gateway::{
    allows_control_execute_emergency, build_client_response, build_local_auth_rejection_response,
    build_local_http_error_response, build_local_overloaded_response,
    build_local_user_rpm_limited_response, execute_execution_runtime_stream,
    execute_execution_runtime_sync, maybe_build_stream_decision_payload,
    maybe_build_stream_plan_payload, maybe_build_sync_decision_payload,
    maybe_build_sync_finalize_outcome, maybe_build_sync_plan_payload, maybe_execute_via_control,
    maybe_execute_via_execution_runtime_stream, maybe_execute_via_execution_runtime_sync,
    record_shadow_result_non_blocking, request_model_local_rejection,
    resolve_public_request_context, should_buffer_request_for_local_auth,
    trusted_auth_local_rejection, AppState, FrontdoorUserRpmOutcome, GatewayControlDecision,
    GatewayError, GatewayFallbackMetricKind, GatewayFallbackReason, GatewayPublicRequestContext,
    LocalProviderDeleteTaskState,
};

const ADMIN_PROVIDER_MAPPING_PREVIEW_MAX_KEYS: usize = 200;
const ADMIN_PROVIDER_MAPPING_PREVIEW_MAX_MODELS: usize = 500;
const ADMIN_PROVIDER_MAPPING_PREVIEW_FETCH_LIMIT: usize = 10_000;
const ADMIN_PROVIDER_POOL_SCAN_BATCH: u64 = 200;
const ADMIN_EXTERNAL_MODELS_CACHE_KEY: &str = "aether:external:models_dev";
const ADMIN_EXTERNAL_MODELS_CACHE_TTL_SECS: u64 = 15 * 60;
const ADMIN_PROVIDER_OAUTH_RUST_BACKEND_DETAIL: &str =
    "Admin provider OAuth requires Rust maintenance backend";

#[path = "handlers/proxy.rs"]
mod proxy;

use proxy::matches_model_mapping_for_models;
pub(crate) use proxy::proxy_request;

const OFFICIAL_EXTERNAL_MODEL_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "google",
    "google-vertex",
    "azure",
    "amazon-bedrock",
    "xai",
    "meta",
    "deepseek",
    "mistral",
    "cohere",
    "zhipuai",
    "alibaba",
    "minimax",
    "moonshot",
    "baichuan",
    "ai21",
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct AdminProviderPoolConfig {
    pub(crate) lru_enabled: bool,
    pub(crate) cost_window_seconds: u64,
    pub(crate) cost_limit_per_key_tokens: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct AdminProviderPoolRuntimeState {
    pub(crate) total_sticky_sessions: usize,
    pub(crate) sticky_sessions_by_key: BTreeMap<String, usize>,
    pub(crate) cooldown_reason_by_key: BTreeMap<String, String>,
    pub(crate) cooldown_ttl_by_key: BTreeMap<String, u64>,
    pub(crate) cost_window_usage_by_key: BTreeMap<String, u64>,
    pub(crate) lru_score_by_key: BTreeMap<String, f64>,
}

#[path = "handlers/admin/adaptive.rs"]
mod admin_adaptive_handler;
#[path = "handlers/admin/api_keys.rs"]
mod admin_api_keys_handler;
#[path = "handlers/admin/billing.rs"]
mod admin_billing_handler;
#[path = "handlers/admin/endpoints_health_helpers.rs"]
mod admin_endpoints_health_helpers;
#[path = "handlers/admin/gemini_files.rs"]
mod admin_gemini_files_handler;
#[path = "handlers/admin/ldap.rs"]
mod admin_ldap_handler;
#[path = "handlers/admin/models_helpers.rs"]
mod admin_models_helpers;
#[path = "handlers/admin/monitoring.rs"]
mod admin_monitoring_handler;
#[path = "handlers/admin/oauth_helpers.rs"]
mod admin_oauth_helpers;
#[path = "handlers/admin/payments.rs"]
mod admin_payments_handler;
#[path = "handlers/admin/pool.rs"]
mod admin_pool_handler;
#[path = "handlers/admin/provider_oauth/quota.rs"]
mod admin_provider_oauth_quota;
#[path = "handlers/admin/provider_oauth/refresh.rs"]
mod admin_provider_oauth_refresh;
#[path = "handlers/admin/provider_oauth/state.rs"]
mod admin_provider_oauth_state;
#[path = "handlers/admin/provider_ops.rs"]
mod admin_provider_ops;
#[path = "handlers/admin/provider_query.rs"]
mod admin_provider_query_handler;
#[path = "handlers/admin/provider_strategy.rs"]
mod admin_provider_strategy_handler;
#[path = "handlers/admin/providers_helpers.rs"]
mod admin_providers_helpers;
#[path = "handlers/admin/proxy_nodes.rs"]
mod admin_proxy_nodes_handler;
#[path = "handlers/admin/security.rs"]
mod admin_security_handler;
#[path = "handlers/admin/stats.rs"]
mod admin_stats_handler;
#[path = "handlers/admin/usage.rs"]
mod admin_usage_handler;
#[path = "handlers/admin/users.rs"]
mod admin_users_handler;
#[path = "handlers/admin/video_tasks.rs"]
mod admin_video_tasks_handler;
#[path = "handlers/admin/wallets.rs"]
mod admin_wallets_handler;
#[path = "handlers/internal/gateway.rs"]
mod internal_gateway;
#[path = "handlers/internal/gateway_helpers.rs"]
mod internal_gateway_helpers;
#[path = "handlers/public/catalog_helpers.rs"]
mod public_catalog_helpers;
#[path = "handlers/public/system_modules_helpers.rs"]
mod public_system_modules_helpers;
#[path = "handlers/shared.rs"]
mod shared;

#[path = "handlers/admin/catalog_write_helpers.rs"]
mod admin_catalog_write_helpers;
pub(crate) use self::admin_catalog_write_helpers::*;
#[path = "handlers/admin/misc_helpers.rs"]
mod admin_misc_helpers;
pub(crate) use self::admin_misc_helpers::*;

pub(crate) use self::admin_adaptive_handler::*;
pub(crate) use self::admin_api_keys_handler::*;
pub(crate) use self::admin_billing_handler::*;
pub(crate) use self::admin_endpoints_health_helpers::*;
pub(crate) use self::admin_gemini_files_handler::*;
pub(crate) use self::admin_ldap_handler::*;
pub(crate) use self::admin_models_helpers::*;
pub(crate) use self::admin_monitoring_handler::maybe_build_local_admin_monitoring_root_response as maybe_build_local_admin_monitoring_response;
pub(crate) use self::admin_oauth_helpers::*;
pub(crate) use self::admin_payments_handler::*;
pub(crate) use self::admin_pool_handler::*;
pub(crate) use self::admin_provider_oauth_quota::*;
pub(crate) use self::admin_provider_oauth_refresh::*;
pub(crate) use self::admin_provider_oauth_state::*;
pub(crate) use self::admin_provider_ops::admin_provider_ops_local_action_response;
pub(crate) use self::admin_provider_ops::maybe_build_local_admin_provider_ops_response;
pub(crate) use self::admin_provider_query_handler::*;
pub(crate) use self::admin_provider_strategy_handler::*;
pub(crate) use self::admin_providers_helpers::*;
pub(crate) use self::admin_proxy_nodes_handler::*;
pub(crate) use self::admin_security_handler::*;
pub(crate) use self::admin_stats_handler::*;
pub(crate) use self::admin_usage_handler::*;
pub(crate) use self::admin_users_handler::*;
pub(crate) use self::admin_video_tasks_handler::*;
pub(crate) use self::admin_wallets_handler::*;
pub(crate) use self::internal_gateway::maybe_build_local_internal_proxy_response_impl;
pub(crate) use self::internal_gateway_helpers::*;
pub(crate) use self::public_catalog_helpers::*;
pub(crate) use self::public_system_modules_helpers::*;
pub(crate) use self::shared::*;
