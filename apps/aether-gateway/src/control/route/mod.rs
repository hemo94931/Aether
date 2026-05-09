use axum::http::Uri;

use crate::headers::header_value_str;
use crate::{AppState, GatewayError};

mod admin;
mod ai;
mod internal;
mod oauth;
mod public_support;

use super::auth::{resolve_control_decision_auth, ControlDecisionAuthResolution};
use super::{GatewayAdminPrincipalContext, GatewayControlAuthContext, GatewayLocalAuthRejection};

#[derive(Debug, Clone)]
pub(crate) struct GatewayControlDecision {
    pub(crate) public_path: String,
    pub(crate) public_query_string: Option<String>,
    pub(crate) route_class: Option<String>,
    pub(crate) route_family: Option<String>,
    pub(crate) route_kind: Option<String>,
    pub(crate) request_auth_channel: Option<String>,
    pub(crate) auth_endpoint_signature: Option<String>,
    pub(crate) execution_runtime_candidate: bool,
    pub(crate) auth_context: Option<GatewayControlAuthContext>,
    pub(crate) admin_principal: Option<GatewayAdminPrincipalContext>,
    pub(crate) local_auth_rejection: Option<GatewayLocalAuthRejection>,
}

impl GatewayControlDecision {
    pub(crate) fn synthetic(
        public_path: impl Into<String>,
        route_class: Option<String>,
        route_family: Option<String>,
        route_kind: Option<String>,
        auth_endpoint_signature: Option<String>,
    ) -> Self {
        Self {
            public_path: public_path.into(),
            public_query_string: None,
            route_class,
            route_family,
            route_kind,
            request_auth_channel: None,
            auth_endpoint_signature,
            execution_runtime_candidate: false,
            auth_context: None,
            admin_principal: None,
            local_auth_rejection: None,
        }
    }

    pub(crate) fn proxy_path_and_query(&self) -> String {
        if let Some(query) = self
            .public_query_string
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            format!("{}?{}", self.public_path, query)
        } else {
            self.public_path.clone()
        }
    }

    pub(crate) fn is_execution_runtime_candidate(&self) -> bool {
        self.execution_runtime_candidate
    }

    pub(crate) fn with_execution_runtime_candidate(mut self, value: bool) -> Self {
        self.execution_runtime_candidate = value;
        self
    }
}

#[derive(Debug, Clone)]
pub(super) struct ClassifiedRoute {
    route_class: &'static str,
    route_family: &'static str,
    route_kind: &'static str,
    request_auth_channel: Option<&'static str>,
    auth_endpoint_signature: String,
    execution_runtime_candidate: bool,
}

pub(super) fn classified(
    route_class: &'static str,
    route_family: &'static str,
    route_kind: &'static str,
    auth_endpoint_signature: impl Into<String>,
    execution_runtime_candidate: bool,
) -> ClassifiedRoute {
    ClassifiedRoute {
        route_class,
        route_family,
        route_kind,
        request_auth_channel: None,
        auth_endpoint_signature: auth_endpoint_signature.into(),
        execution_runtime_candidate,
    }
}

pub(super) fn classified_with_request_auth_channel(
    route_class: &'static str,
    route_family: &'static str,
    route_kind: &'static str,
    request_auth_channel: &'static str,
    auth_endpoint_signature: impl Into<String>,
    execution_runtime_candidate: bool,
) -> ClassifiedRoute {
    ClassifiedRoute {
        route_class,
        route_family,
        route_kind,
        request_auth_channel: Some(request_auth_channel),
        auth_endpoint_signature: auth_endpoint_signature.into(),
        execution_runtime_candidate,
    }
}

impl ClassifiedRoute {
    fn into_decision(self, public_path: String) -> GatewayControlDecision {
        GatewayControlDecision {
            public_path,
            public_query_string: None,
            route_class: Some(self.route_class.to_string()),
            route_family: Some(self.route_family.to_string()),
            route_kind: Some(self.route_kind.to_string()),
            request_auth_channel: self.request_auth_channel.map(str::to_string),
            auth_endpoint_signature: Some(self.auth_endpoint_signature),
            execution_runtime_candidate: self.execution_runtime_candidate,
            auth_context: None,
            admin_principal: None,
            local_auth_rejection: None,
        }
    }
}

pub(crate) async fn resolve_control_route(
    state: &AppState,
    method: &http::Method,
    uri: &Uri,
    headers: &http::HeaderMap,
    trace_id: &str,
) -> Result<Option<GatewayControlDecision>, GatewayError> {
    let Some(mut decision) = classify_control_route(method, uri, headers) else {
        return Ok(None);
    };
    decision.public_query_string = uri.query().map(ToOwned::to_owned);

    match resolve_control_decision_auth(state, headers, uri, trace_id, decision).await? {
        ControlDecisionAuthResolution::Resolved(decision) => Ok(Some(decision)),
    }
}

pub(crate) fn classify_control_route(
    method: &http::Method,
    uri: &Uri,
    headers: &http::HeaderMap,
) -> Option<GatewayControlDecision> {
    let path = uri.path();
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    let public_models_auth_signature = detect_public_models_auth_signature(uri, headers);
    let ai_public_path = canonical_amp_provider_alias_path(&normalized_path)
        .unwrap_or_else(|| normalized_path.clone());

    let classified = public_support::classify_public_support_route(
        method,
        &normalized_path,
        &public_models_auth_signature,
    )
    .or_else(|| oauth::classify_oauth_route(method, &normalized_path))
    .or_else(|| admin::classify_admin_route(method, &normalized_path))
    .or_else(|| internal::classify_internal_route(method, &normalized_path))
    .or_else(|| ai::classify_ai_public_route(method, &ai_public_path, headers))?;

    Some(classified.into_decision(normalized_path))
}

pub(crate) fn canonical_amp_provider_alias_path(path: &str) -> Option<String> {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let remainder = normalized_path.strip_prefix("/api/provider/")?;
    let (provider, raw_provider_path) = remainder.split_once('/')?;
    let provider = provider.trim().to_ascii_lowercase();
    let provider_path = format!("/{}", raw_provider_path.trim_start_matches('/'));

    match provider.as_str() {
        "openai" => canonical_amp_openai_provider_path(&provider_path),
        "anthropic" | "claude" => canonical_amp_anthropic_provider_path(&provider_path),
        "google" | "gemini" => canonical_amp_google_provider_path(&provider_path),
        _ => canonical_amp_openai_compatible_provider_path(&provider_path),
    }
}

fn canonical_amp_openai_provider_path(path: &str) -> Option<String> {
    match path {
        "/v1/models" | "/models" => Some("/v1/models".to_string()),
        "/v1/chat/completions" | "/chat/completions" => Some("/v1/chat/completions".to_string()),
        "/v1/responses" | "/responses" => Some("/v1/responses".to_string()),
        "/v1/responses/compact" | "/responses/compact" => Some("/v1/responses/compact".to_string()),
        "/v1/embeddings" | "/embeddings" => Some("/v1/embeddings".to_string()),
        "/v1/rerank" | "/rerank" => Some("/v1/rerank".to_string()),
        _ => None,
    }
}

fn canonical_amp_openai_compatible_provider_path(path: &str) -> Option<String> {
    match path {
        "/v1/chat/completions" | "/chat/completions" => Some("/v1/chat/completions".to_string()),
        _ => None,
    }
}

fn canonical_amp_anthropic_provider_path(path: &str) -> Option<String> {
    match path {
        "/v1/models" | "/models" => Some("/v1/models".to_string()),
        "/v1/messages" | "/messages" => Some("/v1/messages".to_string()),
        "/v1/messages/count_tokens" | "/messages/count_tokens" => {
            Some("/v1/messages/count_tokens".to_string())
        }
        _ => None,
    }
}

fn canonical_amp_google_provider_path(path: &str) -> Option<String> {
    let path = canonical_amp_google_publisher_model_path(path).unwrap_or_else(|| path.to_string());
    if path == "/models"
        || path == "/v1/models"
        || path == "/v1beta/models"
        || path == "/v1beta1/models"
    {
        Some("/v1beta/models".to_string())
    } else if let Some(rest) = path.strip_prefix("/v1beta1/models/") {
        Some(format!("/v1beta/models/{rest}"))
    } else if is_gemini_models_route(&path)
        || is_gemini_operation_route(&path)
        || path == "/upload/v1beta/files"
        || path.starts_with("/v1beta/files")
    {
        Some(path)
    } else {
        None
    }
}

fn canonical_amp_google_publisher_model_path(path: &str) -> Option<String> {
    for prefix in [
        "/v1/publishers/google/models/",
        "/v1beta/publishers/google/models/",
        "/v1beta1/publishers/google/models/",
        "/v1/models/publishers/google/models/",
        "/v1beta/models/publishers/google/models/",
        "/v1beta1/models/publishers/google/models/",
    ] {
        if let Some(suffix) = path.strip_prefix(prefix) {
            let version = prefix
                .strip_prefix('/')
                .and_then(|value| value.split_once('/'))
                .map(|(version, _)| version)?;
            let version = if version == "v1beta1" {
                "v1beta"
            } else {
                version
            };
            return Some(format!("/{version}/models/{suffix}"));
        }
    }
    None
}

pub(super) fn detect_public_models_auth_signature(uri: &Uri, headers: &http::HeaderMap) -> String {
    let has_claude_key = header_value_str(headers, "x-api-key")
        .or_else(|| header_value_str(headers, "api-key"))
        .is_some();
    let has_anthropic_version = header_value_str(headers, "anthropic-version").is_some();
    if has_claude_key && has_anthropic_version {
        return "claude:messages".to_string();
    }

    let has_gemini_key = header_value_str(headers, "x-goog-api-key").is_some()
        || uri.query().is_some_and(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .any(|(key, value)| key == "key" && !value.trim().is_empty())
        });
    if has_gemini_key {
        return "gemini:generate_content".to_string();
    }

    if uri.path().starts_with("/v1beta/models") {
        return "gemini:generate_content".to_string();
    }

    "openai:chat".to_string()
}

pub(super) fn is_claude_cli_request(headers: &http::HeaderMap) -> bool {
    let auth_header = header_value_str(headers, http::header::AUTHORIZATION.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    auth_header.starts_with("bearer ")
}

pub(super) fn is_gemini_cli_request(headers: &http::HeaderMap) -> bool {
    let x_app = header_value_str(headers, "x-app")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if x_app.contains("cli") {
        return true;
    }

    let user_agent = header_value_str(headers, http::header::USER_AGENT.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    user_agent.contains("geminicli") || user_agent.contains("gemini-cli")
}

pub(super) fn is_gemini_models_route(path: &str) -> bool {
    (path.starts_with("/v1/models/") || path.starts_with("/v1beta/models/"))
        && (path.contains(":generateContent")
            || path.contains(":streamGenerateContent")
            || path.contains(":predictLongRunning"))
}

pub(super) fn is_gemini_operation_route(path: &str) -> bool {
    (path.starts_with("/v1beta/models/") && path.contains("/operations/"))
        || path == "/v1beta/operations"
        || path.starts_with("/v1beta/operations/")
}
