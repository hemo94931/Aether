use crate::handlers::admin::provider::shared::paths::{
    admin_provider_ops_architecture_id_from_path, is_admin_provider_ops_architectures_root,
};
use crate::handlers::admin::request::AdminRequestContext;
use crate::GatewayError;
use aether_admin::provider::ops::{get_architecture, list_architectures};
use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};

fn admin_provider_ops_architectures_list_payload() -> Vec<serde_json::Value> {
    list_architectures(false)
        .into_iter()
        .map(|architecture| architecture.api_payload())
        .collect()
}

fn admin_provider_ops_architecture_payload(architecture_id: &str) -> Option<serde_json::Value> {
    get_architecture(architecture_id).map(|architecture| architecture.api_payload())
}

pub(super) async fn maybe_build_local_admin_provider_ops_architectures_response(
    request_context: &AdminRequestContext<'_>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(decision) = request_context.decision() else {
        return Ok(None);
    };

    if decision.route_family.as_deref() == Some("provider_ops_manage")
        && decision.route_kind.as_deref() == Some("list_architectures")
        && request_context.method() == http::Method::GET
        && is_admin_provider_ops_architectures_root(request_context.path())
    {
        return Ok(Some(
            Json(admin_provider_ops_architectures_list_payload()).into_response(),
        ));
    }

    if decision.route_family.as_deref() == Some("provider_ops_manage")
        && decision.route_kind.as_deref() == Some("get_architecture")
        && request_context.method() == http::Method::GET
    {
        let Some(architecture_id) =
            admin_provider_ops_architecture_id_from_path(request_context.path())
        else {
            return Ok(Some(
                (
                    http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "detail": "架构不存在" })),
                )
                    .into_response(),
            ));
        };

        return Ok(Some(
            match admin_provider_ops_architecture_payload(&architecture_id) {
                Some(payload) => Json(payload).into_response(),
                None => (
                    http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "detail": format!("架构 {architecture_id} 不存在") })),
                )
                    .into_response(),
            },
        ));
    }

    Ok(None)
}
